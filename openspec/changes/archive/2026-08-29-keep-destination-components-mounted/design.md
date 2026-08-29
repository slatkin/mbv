## Context

See proposal.md — Why. `migrate-tui-to-tuirealm` design D6 is the target:
`UiRoot`/`Playback`/`Queue`/`Home` are session-mounted; `Browser` /
`InlineSearch` / `Feeds` destinations "stay mounted while their Service library
exists"; overlays/modals/popups mount-on-open. D6 line ~793 explicitly defers
eviction *tuning* ("a later tuning pass can add eviction") but not the
keep-while-library-exists baseline.

Today each dynamic family is reconciled by an idempotent `sync_*` / `mount_*`
that compares `self.<family>_id: Option<ComponentId>` against a freshly computed
`<family>_component_id()` and, on any difference, `umount`s the old and mounts +
`active`s the new. The predicate mixes two questions:

1. **Which Service library is selected** — `TabSelection::EmbyLibrary(index)` →
   `app.libs[index].library.id` → `BrowserKey.library_id`. Stable identity.
2. **Is this family's component the right renderer right now** — `is_wide_tv_active()`,
   `is_music_group_view()` + `is_viewing_album_folders()`, ABS browse kind. A
   transient layout/view fact.

Unmounting on a change to (2) — a wide→narrow resize, drilling into an album —
throws away component state that (1) says should persist.

Catalog retirement happens in `App`-level code the shell does not currently
observe for mount purposes: `emby_service_actions.rs:77` (`self.libs.clear()` on
Emby disconnect), `run_loop_drains.rs:54` (`audiobookshelf_libraries =
libraries` on ABS refresh), plus library-hidden config changes. `Application`
lives on `Model`, so the unmount must be a `Model`-level reconciliation.

## Goals / Non-Goals

**Goals:**
- A destination component's cursor/scroll survives switching away and back
  (different tab, different library, wide↔narrow, album↔track drill).
- A destination component is unmounted when — and only when — its
  `BrowserKey.library_id` is no longer in the live catalog.
- Focus resolution stays a single idempotent pass; no async mount steals focus.
- No new registry type; `Application`'s mounted set is the source of truth.

**Non-Goals:**
- LRU / count-capped eviction of idle destination components (D6 defers this;
  catalog size bounds the count).
- Keeping `InlineSearch` components mounted longer — they already release on
  async tab moves by design (ledger row 67) and are out of scope.
- Changing `Home`/`Feeds` lifetime (already session-mounted).
- Mouse (D16, accepted-broken).
- Migrating the narrow TV / narrow Music / album-folder legacy painters into
  components — those stay legacy underpaint until #613's underpaint change.

## Decisions

**D1 — `*_id` fields are redefined as active-destination pointers; `umount` is
removed from every `sync_*` / `mount_*`.**

`sync_emby_browser` / `sync_tv_workspace` / `sync_music_workspace` /
`sync_audiobookshelf_*` become:

```
let next_id = self.<family>_component_id();
if self.<family>_id != next_id {
    if let Some(id) = &next_id {
        if !self.application.mounted(id) {
            self.application.mount(id.clone(), Box::new(<Component>::new()), vec![])
                .expect("mount <family>");
        }
        self.push_<family>_content();   // refresh the (possibly stale) instance
    }
    self.<family>_id = next_id;         // may be None (narrow / drilled away)
}
```

No `active()` here — focus is D3's job. `next_id == None` (resize narrow, drill
into a folder) just clears the active pointer; the component stays mounted with
its last state frozen. Re-entering re-points, re-pushes content, and D3
re-activates it.

Rejected alternative: a `HashSet<ComponentId>` of mounted destinations on
`Model`. It duplicates `Application`'s bookkeeping and the boundary rules
already push toward querying the framework (`mounted()`), not mirroring it.

**D2 — one `reconcile_destination_mounts()` on `Model`, called once per tick
before the focus pass.**

It builds the live valid-key set from the catalog:

- every `app.libs[i]` → the `BrowserKey`s that library could produce
  (`Generic`/`Movies`/`HomeVideos` via `BrowserComponent`, `TvShows` via
  `TvWorkspaceComponent`, `Music` via `MusicWorkspaceComponent`) — keyed by
  `library.id`, independent of the current view mode;
- every `app.audiobookshelf_libraries[i]` → its `AudiobookshelfBook` /
  `AudiobookshelfPodcast` key.

Then: for each mounted `ComponentId::Browser(key)`, if `key.library_id` is not
in the live set, `umount` it and clear any `*_id` pointer still holding it.
`InlineSearch(key)` is included in the sweep for the same library-gone reason
(it is otherwise released by its own async-move logic; this is a safety net for
Service disconnect, not a behavior change to the normal path).

Keying on `library_id` presence — not on exact `BrowserKey` equality — means a
library that changes `collection_type` (rare, admin action) retires its old
component and the next visit mounts the new kind. Acceptable and simple.

**D3 — focus is one idempotent pass in `sync_active_destination`, after
reconciliation; mount never implies `active()`.**

Order, first match wins: a mounted blocking overlay / modal / popup keeps
native LIFO focus (return early, as today via `library_overlay_mounted()`) →
Queue when `effective_panel_focus() == Queue` and no blocking overlay (return
early, as today, issue #610) → the active destination child
(`library_child_id().filter(|c| mounted(c))`) → `UiRoot`.

This is nearly what `sync_active_destination` already does; the change is that
`sync_*` no longer calls `active()` itself (D1), so this pass is the *only*
activator of a destination, and a lazily-mounted-but-inactive sibling can't be
left active by a previous tick. Because destination components mount with no
subscriptions (`vec![]`), a mounted-inactive component receives no events — it
is inert, so leaving it mounted has no input-routing cost.

**D4 — render gates stay geometry-driven and already handle mounted-inactive.**

`render_tv_workspace_component` already returns early when
`layout.main.tv_wide_area` is zero; `render_emby_browser_component` /
`render_music_workspace_component` gate similarly on their painted areas and on
`self.<family>_id`. With D1, `self.<family>_id` is `None` whenever the family is
not the active renderer (narrow, drilled away), so the existing `let Some(id) =
self.<family>_id` guard already suppresses the view. No new gate needed; add a
test that a mounted-but-inactive destination paints nothing.

## Risks / Trade-offs

- [Risk] A kept-mounted component holds a stale content snapshot; on re-entry
  the user briefly sees old data before `push_<family>_content` runs.
  → Mitigation: D1 calls `push_<family>_content()` in the same `sync_*` branch
  that re-points to the component, before the frame is drawn. Add a test:
  switch away, mutate the library list, switch back, assert the first painted
  frame shows the new content.
- [Risk] `reconcile_destination_mounts` runs every tick and iterates mounted
  components + the catalog.
  → Mitigation: both are tiny (single-digit to low-tens). If it ever matters,
  gate it on a catalog-version counter — noted, not done (ponytail: O(n) sweep,
  add a dirty flag if tick profiling flags it).
- [Risk] Removing `active()` from `sync_*` could regress the initial-mount
  focus for the very first destination shown at startup.
  → Mitigation: `sync_active_destination` (D3) runs in the same tick after the
  `sync_*` calls (`shell_run.rs:436-442`) and activates the child; the existing
  `library_parent` / focus tests cover startup. Extend them to assert focus
  lands on the destination child on the first tick with no prior `active()`.
- [Risk] A `BrowserKey` produced by two families for one library (e.g. a music
  library that can show both `MusicWorkspaceComponent` and a generic
  `BrowserComponent` fallback) could leave two mounted components for one
  surface.
  → Mitigation: the `*_component_id()` predicates are mutually exclusive on
  `collection_type` (`"music"` vs `"tvshows"` vs generic) so at most one
  non-InlineSearch destination component exists per library at a time; the
  reconciliation keys on `library_id` presence, so both are retired together
  when the library leaves. Add a test asserting no two destination components
  share a `library_id` after a music library is visited in both album and
  generic views.

## Migration Plan

Single change, no data/protocol migration. Sequence: (1) `reconcile_destination_mounts`
+ its test; (2) strip `umount`/`active` from `sync_emby_browser`, retest Emby
browser switch-and-return; (3) repeat for TV, Music, ABS book, ABS podcast; (4)
consolidate the focus pass in `sync_active_destination` and extend the
`library_parent`/focus/queue-focus tests; (5) ledger Notes update. Rollback is a
plain revert — no persisted state touched.

## Open Questions

None that change the specs or task breakdown. Eviction tuning is explicitly
deferred by D6 and out of scope.

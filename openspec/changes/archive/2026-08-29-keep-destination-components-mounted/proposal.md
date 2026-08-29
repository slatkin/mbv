## Why

Design D6 of `migrate-tui-to-tuirealm` states that destination components "stay
mounted while their Service library exists so their cursor and scroll persist; a
component for a removed Service library is unmounted." The implementation does
the opposite: each dynamic family (`emby_browser_id`, `tv_workspace_id`,
`music_workspace_id`, `abs_podcast_id`, `abs_book_id` on `Model`) holds a single
`Option<ComponentId>` pointer and `umount`s it the moment the pointer would
change — `shell_browser.rs:139-151`, `shell_tv_workspace.rs:96-108`,
`shell_music_workspace.rs:51-64`, `shell_audiobookshelf_book.rs`,
`shell_audiobookshelf_podcast.rs`.

Switching from Emby library A to library B and back destroys A's
`BrowserComponent` and rebuilds it fresh, discarding the component-private
cursor and scroll even though `ComponentId::Browser(BrowserKey { library_id })`
is stable and was designed precisely to survive this. The keep-mounted policy is
also the enabler for issue #613's underpaint removal
(`remove-migrated-surface-underpaint`): once a re-shown destination keeps its own
state, the legacy base frame is no longer the fallback that hides the loss.

This is issue #613's keep-mounted slice, split out from the deleted
`resolve-migrated-surface-correctness` bundle.

## What Changes

- Stop `umount`ing a destination component when the active-destination pointer
  changes. A visited `BrowserComponent` / `TvWorkspaceComponent` /
  `MusicWorkspaceComponent` / `AudiobookshelfPodcastComponent` /
  `AudiobookshelfBookComponent` stays mounted in TuiRealm's `Application` for
  the rest of the session, or until its backing Service library leaves the
  catalog.
- The `Model` `*_id` fields become **active-destination pointers**, not mount
  ledgers: they select which mounted instance receives `active()`, content
  pushes, and per-frame `view()`. Mounting a destination the first time it is
  visited stays lazy; re-visiting an already-mounted destination only
  re-points and re-activates.
- The `*_component_id()` predicates keep their non-library conditions
  (`is_wide_tv_active`, `is_music_group_view` / `is_viewing_album_folders`,
  wide/narrow) — but those now gate only whether the component is the
  *active/rendered* target this frame, not whether it is mounted. A wide→narrow
  resize no longer destroys the wide component's state.
- Add one shell-owned reconciliation that unmounts destination components whose
  `BrowserKey.library_id` is no longer present in `app.libs` /
  `app.audiobookshelf_libraries` (Service disconnect, catalog refresh, library
  hidden/removed). This is the D6 "component for a removed Service library is
  unmounted" clause. No parallel component registry — `Application`'s own
  mounted set is the registry, queried via `mounted()` / the existing typed
  `ComponentId`s.
- Focus after reconciliation runs exactly once and idempotently: overlay /
  modal / popup → Queue (when Queue owns panel focus) → the active destination
  child → `UiRoot`. Mount no longer implies `active()` — a lazily-mounted
  destination that is not the active target is mounted inert (no
  subscriptions), so it never steals focus from an async mount.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `interactive-component-framework`: the "Complete conversion with no
  mixed-framework endpoint" requirement gains a concrete keep-mounted
  scenario — a `migrated` destination surface with a stable `ComponentId`
  SHALL retain its component-private cursor/scroll across destination
  switches and layout-breakpoint changes, and SHALL be unmounted only when
  its backing Service library is gone.

## Impact

- `src/app/shell.rs` (`Model` field docs; possibly a small `mounted_destinations`
  helper), `src/app/shell_browser.rs`, `src/app/shell_tv_workspace.rs`,
  `src/app/shell_music_workspace.rs`, `src/app/shell_audiobookshelf_book.rs`,
  `src/app/shell_audiobookshelf_podcast.rs`, `src/app/shell_library.rs`
  (`sync_active_destination` focus pass), `src/app/shell_run.rs` (tick order:
  one reconciliation call), and their `_tests.rs` siblings.
- No `App` state, protocol, or persistence change. `BrowseLevel.cursor`/`.scroll`
  and the ABS/TV/Music selection fields are untouched — this change is purely
  about component lifetime in `Application`.
- Sequencing: land **after** the four #611 mirror-removal changes (#615–618) so
  a kept-mounted component's cursor cannot be re-stomped by a surviving
  two-way mirror, and **before** `remove-migrated-surface-underpaint` (#613),
  which depends on re-shown destinations owning their own state.
- `docs/architecture/interactive-surface-ledger.md`: the destination rows'
  Notes cells record the keep-mounted lifetime.

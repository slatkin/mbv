## Context

Verified against `b34ee375`. `library_list_render_ctx`
(`render/components/list_context.rs:5`, `impl App`, reads
`nav_stack.last().cursor/.scroll`) has 9 non-test call sites in two groups:

| Group | Sites | Component reachable? |
|---|---|---|
| A — shell | `shell_browser.rs:204`, `shell_tv_workspace.rs:109`, `shell_music_workspace.rs:108` | yes |
| B — legacy render tree | `list.rs:145` (`render_list`), `detail.rs:106`/`:138` (`selected_movie_item`/`selected_series_item`), `detail.rs:358` (`render_compact_detail`), `music_wide.rs:140` (`wide_music_render_ctx`), `tv_wide.rs:100` (`wide_tv_render_ctx`) | no — these run under `App::compose_base_frame(f)`, and `App` holds no handle to the TuiRealm `Application` |

Actual per-surface state at the narrow breakpoint:

| Narrow surface | Component mounted? | Painted by | Symptom |
|---|---|---|---|
| generic / Movies / home video | `BrowserComponent`, yes | **both** legacy `render_list` *and* `BrowserComponent::view`, same `left_area` | double paint, diverged cursors |
| grouped Music | `MusicWorkspaceComponent`, yes (no width gate on `music_workspace_component_id`) — but **never focused**: `emby_library_child_id` (`shell_library.rs:64-68`) requires `is_wide_music_active()`, so focus falls back to `UiRoot` | legacy only — the component is placed with `wide_music_area`, empty at narrow | **all navigation dead**, and the painted cursor is frozen |
| TV | **no** | legacy only | navigation dead |
| Emby podcast | **no**, at either width | legacy narrow only | narrow dead; wide blank |
| feed / home-video group picker | **no** — excluded at `shell_browser.rs:130`; `emby_library_child_id` (`shell_library.rs:57`) returns `None` | legacy only (paints the pills) | **all navigation dead**, not only `[`/`]` |

Two corrections to an earlier reading of this table, both from the audit
recorded in #623:

- **Narrow grouped Music is F1, not merely a degenerate F2.** The component
  mounting is not the same as the component owning: it is never focused, so
  `handle_key` bails on `!self.context.focused` and every key routes to
  `UiRoot` and dies. Threading the cursor into the painter (D5) is necessary
  but **not sufficient** — the focus gate at `shell_library.rs:64-68` must
  drop its width condition as well, or the surface stays inert.
- **The feed/home-video group picker belongs in scope.** It was previously
  excluded on the grounds that it reads its own `feed_home_video.video_cursor`
  rather than `BrowseLevel`. That is true and is why it does not block #626 —
  but it is irrelevant to D1, which is about ownership, not about which field
  holds the cursor. Nothing mounts for it and every key is dead, so it is an F1
  instance like any other.

## Decisions

### D1 — One cursor owner and one painter per interactive surface, at every breakpoint

The general invariant, not a browse-specific rule:

- Every interactive surface reachable at a breakpoint has **exactly one**
  mounted component owning its cursor, scroll, and keyboard handling there.
- Exactly **one** painter runs for that surface's rect at that breakpoint.
- A breakpoint gate on a mount decision leaves no reachable width with no
  owner. `tv_workspace_component_id`'s `is_wide_tv_active()` gate is precisely
  this bug.

**This change is rule-first with an open surface list.** The invariant above is
the change; the surfaces below are the instances known at planning time. A
parallel audit is sweeping for more. A new finding is **an added row in the
task 3 checklist, not a re-plan** — it inherits the same per-surface template
(own → verify → hoist → delete legacy → verify) and the same acceptance
criteria. The design does not enumerate a fixed surface set anywhere, and
nothing downstream depends on the list's length.

Two defect families are in scope, and a new finding is classified into one:

- **F1 — no owner at some reachable breakpoint.** A mount gate excludes a width
  (narrow TV, podcast at both widths). Symptom: dead navigation.
- **F2 — two painters on one rect.** A legacy painter and a component both
  paint (narrow Movies; Queue). Symptom: ghost/doubled rows, worse once a
  cursor mirror is removed. A component that is mounted but placed with an
  empty rect (narrow grouped Music) is the degenerate case: one painter, wrong
  one.

The Queue surface is F2 (#623: `shell_run.rs:56` paints from the deliberately
stale `App::queue_cursor`, `:72` overlays `QueueComponent`) and is **deferred**
to `remove-queue-legacy-underpaint` — different painter, different state, and
its panel chrome complicates the deletion. The rule covers it and the ledger
records it in this change, so it cannot be quietly forgotten; only the fix is
elsewhere.

### D2 — The component owns *and* paints at every width; `render_list` is deleted

The end state is the one every other destination already has. `render_library`
(`widgets.rs:507`) reserves geometry and paints nothing for Home, Feeds, ABS
book, and ABS podcast. Its `EmbyLibrary` arm is the last one that still paints
a body; after this change it reserves `left_area` and returns, and `App` has no
browse painter at all.

*Rejected — keep legacy as the narrow painter and only move ownership.* It
works (the threaded cursor makes both painters agree again, the latent
pre-`6cf469e1` state) and it unblocks the field deletion, but it leaves two
painters on one rect and defers the real fix indefinitely. Maintainer decision
2026-08-29: do the hoist now.

*Rejected — suppress `render_list` narrow without hoisting.* Silently drops the
inline hero, pill rows, search box, count label, and empty-state messages, none
of which the component's narrow path renders today.

### D3 — What the hoist costs: the seams already exist

`render_list` is ~560 lines, but the wide migrations built every seam it needs
and its `impl App` dependencies are thin. Measured `self.` usage in the
painters it reaches: `compact_banner_layout_with_overview` 5,
`render_grouped_album_rows` 12, `render_series_inline_detail` 8,
`render_letter_pills_row` 1, `series_inline_detail_rows` 1. Three kinds:

1. **Image cache and fetch effects** — `images_enabled`, `fetch_card_image`,
   `fetch_list_card_image_when_idle`, `fetch_series_detail`,
   `cached_image_protocol_mut`, `card_image_loading`,
   `right_panel_image_renders_allowed`. Resolved by the established
   `HomeImagePaint` seam: the component computes the paint, the shell executes
   it via `App::paint_home_image` (`home_hero.rs:279`), which already does both
   the fetch and the stateful-image paint for wide Movies, Home, ABS
   book/podcast, and Music. Components hold no image authority and issue no
   effects (ADR 0022).
2. **Shell-computed facts** — `is_music_group_view`, `is_viewing_album_folders`,
   `is_viewing_season_grid`, `is_home_video_view`, `is_podcast_library`,
   `should_show_letter_pills`, `recursive_album_search_enabled`,
   `collection_type`, `group_album_info`, `build_grouped_album_display_plan`,
   `music_grouping.settled`. These become ctx fields. `MusicWideRenderCtx`
   (`music_wide.rs:19`) **already carries** `groups`, `group_cursor`,
   `album_info`, `album_order`, `images_enabled` — the whole grouped-album
   set — so narrow Music reuses the ctx it is already handed.
3. **Pure layout** — already free functions or trivially made so.

Reused rather than written: `render_generic_movies_home_video_rows_with_ctx`,
`render_music_group_pills_row_with_ctx` (`music.rs:43`), `render_search_box`,
`render_count_label`, `selected_detail_shell`, `prepare_wide_emby_hero_card`,
`render_letter_grouped_rows`, `render_plain_rows`, and
`BrowserComponent::render_letter_pills_row` (`browser.rs:256`), already used by
the wide branch.

The one genuinely new piece of work is splitting
`compact_banner_layout_with_overview` (`detail.rs:166`) into a **pure sizing**
function plus the fetch it currently performs as a side effect. Sizing must run
*before* the list flows, because `inline_hero_rows` determines row layout — so
the component needs the pure half and the shell keeps the fetch. Its three
impure inputs (`images_enabled`, the `right_panel_image_renders_allowed`
nav-idle gate, and whether the image is cached) become parameters. Five
`self.` sites in a ~105-line function.

### D4 — TV and podcast get `BrowserComponent`, not a narrow `TvWorkspaceComponent` view

Extend `emby_browser_component_id` to accept `BrowserKind::TvShows` and
`is_podcast_library` when `!is_wide_tv_active()`, keeping
`tv_workspace_component_id` as the wide half so the two are mutually exclusive
at every width. Narrow TV is a flat series list: exactly the cursor, scroll,
paging, and activation `BrowserComponent` already implements, and its
composition (series inline hero, season grid, letter pills) lands in the same
narrow composer as Movies and generic — one body of code for five collection
kinds instead of two.

*Rejected — a narrow `TvWorkspaceComponent` view (drop its wide-only gate).*
One component would own TV at both widths, so the cursor survives a resize, but
it must carry a second render context (`LibraryListRenderCtx` alongside
`TvWideRenderCtx`), a second key mode, and its own copy of the narrow
composition `BrowserComponent` is already gaining. Two implementations of one
surface.

*Rejected — a new narrow-TV component.* A third list-cursor implementation.

Podcast at **wide** is fixed by the same mount: today nothing paints it at all.

### D5 — Hand off through the resting position on a breakpoint flip

D4's cost is that a wide↔narrow resize hands TV between two components, so the
live cursor does not carry across. On an active-destination pointer flip,
persist the outgoing component's live cursor into the resting position and
re-anchor the incoming one from it. Both halves exist:
`persist_emby_browser_scroll` is the persist shape, `Model::music_workspace_reanchor`
(#620) the one-shot re-anchor. Same resting-position round trip a tab switch
already performs, fired on one more event.

### D6 — Grouped Music: no mount work, but a narrow view

`MusicWorkspaceComponent` is already mounted at narrow and already owns the
cursor, so no gate changes — the divergence from D4 is that TV has no owner
while Music has an owner whose value never reaches the painter. Two things
move:

- `render_music_workspace_component` (`shell_music_workspace.rs:157`) falls back
  to `layout.main.left_area` when `wide_music_area` is empty.
- `MusicWorkspaceComponent::view` gains a narrow branch —
  grouped-album rows plus the Model A hero, **not** the wide right-rail track
  table, which narrow deliberately does not have (task 3.2 of the original
  migration). Today it unconditionally calls `render_wide_music_group_with_ctx`,
  whose `publish_geometry` returns `None` at narrow.

### D7 — Migrate one surface at a time, ownership before painting

Per surface: mount/own first (keys work, cursor threaded, legacy still paints),
then hoist the paint and delete the legacy branch. Each step is independently
verifiable and revertible. Ownership for *all* surfaces lands before any paint
moves, because that is the state
`delete-browse-level-cursor-scroll` actually depends on — see Sequencing.

## Risks

- **Narrow visual regressions are the failure mode.** The narrow browser
  composes seven elements whose interaction (hero sizing feeding row flow, pill
  row and search box sharing one slot, letter grouping) is where bugs will be.
  Every paint-hoist task carries a before/after `TestBackend` snapshot at a
  narrow size, not a "component paints something" assertion.
- **File-size cap**, at HEAD line counts:

  | File | Now | After |
  |---|---|---|
  | `render/components/list.rs` | 599 | deleted — `render_list` has no production caller once `render_library`'s Emby arm reserves geometry only |
  | `components/browser.rs` | 625 | over cap; the narrow composer lands in a new sibling `components/browser_narrow.rs`, mirroring how the wide branch delegates to free ctx functions |
  | `components/music_workspace.rs` | 443 | near cap with the narrow branch; split to `music_workspace_narrow.rs` if it crosses |
  | `render/components/widgets.rs` | 645 | shrinks — the Emby paint branch of `render_library` goes |
  | `render/components/detail.rs` | 451 | flat; `compact_banner_layout_with_overview` splits into pure sizing + fetch |
  | `render/components/album.rs` | 640 | flat; gains a `_with_ctx` free-function `render_grouped_album_rows`, loses the `impl App` one |

- **Poster prefetch is an effect inside a painter.** `render_list`'s prefetch
  window calls `fetch_list_card_image_when_idle` per frame. It cannot move into
  a component. It moves to the shell beside `paint_home_image`, keyed off the
  component's selection — a behaviour-preserving relocation that must be
  verified (prefetch still fires as the cursor moves), not assumed.
- **Mouse hit geometry** moves with the paint. Accepted-broken under D16;
  do not repair, but do not silently delete a hitmap another surface reads.
- **Coupling with Change D (keyboard routing).** A surface can pass this
  change's ownership criterion — a component mounted, focused, emitting typed
  intents — and still have dead keys, because ADR 0023's policy in
  `key_policy.rs`/`router.rs` has no translation for the intent it emits.
  #623 lists roughly ten such gaps. Narrow TV is the surface to watch: D4
  mounts `BrowserComponent` for it, so its intents are `BrowserComponent`'s,
  already translated — but any *TV-specific* chord (season/episode) has no
  narrow translation and will still be dead after this change. **Verification
  consequence:** task 3's per-surface key check asserts the component *receives
  and emits*, and separately records which emitted intents currently resolve to
  a command. A dead chord attributable to a policy gap is a Change D finding,
  not a failure of this change — record it, do not fix it here.

## Sequencing

Depends on `split-browse-state-interaction-fields` (phases 1–4). Blocks
`delete-browse-level-cursor-scroll`, which needs only the **ownership** half
(task 2): once every surface has an owner and the render tree takes cursor and
scroll as parameters, `BrowseLevel::cursor` can be deleted whether or not the
paint has moved. If the hoist (task 3) stalls, B is not stranded.

Independent, not in this chain: `remove-queue-legacy-underpaint` (F2 instance,
above) and the keyboard-routing family, already scoped by the existing
`fix-router-overlay-textentry` change (#627, committed `de45cdb8`). Neither
blocks nor is blocked by #625/#626/#614.

### The #627 wide-music hitmap — investigated, no conflict

A first reading of `fix-router-overlay-textentry`'s *"Restores the wide-music
track hitmap underpaint that `dce4389d` removed"* looked like a direct
collision: #627 reintroducing an F2 instance on wide Music exactly as this
change asserts one painter per surface. **Investigation dissolved it.** Recorded
here so it is not re-raised:

- **No surviving consumer.** `wide_music_track_hitmap` is read only by
  `LayoutMain::wide_music_track_at()` (`layout.rs:243`), whose only caller is
  `MusicWorkspaceComponent::handle_mouse` (`music_workspace.rs:372`) — mouse,
  accepted-broken under D16 — and which reads the **component's own**
  `LayoutMain`, not the shell projection. The shell-side assignment
  (`shell_music_workspace.rs:181`) has no live reader at all.
- **The item is already satisfied.** `de45cdb8` recorded
  `music_resize_push_uses_current_frame_geometry` as a baseline failure, but the
  progressive-geometry series landed after it (`324386f2`, `b7e97d5b`,
  `971106ab`) and the test passes at HEAD. The hitmap is now published by the
  component's own `view` and copied into `app.layout.main` through a narrow
  accessor.
- **#627 already describes component ownership, not an underpaint.** Its design
  §5 says *"the leaf owns the geometry and the shell reads a narrow projection
  through a known accessor"* — which is this change's D1/D2 model. `dce4389d`
  removed a branch in `render_list` (`impl App`); nothing re-added an App-side
  painter and #627 never proposed to.

**No ordering constraint between #625 and #627 on this axis.** #627's remaining
work (router `text_entry_focused`, confirm-intent re-encoding, double-tap
arming, `LibraryTabJump` modifiers) touches `router.rs` / `key_policy.rs` /
`shell.rs` / `shell_modal_actions.rs` and does not overlap the hoist. #627's
task 4.1 should be annotated *satisfied by the progressive-geometry series*
rather than implemented.

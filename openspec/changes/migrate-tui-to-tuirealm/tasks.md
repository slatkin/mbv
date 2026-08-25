## How to read this task list

**Read this section before starting any task.** Three consecutive apply
sessions stalled without writing code because the previous version of this
preamble demanded a per-task bar (`delete, don't mirror`) that neither
`design.md` nor the capability spec actually requires. It has been corrected.

A surface conversion in groups 2–4 is **not** an ownership transfer. It is a
render/local-state extraction behind a shell-owned mirror. `App` keeps its
fields and its legacy input handlers until group 5. See design D14 for the
bridge contract and the reasoning.

### Standard bundle (groups 2–4)

Each surface-conversion task below bundles the following unless noted:

1. Create the component under `src/app/components/`.
2. The component owns its **rendering** and its **own local interaction
   state** (cursor/scroll/query/mode — whatever it needs to paint and to
   answer its own keys), reproducing the surface's current cursors, pills,
   panes, hero behaviour, focus targets, and keys exactly as the source
   defines them today (design §Governing Principle — there is no target
   design to invent).
3. The shell mirrors `App` into the component every tick via a
   `sync_<surface>()` method in `shell_overlays.rs` / `shell_<surface>.rs`,
   following the existing `get_component_mut` + downcast pattern (design
   D14). Async results arrive through an `apply_drain`-style push, as
   `SearchSidebarComponent` already does.
4. **Do NOT delete `App` state or `App` input handlers.** Leaving the legacy
   field and its `handle_key_*` in place, still forwarding, is the *correct*
   outcome for these tasks — not a shortcut. Deletion is group 5's job and is
   scheduled there per cluster. A task that tries to delete `App.<field>`
   will pull in every unrelated authority that reads it and will not land.
5. Emit a typed `Msg` for work crossing the component's authority boundary
   (Service calls, Player effects, nav_stack mutation, persistence). The
   component never owns an `mpsc`, a Service client, or a `PlayerProxy`.
6. Tests: local update/output tests, an `App`-free `TestBackend` render test,
   and one shell-routing test.
7. Flip the surface's `docs/architecture/interactive-surface-ledger.md` row to
   **`component`** (not `migrated` — see 1.10) with its verification record.
8. Verify with the named narrow `rtk cargo nextest` selector plus a clean
   `rtk ast-grep scan`.

Every checkpoint commit must be behaviour-preserving. None except group 5 is a
completion; a mixed framework is never a mergeable endpoint (spec:
"Complete conversion with no mixed-framework endpoint").

### Deferred by construction

These are **not** open questions blocking any group 2–4 task. They are all
answered in one place, at 5.2, when the precedence table moves as a unit:

- `key_policy.rs`'s per-key gates that cannot be a static `SubClause`
  (`playback`, `lib_search`, `album_track_mode`).
- How a per-instance `SubClause` guard is built at mount time for surfaces
  with one component per tab (`InlineSearch(BrowserKey)`, `Browser(BrowserKey)`).

Under the mirror-first bar these surfaces keep forwarding to legacy input, so
no subscription guard is needed for them until 5.2. Do not attempt to design
one earlier.

## 1. Foundation (runs the app on TuiRealm without behaviour change)

- [x] 1.1 Add `tuirealm = "4.1"` (default features already include `crossterm` and `derive`); verify `rtk cargo check -p mbv` succeeds and `Cargo.lock` resolves tuirealm 4.1 on the existing ratatui 0.30/crossterm 0.29.
- [x] 1.2 Declare `rust-version = "1.88"` in `[workspace.package]` **and** add `rust-version.workspace = true` to each member (`mbv`, `mbv-core`, `mbvd`) — a bare `[workspace.package]` entry is not inherited automatically; verify `rtk cargo check --workspace` passes and CI uses a ≥1.88 toolchain.
- [x] 1.3 Add `src/app/components/` with the `ComponentId`, `Msg`, and `UserEvent` enums from design D3–D5 (surface variants may start empty); verify `rtk cargo check -p mbv`.
- [x] 1.4 Introduce the shell `Model` holding `App` and the TuiRealm `Application<ComponentId, Msg, UserEvent>`; verify it builds and the binary still launches.
- [x] 1.5 Convert `App::run` to drive `application.tick(PollStrategy::Once(..))` and mark the frame dirty when `tick` reports a processed event (reuse the existing `had_events` → `wants_terminal_render` path). The Model keeps `App` and draws the current legacy UI and runs existing handlers **directly**; a temporary message-only `LegacyInput` component (owns no `App`) only translates terminal events into a typed legacy message the Model consumes. Verify the app boots, the first frame still precedes Remote Service startup (ADR 0018), and existing input/render characterization tests pass unchanged.
- [x] 1.6 Map each run-loop receiver (`src/app/mod.rs:412-517`: startup, player, library, Search, session, cast, shared-data, feed, image, websocket, ABS socket) to either a shell-owned adapter (default) or a TuiRealm `Port`, each injecting a `UserEvent` token; the owned model is validated in the shell by the existing generation/revision/session/image-key guards and then written into the target via `get_component_mut`+downcast. Prefer shell-owned adapters for receivers that are replaced at runtime (player, websocket, ABS socket, setup), since `restart_listener` is the only runtime port mechanism and it replaces the whole listener. Verify async-completion behaviour and stale-completion discards are unchanged by characterization.
- [x] 1.7 Add the `key_policy` ordered precedence table mirroring the current `CONTEXT_STACK` order and wire global/parent bindings as TuiRealm subscriptions with mutually-exclusive `SubClause` guards derived from it; verify against the existing ADR 0002 input-precedence tests.
- [x] 1.8 Route mouse via `EventClause::Any` subscriptions on visible top-level regions, each filtering `Event::Mouse(column,row)` against its own painted geometry and guarded `Not(IsMounted(overlay))` under blocking overlays (no shell hit-router — `Application` has no per-component event delivery); during CP1 `LegacyInput` forwards the mouse event and the Model runs the existing legacy mouse path. Verify mouse behaviour is unchanged by characterization.
- [x] 1.9 Add enforcement scaffolding: `rules/interactive-component-boundary/*.yml` (reject `impl App`, `App` as type, Service-client/`PlayerProxy` deps, `mpsc` ownership) each with one accepted + one rejected fixture, register the dir in `sgconfig.yml`, and add `.github/workflows/architecture-boundaries.yml` job `interactive-component-boundary` pinning `ast-grep` 0.44.1; verify `rtk ast-grep scan` passes and fixtures demonstrate accept/reject.
- [x] 1.10 **Doc-only, do first — reconcile the ledger vocabulary.** The ledger currently has one `migrated` state, but the spec ties `migrated` to "old fields and handlers are deleted, not synchronised with a mirror" — and all seven surfaces flipped so far still mirror (`shell_overlays.rs`) and still forward keys to their `handle_key_*`. Add the intermediate state `component` to `docs/architecture/interactive-surface-ledger.md`'s legend ("component landed and painting; shell still mirrors `App` state and/or legacy input still forwards; `App` teardown pending group 5"), demote the seven already-flipped rows (Help, Confirm, Daemon-lost, Remote-reanchor, Context menu, Global Search sidebar, Sessions) from `migrated` to `component`, and add the term to `CONTEXT.md` per AGENTS.md's new-domain-term rule. Verify: no row reads `migrated` until its group-5 teardown task lands, and 5.5's "no `legacy` or `component` row remains" gate is meaningful.

## 2. Low-risk leaf surfaces

- [x] 2.1 Convert Help sidebar (local scroll, destination-derived content); verify `rtk cargo nextest run -p mbv help` + `rtk ast-grep scan`.
- [x] 2.2 Convert Confirm modal (shared yes/no); verify `rtk cargo nextest run -p mbv confirm_modal` + scan.
- [x] 2.3 Convert Daemon-lost modal (process-lifecycle effects stay shell-owned); verify `rtk cargo nextest run -p mbv daemon_lost` + scan.
- [x] 2.4 Convert Remote-reanchor popup (reconciliation stays shell-owned); verify `rtk cargo nextest run -p mbv remote_reanchor` + scan.
- [x] 2.5 Convert Context menu (exclusive top-priority overlay with anchor geometry); verify `rtk cargo nextest run -p mbv context_menu` + scan.

## 3. Medium-risk surfaces

All group-3 tasks below are independent of each other and schedulable in any
order, **except** 3.3 and 3.5, which are gated on the shared render seam 3.11.

- [x] 3.1 Extract the Search render seam: expose `render_panel_shell*`, `render_sidebar_scrollbar`, `panel_row_text_width`, `render_panel_row` as typed render-component functions (output-preserving, no `impl App`); verify existing Search buffer characterization is unchanged.
- [x] 3.2 Convert the global Search sidebar as an ordinary row (component-owned 300 ms debounce driven by `UserEvent::Clock`; preserve the `global-search-sidebar` behaviour contract; do NOT fix its known bugs); verify `rtk cargo nextest run -p mbv search_sidebar` + scan.
- [x] 3.3 Convert inline library Search (`LibSearch`, child of one Emby browser, distinct from global Search). **Gated on 3.11.** Component owns the query string, its own results cursor/scroll, and the two candidate-pool shapes the shell pushes into it (plain fuzzy-matched items vs. recursive `AlbumSearchEntry`); the shell keeps `spawn_search_items_load`, `App.album_indexes`, and `activate_recursive_album` and pushes validated results in `apply_drain`-style. `LibraryTab.search` stays on `App` and every `search.is_some()` branch in `lib_cursor_actions.rs`/`actions_navigation.rs`/`input_mouse.rs`/`lib_event_actions.rs` stays as-is — they are deleted at 5.3a, not here. Verify `rtk cargo nextest run -p mbv inline_library_search` + scan.
- [x] 3.4 Convert Home (cross-Service rows and hero presentation). **Partly landed already:** `src/app/shell_home.rs` mounts `HomeComponent` and mirrors `App.home` per tick, and the component renders. Remaining work is the component-owned local cursor/section/scroll (today mirrored in from the legacy path via `sync_cursor_section_scroll`), the test bundle, and the ledger row (still reads `legacy`). Home's `key_policy` precedence-gate question is deferred to 5.2 — Home keeps legacy input. Verify `rtk cargo nextest run -p mbv home` + scan.
- [x] 3.5 Convert the Emby generic/Movies/home-video browser. **Gated on 3.11.** Converts only the generic/Movies/home-video-owned painting and local cursor; music-group, series-selection, and album-track branches stay behind their 3.11 seam functions and stay `impl App` until 4.2/4.3/4.4 claim them. Verify `rtk cargo nextest run -p mbv emby_browser` + scan.
- [x] 3.6 Convert Feeds (grouping, selector, list, inline hero). Component owns the feed list/selector cursor, grouping presentation, and inline hero painting; the shell keeps the refresh `mpsc`, `feed_tab_actions.rs`, `library_load_actions.rs`'s Home-Feeds section build, and `feeds_manage_actions.rs`'s reset, and mirrors `App.feed_tab` in via `sync_feeds()`. `input_feed_tab_keys.rs`/`input_mouse*.rs` keep routing; existing `App.feed_tab` tests keep passing unmodified. Teardown of all of the above is 5.3b. Verify `rtk cargo nextest run -p mbv feeds` + scan.
- [x] 3.7 Convert Sessions sidebar (merged Emby/Cast targets, fixed-stride geometry); verify `rtk cargo nextest run -p mbv sessions` + scan.
- [x] 3.8 Convert Selection modal (filters, source-specific behaviour, explicit row/selector targets); verify `rtk cargo nextest run -p mbv selection_modal` + scan.
- [x] 3.9 Convert Playback prompts (skip-intro/next-up; Player effects stay shell-owned); verify `rtk cargo nextest run -p mbv playback_prompt` + scan.
- [x] 3.10 Convert Settings nested popups — Multiselect, Library-routes, Feed-management — as `Popup` children; verify `rtk cargo nextest run -p mbv settings_popup` + scan.
- [x] 3.11 **Shared wide-list render seam — gates 3.3, 3.5, 4.2, 4.3, 4.4.** Behaviour-preserving extraction only; converts no surface and mounts no component. Mirrors what 3.1 did for Search, at the scale the browser needs. Parameterize the item-source / cursor / scroll / column / hero-sizing decisions currently made by `impl App` reading `lib.search` and `lib.nav_stack` directly into a typed context (the `ListRenderCtx` seam already present one layer down in `list_letter_groups.rs` is the target shape), across: `render/components/list.rs` (`render_list`, `render_wide_library_rows`), `render/components/tv_wide.rs` (`render_wide_tv`), `render/components/movies_wide.rs` (`render_wide_movies`, `selected_wide_movie`), `render/components/music_wide.rs` (`render_wide_music_group`, `render_wide_left_tracks`), and `render/components/detail.rs` (`selected_movie_item`, `selected_series_item`). Split each per-concern branch (generic/movies/home-video, music-group, album-track-focus, series-selection) into its own named function so 4.2/4.3/4.4 do not re-touch these files. ~1,500 lines of dense, image- and geometry-sensitive code — this is the largest single diff in the change; splitting it into sequential per-file commits is expected and encouraged. Verify: existing render characterization passes **unmodified** (no test may be edited), plus `rtk cargo nextest run -p mbv` and scan. See `scoping-3.3-3.5.md` for the full trace behind this task.

## 4. High-risk surfaces

- [x] 4.1 Convert Queue (cursor/scroll/scope move to the component; canonical queue stays in the Player owner, referenced by opaque `QueueSlotId`); verify `rtk cargo nextest run -p mbv queue` + scan.
- [x] 4.2 Convert the TV workspace (two focusable panes, season/episode child targets). **Gated on 3.11**, independent of 3.5. Verify `rtk cargo nextest run -p mbv tv_workspace` + scan.
- [x] 4.3 Convert the grouped Music workspace (album/track focus coupling, track targets). **Gated on 3.11**, independent of 3.5. Verify `rtk cargo nextest run -p mbv music_workspace` + scan.
- [x] 4.4 Convert inline album-track interaction (child state machine of the Music workspace). **Gated on 3.11**; prefer scheduling after 4.3 so both read the same seam functions, but under the mirror-first bar it may mount independently and paint over — it is not hard-blocked on 4.3. Verify `rtk cargo nextest run -p mbv album_track` + scan.
- [x] 4.5 Convert the Audiobookshelf podcast browser (show/episode workspace, selector targets); verify `rtk cargo nextest run -p mbv abs_podcast` + scan.
- [x] 4.6 Convert the Audiobookshelf book browser (browser/chapter workspace, replacement geometry); verify `rtk cargo nextest run -p mbv abs_book` + scan.
- [x] 4.7 Convert Playlists sidebar with component-owned variable-row `hit_test`; verify `rtk cargo nextest run -p mbv playlists` + scan. (Removal of the duplicated mouse-path geometry in `input_mouse_panels.rs` is 5.3c, not here.)
- [x] 4.8 Convert the Save-playlist dialog (child of the Playlists workflow); verify `rtk cargo nextest run -p mbv save_playlist` + scan.
- [x] 4.9 Convert the Settings sidebar and setup forms (Service effects stay shell-owned via `Msg::Service`); verify `rtk cargo nextest run -p mbv settings` + scan.
- [x] 4.10 Convert Playback chrome and global controls (Player authority stays outside; reduced playback-status projection only); verify `rtk cargo nextest run -p mbv playback_chrome` + scan.

## 5. Teardown, root routing, and completion gate

This is where `App` state and legacy input are **deleted**, not mirrored — the
requirement groups 2–4 deliberately defer. Teardown is scheduled by *authority
cluster*, not by surface, because that is how the entanglement actually
clusters: a single surface's field is read by several unrelated authorities
(the finding that stalled task 3.6), but a cluster's fields are read only
within the cluster plus the shell. Each teardown task requires every
contributing surface's group 2–4 conversion to have landed.

- [x] 5.1 Convert the Library parent (active destination, Panel focus/mode, child routing); verify `rtk cargo nextest run -p mbv library_parent` + scan.
- [x] 5.2 Convert Root UI + overlay-stack routing using TuiRealm's native LIFO focus stack (open = `active`, dismiss = `umount` → auto-`blur`/restore; no shell-owned focus stack), keeping only overlay z-order in the owning component. **Resolve here, as one unit, the precedence questions deferred from groups 2–4** (see "Deferred by construction"): the non-static per-key gates (`playback`, `lib_search`, `album_track_mode`) and how a per-instance `SubClause` is built at mount time for the one-component-per-tab surfaces. Verify `rtk cargo nextest run -p mbv root_ui` + scan.
- [x] 5.3-pre **Prerequisite — give `LibraryTab` a constructor.** No behavior
  change. `LibraryTab` has no constructor, so every one of the 94
  `LibraryTab { .. }` literal sites (~30 of them test modules) is a
  compile-forced edit when *any* field is deleted. That cost is constant per
  field, is paid again by each 5.3 teardown, and is the real reason task 3.6
  read as "Feeds is not an independent surface": an agent spends its context
  on ~90 identical one-line deletions before its actual change compiles. Add
  `LibraryTab::new(library: EmbyItem)` returning every other field at its
  empty value (`EmbyItem` has no `Default` derive, so the one non-defaultable
  field is the parameter), then rewrite each literal as
  `LibraryTab { <only the fields that site sets>, ..LibraryTab::new(item) }`.
  Delete no field, change no assertion. Verify `rtk cargo nextest run -p mbv`
  passes with an unchanged test count, plus `rtk cargo clippy --workspace
  --all-targets` and `rtk make check-code-file-lines`.
- [x] 5.3a **Teardown — Library/browse cluster.** Requires 3.3, 3.5, 3.11, 4.2, 4.3, 4.4, 5.1. Delete `LibraryTab`'s component-owned fields (`search`, `series_selection`/`series_season_cursor`) and the `impl App` handlers that read them: `input_browse_dispatch.rs`'s `handle_key_emby_library` branches, `input_lib_keys.rs`, `lib_cursor_actions.rs`'s eight `search.is_some()` branches, `actions_navigation.rs`'s `select`/`go_back` search arms, `lib_event_actions.rs`'s `lib.search` handlers, and the `library_search_actions.rs` query-editing path. Extract `select(lib_idx)` → resolve item → `select_item(lib_idx, item)` so plain-list Enter and the component's activation `Msg` share one body. Rewrite the `App`-based browse/search tests around the component and shell boundary. Verify `rtk cargo nextest run -p mbv` + scan + `rtk make check-code-file-lines`.
  Landed in three passes. Search (`008be6c5`..`9ac69d81`): `LibraryTab.search`,
  `LibSearch`, and `key_policy` entry 13 are gone, and `select_item` is
  extracted. Prerequisite `5.3-pre` (`5d9e77ec`) added `LibraryTab::new`.
  Series (`9e4bd7c`, `153c9b9`, `758d0a84`): `series_selection` and
  `series_season_cursor` are gone; `TvWorkspaceComponent` owns the season and
  episode cursors and resets them on a series-identity change via
  `last_series_id`. Scoping: `scoping-5.3a.md`.

  **`album_track_focus` was moved out of this cluster to 5.3d.** It is not a
  component-owned field under the current boundary, for three independent
  reasons found while scoping the album pass:

  1. *The field outlives its component.* `MusicWorkspaceComponent` mounts only
     when `is_wide_music_active()` (`shell_music_workspace.rs:16`), but
     `album_track_focus` is read by the narrow grouped-album renderer
     (`render/components/album.rs:472,508`) and written with no wide gate by
     `LibEvent::RecursiveAlbumActivated` (`lib_event_actions.rs:496`). Driving
     the field from `MusicWorkspaceComponent::track_cursor()` would zero it on
     every tick the component is unmounted.
  2. *Two of its three non-render readers sit in `impl App`, where the
     component is unreachable.* `actions.rs:164` resolves play/enqueue/context
     targets to the focused **track** rather than the album, and
     `input_resolver.rs:70` (`track_select_active`) makes Esc exit track mode
     instead of stopping playback. Both are Service/Player authority on the
     wrong side of the boundary; relocating them is `AppLayout` and
     legacy-input removal, i.e. 5.3d's work. Only the third reader,
     `shell_gates.rs:25` (`ATTR_ALBUM_TRACK_FOCUSED`), is already in `Model`.
  3. *Four of its mutation sites are inside the render tree.*
     `render/screens/album_cursor.rs` clears the field at lines 98, 147 and 206
     and gates on it at 166. See 5.3d below.

  This is the same finding that stalled task 3.6, and it is recorded here for
  the same reason: the cluster boundary in the preamble to section 5 assumes a
  field is read only within its cluster plus the shell. `album_track_focus`
  violates that, so it teardown-orders with the framework removal, not with
  the browse surfaces.
  A proposed follow-on task — relocating `render/screens/album_cursor.rs` out of
  the render tree into `src/app/album_cursor_actions.rs` — was **attempted and
  withdrawn**, and the reason belongs with 5.3d's album work. The move does not
  compile: `album_plan`'s types, fields, `row_target()`, and
  `build_grouped_album_display_plan` are all `pub(in crate::app::render)`, so a
  module outside `render` cannot name them, and a `render/mod.rs` re-export (the
  D9 idiom) exposes only the type names, not the fields the cursor code reads.
  Widening ~8 visibility markers would have been required.

  That wall is correct, not incidental. `music_group_navigation`
  (`album_cursor.rs:35-65`) *builds a display plan* to derive its navigation
  targets — grouped-album cursor movement is defined over rendered rows, which
  is what makes it column-aware. So this is render-dependent navigation, not
  interaction logic that strayed into the render tree, and the right destination
  is inward rather than outward: `MusicWorkspaceComponent`, which is already the
  owner of render-derived cursor geometry for this surface (compare
  `BrowserComponent`'s `layout`/`left_row_map` hit-testing). It cannot move
  there for the same reason `album_track_focus` cannot — the component is
  wide-only and these three `pub(in crate::app)` functions serve narrow too — so
  it is folded into 5.3d below rather than scheduled separately.
- [x] 5.3b **Teardown — Feeds cluster.** Requires 3.6, 3.4, 3.10, 5.1. Delete `FeedTabState`'s interaction fields and move its readers to the component/shell boundary: `feed_tab_actions.rs` (cursor/playback/enqueue → typed `Msg` + shell handlers), `library_load_actions.rs`'s Home-Feeds section build (→ shell-owned projection, not direct `feed_tab.all_entries` access), `feeds_manage_actions.rs`'s post-subscription reset, and the Feeds branches in `input_feed_tab_keys.rs`/`input_mouse.rs`/`input_mouse_dispatch.rs`. The refresh `mpsc` and its result validation stay shell-owned. Rewrite the `App.feed_tab` tests around the component and shell boundary. Verify `rtk cargo nextest run -p mbv feeds` + scan.
  `App.feed_tab` itself survives at `app_struct.rs:399` holding shell-owned
  fetch state (subscriptions, entries, refresh bookkeeping) — that is not
  component-owned interaction state and 5.6's gate does not require its removal.
  Loose ends verified: filtered playback selection, unchanged-snapshot cursor preservation, component group count, and exhaustive Feeds mouse routing.
- [x] 5.3c **Teardown — overlay/modal cluster.** Requires 2.1–2.5, 3.2, 3.7, 3.8, 3.9, 4.7, 4.8, 4.9, 5.2. Delete the `App` open-flags, overlay state, and the `handle_key_*` handlers the converted overlays still forward to, plus the duplicated variable-row geometry in `input_mouse_panels.rs`. Verify `rtk cargo nextest run -p mbv` + scan.
  Dispatched as named units, not as sub-numbered tasks. A unit is sized by the
  **files it forces open**, not by reference count. *Modals* measured 48 files /
  958 changed lines and consumed one agent's whole context — it compiled and
  passed 1,216 tests but had nothing left to verify or commit with. Treat
  **~45 files as the ceiling and ~25 as the target.**
  - [x] *Overlay prep* — `shell_overlays.rs` split by family, `App::ask_confirm`
    added. Behaviour-neutral; no field or clear site deleted (`75702a87`).
  - [x] *Modals* — `confirm_modal`, `daemon_lost_modal`, `remote_reanchor_popup`,
    `save_playlist_dialog`, replaced by `pending_overlay: Option<OverlayRequest>`
    (an App→shell raise/dismiss handoff) and the shell-set
    `blocking_overlay_active` adapter that subsumes the five `impl App` presence
    reads. Both legacy modal handlers and all four fields are deleted; reset
    triggers enqueue component dismissals. 48 files — the sizing reference above.
  - [x] *Sidebar state prep* — the four open-flags are an undocumented
    mutually-exclusive state machine: **39 production write sites** spread over
    `input.rs`, `input_playlist_keys.rs`, `input_settings_keys.rs`,
    `input_confirm_keys.rs`, `input_mouse_dispatch.rs`, `input_mouse_panels.rs`,
    `shell.rs`, `shell_settings.rs`, `session_switch.rs`,
    `library_load_actions.rs`, `run_loop_events_session.rs`,
    `services_settings.rs` — most of them closing a sibling to keep the
    exclusion invariant that nothing enforces. Collapse them into one
    `open_sidebar: Option<SidebarId>` with `open`/`close`/`toggle` transitions,
    behaviour-preserving, no field deleted. This is why *Sidebars* is not the
    mechanical unit its file count suggests, and it is the same prep move as
    `5.3-pre` and *Overlay prep*.
  - [x] *Sidebars* — delete `open_sidebar` and the sidebar `handle_key_*` in
    `input_settings_keys.rs`, `input_playlist_keys.rs`, and
    `services_settings.rs`. Sidebar transitions now mount/unmount the TuiRealm
    components through the shell, with component-owned Settings and Playlists
    mouse geometry. Verified with `rtk cargo check -p mbv`,
    `rtk cargo nextest run -p mbv` (1,154 passed), and
    `rtk cargo fmt --all -- --check`; the repository-wide `rtk ast-grep scan`
    remains blocked by pre-existing render-screen boundary diagnostics outside
    this unit.
  - [x] *Selection modal* — `selection_modal` + `input_selection_modal_keys.rs`.
    44 files (29 / 15), 362 refs, but only **four** write sites, all choked
    through `selection_modal_actions.rs`; the fan-out is presence-reads and
    render, which `blocking_overlay_active` already covers. At the ceiling —
    own unit.
  - [x] *Context menu* — `context_menu` + `input_context_menu.rs`.
    39 files (25 / 14), 199 refs, **nine** write sites — worse per-file fan-out
    than Selection modal despite the smaller count. Own unit. (The duplicated
    variable-row geometry in `input_mouse_panels.rs` was folded into *Sidebars*,
    which deleted that file.) Verified with `rtk cargo check -p mbv`,
    `rtk cargo nextest run -p mbv` (1,154 passed), `rtk cargo clippy
    --workspace --all-targets`, `rtk cargo fmt --all -- --check`, and
    `rtk make check-code-file-lines`.
  - [x] *Settings popups* — `multiselect_popup`, `library_routes_popup`,
    `feeds_manage_popup` + `input_feeds_manage_keys.rs`. 21 files (15 / 6),
    132 refs. One unit; the three share a parent and a dismissal path.
- [ ] 5.3d **Teardown — framework removal.** Requires 5.3a, 5.3b, 5.3c, 4.1, 4.10. Remove `LegacyInput`, `CONTEXT_STACK` interaction dispatch, `AppLayout`, all remaining duplicated mouse-coordinate paths, every `sync_<surface>()` mirror, and all remaining temporary adapters.
  Dispatched as named units, sized on the same files-forced-open basis as 5.3c:

  **Verification policy for the remaining units (decided 2026-08-25).** The
  compiler is the primary gate: `layout.main.*` alone is 158 production refs
  across 34 files, and deleting a field turns every stale reader into a build
  error — coverage no test suite here approaches. The per-unit gate is
  therefore `rtk cargo check -p mbv`, `rtk cargo clippy --workspace
  --all-targets`, `rtk cargo nextest run -p mbv` (existing coverage only),
  `rtk ast-grep scan`, and `rtk make check-code-file-lines`, with the
  maintainer's manual pass as acceptance. Do **not** write
  behaviour-preservation tests. This migration moves behaviour that has already
  drifted, so a test asserting current output pins the drift and will be
  "fixed" back the next time the underlying bug is addressed. Differential
  tests — two paths agree, no expected values encoded — are permitted only
  while both paths coexist, and are deleted together with the second path.
  Mouse units must never assert against a hand-set `layout.main.*` rect: a
  fabricated coordinate tests arithmetic against itself and can pass while the
  real app hits the wrong row. If a mouse test is written at all, it renders
  into a `TestBackend` at a known size and hit-tests the geometry that render
  produced. Regression tests for defects introduced *by* the migration are the
  one exception and are kept.
  - [x] *Album cursor prep* — settle the narrow-mode question (mount
    `MusicWorkspaceComponent` in narrow, or prove the narrow path cannot reach a
    `Some`), then move `render/screens/album_cursor.rs`'s three
    `pub(in crate::app)` entry points into `MusicWorkspaceComponent`.
    Behaviour-neutral, compiles standalone, deletes no field. Splitting this out
    is what keeps the next unit inside one context — the role `5.3-pre` and
    *Overlay prep* played. The component is now the single owner of grouped-Music
    album cursor targeting: it always emits `MusicAlbumCursor` for Up/k/Down/j/h/
    l/Home/End/PageUp/PageDown whenever it is focused and no track is focused,
    fall-through target included, so the two legacy order sources
    (`rendered_album_target` / `rendered_album_jump_target` and the
    `move_lib_cursor_inner` / `jump_lib_cursor` grouped-Music branches, plus the
    `input_lib_keys` `page_grouped_album_cursor` attempts) are deleted. Verified
    with `rtk cargo nextest run -p mbv music_workspace`, `rtk cargo nextest run -p
    mbv album_track`, `rtk cargo nextest run -p mbv
    up_down_at_group_boundary_moves_between_groups_skipping_headers`, `rtk cargo
    nextest run -p mbv` (full suite), `rtk cargo check -p mbv`, `rtk cargo clippy
    --workspace --all-targets`, `rtk cargo fmt --all -- --check`, and `rtk make
    check-code-file-lines`.
  - [ ] *Album track focus* — delete `LibraryTab.album_track_focus` and re-home
    its four `= None` resets. 30 files (21 / 9), 113 refs. Independent of 5.3c;
    may run in parallel with it.
  - [x] *Mouse gesture prep* — extract the three remaining `match self.tab`
    dispatch points in `App::handle_mouse` (`input_mouse_dispatch.rs`: selector-tab
    click, double-click activate, right-click menu) into one named method per
    gesture per surface, mirroring the already-extracted
    `handle_mouse_scroll_browse`. Behaviour-neutral, deletes no field. This is
    what makes the twelve *Mouse geometry* agents independent — today they would
    all edit the same four nested matches.
  - [ ] *Mouse geometry* — **not one unit.** Re-counted 2026-08-25: 215
    `layout.main.*` refs across 41 files (158 production across 34 files, 57
    test across 7), and **nine** components still forward mouse to legacy, none
    of which has a `hit_test` today: `browser`, `confirm`, `daemon_lost`,
    `home`, `music_workspace`, `playback_prompt`, `queue`, `remote_reanchor`,
    `tv_workspace`. (`legacy_input.rs` matches the same grep but is the bridge,
    not a surface; the earlier "12 components" predates three landed units.)
    Seven units, six to eleven runs depending on bundling. Requires 5.3c and
    *Mouse gesture prep*:
    - [x] `browser` `hit_test` — real row/hit geometry, one unit.
    - [x] `home` `hit_test` — real row/hit geometry, one unit.
    - [x] `queue` `hit_test` — real row/hit geometry, one unit.
    - [x] `tv_workspace` `hit_test` — two focusable panes, one unit.
    - [ ] `music_workspace` `hit_test` — album grid plus inline track list.
    - [ ] Blocking modals and prompt — `confirm`, `daemon_lost`,
      `remote_reanchor`, `playback_prompt`. Geometry is a containment check
      against a single rect (two legacy-mouse refs each), so these share one
      unit; split only if one proves to have real geometry.
    - [ ] Framework deletion — `input_mouse.rs` (653 lines),
      `input_mouse_dispatch.rs` (406), `input_mouse_gestures.rs` (172), and
      `AppLayout`. Cannot start until the nine surfaces above land: the
      ordering is one-directional, so this lane parallelises at the start and
      not at the end. Existing mouse tests that hand-set `layout.main.*`
      (`tests_mouse_browse_dispatch.rs` at 18 refs, plus the other six test
      files holding the remaining 57) are deleted with the fields they
      reference, not ported.
  - [ ] *Mirrors and framework* — delete the 29 `sync_*` (28 files), then
    `CONTEXT_STACK`, then `LegacyInput`, in that order. Mechanical, and shrinks
    as every unit above lands. Requires everything.
  **Also delete `LibraryTab.album_track_focus` here** (deferred from 5.3a — see
  the three reasons recorded there). Its readers can only move once the action
  layer does: relocate `actions.rs`'s focused-track target resolution and
  `input_resolver.rs`'s `track_select_active` to the shell, where
  `MusicWorkspaceComponent::track_cursor()` is reachable; drop
  `shell_gates.rs`'s `ATTR_ALBUM_TRACK_FOCUSED` projection in favour of reading
  the component directly; delete the key-mutation paths in
  `input_browse_dispatch.rs` and `action.rs`'s `AlbumTrackMove`/
  `AlbumTrackDismiss` commands. Narrow mode has no `MusicWorkspaceComponent`
  and no inline track list (`activate_album_folder_row` opens the selection
  modal instead), so the narrow readers in `render/components/album.rs` and the
  `LibEvent::RecursiveAlbumActivated` writer must be resolved explicitly — either
  by mounting the component in narrow mode or by confirming the narrow path
  cannot reach a `Some` value — not by assuming the wide path covers them.
  Move `render/screens/album_cursor.rs`'s three `pub(in crate::app)` entry
  points (`move_music_group_display_cursor`, `jump_music_group_display_cursor`,
  `page_grouped_album_cursor`) into `MusicWorkspaceComponent` in the same pass:
  they derive navigation targets from a built display plan, so they are
  component-owned render geometry, and their four `album_track_focus = None`
  resets (lines 98, 147, 206 and the gate at 166) are the same reset this
  teardown has to re-home anyway. Whichever narrow-mode answer the previous
  paragraph settles on governs both. Verify `rtk cargo check -p mbv` and that no `impl App` interaction handler and no component-local `App` field remains for any surface.
- [ ] 5.4 Confirm every mouse path reads component-owned geometry (no global hit map); verify the six precedence/mouse proofs (blocking-overlay swallow, parent/global precedence, simultaneous Queue+Library mouse, overlay blocks underlying mutation, deterministic focus restoration, geometry cannot drift).
  **Runs inside the *Mouse geometry* lane's final Framework-deletion unit, not
  as a separate lane** — it asserts exactly what that unit delivers. Under the
  verification policy recorded at 5.3d, "geometry cannot drift" is a
  structural check (`rtk ast-grep scan` plus the absence of `AppLayout` and
  the three `input_mouse*.rs` files), not a behaviour test, and none of the
  six may be written as a hand-set-coordinate mouse test. Decide the
  table-vs-runtime question below **before** that unit starts, not after.
  `KEY_POLICY` and `KeyPolicyGate::sub_clause()` are referenced nowhere outside
  `key_policy.rs`'s own ordering test — the file still carries
  `#![allow(dead_code)]`. 5.2 turned the gate descriptions into real
  `SubClause` values, but stack still runs through legacy CONTEXT_STACK
  dispatch, so the clauses do not execute until legacy input is removed at
  5.3c/5.3d. 5.4's six precedence proofs must either activate them first or
  assert against the table rather than runtime behaviour.
  Read decision **D15** in `design.md` before choosing: it scopes adopting
  `Component::perform(Cmd)` as the table's execution path (`Cmd` in, `Msg`
  out), and requires an explicit note here if 5.4 declines it.
- [ ] *Orphan cleanup* (fold into any later unit, not its own run) — `ccc75e30`
  deleted `music_group_navigation`, the only reader of
  `GroupedAlbumGroup.start`/`.end` and `GroupedAlbumCatalog.groups`
  (`src/app/music_grouping.rs:41,42,56`). rustc now reports all three dead.
  Delete the fields and whatever builds them.
- [ ] 5.5 Flip all `docs/architecture/interactive-surface-ledger.md` rows to `migrated` with verification records; verify no `legacy` **and no `component`** row remains (see 1.10).
- [ ] 5.6 Final gate: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`, and `rtk make check-code-file-lines` all pass; confirm no parallel legacy interaction framework remains and the shell Model holds only shell/runtime authority plus the TuiRealm `Application`.

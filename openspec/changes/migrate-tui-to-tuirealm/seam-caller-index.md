# Seam-caller index — migrate-tui-to-tuirealm

Read-only recon artifact. One entry per stable row ID; seeded on first handoff
to that row, re-derived only when the entry is absent or `last-verified HEAD`
differs from current `git rev-parse HEAD`. Scope is the named files in the row;
a caller recorded here that no longer exists at that line means re-derive that
row's entry before emitting. Owned by the recon agent; never edited by writers.

## 5.3d.11-U6

- seam symbol(s): `Model::sync_audiobookshelf_podcast` (RETAINED mount-only),
  new `Model::push_audiobookshelf_podcast_content` (projection, extracted),
  `Model::abs_podcast_component_id` (to zero), deleted
  `AudiobookshelfBrowseState::enter_episode_selection` (to zero)
- definition:
  - `Model::sync_audiobookshelf_podcast` — src/app/shell_audiobookshelf_podcast.rs:100–180
    (pub(super) fn; today mount/unmount + per-sync post-mount projection + cover-fetch
    bridge; after U6: mount lifecycle ONLY, mirroring the Book template
    `sync_audiobookshelf_book` shell_audiobookshelf_book.rs:32–64)
  - `Model::abs_podcast_component_id` — shell_audiobookshelf_podcast.rs:61–73
    (fn; only caller is the sync at :108 — dies with the slim sync's body rework)
  - `AudiobookshelfBrowseState::enter_episode_selection` — src/app/types_audiobookshelf_browse.rs:163–165
    (pub fn; body `self.episode_selection = Some(0);`)
  - NEW `Model::push_audiobookshelf_podcast_content` — modeled on Book template
    `push_audiobookshelf_book_content` shell_audiobookshelf_book.rs:82–117:
    guard `abs_podcast_id` → active-tab Podcast-kind guard → snapshot from
    `self.app.audiobookshelf_browse.get(index)` → `focused`/`images_enabled` →
    downcast `AudiobookshelfPodcastComponent` → `set_content(snapshot, focused, images_enabled)`
- production callers:
  - `sync_audiobookshelf_podcast`: src/app/shell.rs:1028 (per-tick effect-handoff
    block — the ONLY production call site; retained for mount lifecycle)
  - `enter_episode_selection`: **zero production callers** (only definition +
    3 test callers: types_audiobookshelf_browse.rs:549, src/app/tests_podcast.rs:228,
    :282, :325)
  - `abs_podcast_id` (field, shell.rs:60): production readers all inside
    shell_audiobookshelf_podcast.rs (sync :112/:113/:127, abs_podcast_component_mut
    :86, render_audiobookshelf_podcast_component :183)
- tests referencing the seam (all repainted to the mount-only sync + push fn):
  - src/app/shell_audiobookshelf_podcast.rs (mod tests, :242+): :273, :318, :354,
    :388, :435, :468 (model.sync_audiobookshelf_podcast()); :612 (draw path);
    :275, :320, :391, :470 (.abs_podcast_id)
  - src/app/components/audiobookshelf_podcast_component_tests.rs:157, :171
  - src/app/render/tests_audiobookshelf_podcasts.rs:37 (render_podcast_shell
    helper), :47; :125, :230, :386 (.abs_podcast_id geometry read-back)
  - src/app/shell_selection_modal_tests.rs:625, :680 (U3 modal path)
  - src/app/tests_podcast.rs:228, :282, :325 (enter_episode_selection callers →
    `set_episode_selection(Some(0))` or direct field write)
- zero-reference gate (post-teardown): zero refs to `abs_podcast_component_id`
  and `enter_episode_selection` outside their own deleted definitions.
  `sync_audiobookshelf_podcast` SURVIVES as mount-only (per user decision — do
  not delete it); it must no longer call `set_content` or the cover-fetch
  bridge (both move to `push_audiobookshelf_podcast_content`).
- cover-fetch relocation (B2, from U1): the bridge body (currently
  shell_audiobookshelf_podcast.rs:158–180 — `if self.app.images_enabled()` →
  server_url from `config.audiobookshelf_setup` → selected show →
  `fetch_audiobookshelf_cover(server, show.library_item_id)`) moves into
  `push_audiobookshelf_podcast_content`, keeping the image-disabled gate.
- writer call sites for `push_audiobookshelf_podcast_content` (mirror Book;
  see §below for exact shell.rs anchors):
  1. Fresh mount — inside retained mount-only sync (after `mount`/`active`),
     mirroring Book's `self.push_audiobookshelf_book_content();` at
     shell_audiobookshelf_book.rs:63
  2. ABS drain — shell.rs:270 `if drained_abs_events { ... }` (the comment at
     :269 already reads "re-project the active podcast browser" but currently
     calls the Book push — a latent stale-comment/mis-routed call the
     implementer fixes to call the Podcast push)
  3. lib_rx drain — shell.rs:338 region (ShowsFetched/DetailFetched async
     completions, RestoreLibraryPosition, progress reconcile; Book pushes there)
  4. Socket events — shell.rs:417 region (audiobookshelf_socket_rx drain;
     progress reconcile)
  5. Typed show-move arm — shell.rs:747–791
     `Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(movement))`; after
     the move effects + `save_audiobookshelf_position`, push
  6. Typed episode-intent arm — shell.rs:740–747
     `Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent))`;
     after `handle_audiobookshelf_podcast_episode_intent`
  7. Modal filter select — U3 already writes via
     `abs_podcast_component_mut`/`set_episode_filter`
     (audiobookshelf_podcast_modal_actions.rs:56–95); the App mirror write
     `audiobookshelf_browse[index].episode_filter = filter` there needs a push
     after it (or the push is added at the selection-modal request arm)
  Note: Podcast has NO raw-key fallthrough arm (unlike Book's
  `Msg::Shell(AudiobookshelfBookKey)` at shell.rs:798 — keys already route via
  typed intents), so no per-key push is needed; the per-tick `sync` call at
  shell.rs:1028 remains as the idempotent backstop while the component is
  mounted (mount-only, no projection).
- do-not-touch:
  - `AudiobookshelfBrowseState` type + shared members
    (`selected_id`/`episode_filter`/`episode_selection`/`scroll`) — shared with
    Book / later tasks (B1)
  - U0 accessors/mutators (`selected_id()`, `episode_selection()`,
    `episode_filter()`, `set_episode_filter()`, `set_episode_selection()`) +
    `Model::abs_podcast_component_mut` (U0) — still used by U3 modal actions,
    U5 intent handler, and `render_audiobookshelf_podcast_component`
  - U5 playback target (`handle_audiobookshelf_podcast_episode_intent` +
    `play/enqueue_selected_audiobookshelf_episode`), U3 modal filter
    (`audiobookshelf_podcast_modal_actions.rs`), U4 position persistence
    (`library_position_state.rs:226` `save_audiobookshelf_position`),
    U0–U5 component-owned playback/modal-filter/position/queue behavior
  - `render_audiobookshelf_podcast_component` (shell_audiobookshelf_podcast.rs:182–240)
    + its `view`/geometry read-back + `paint_home_image` — still called by
    shell.rs:1085; `LayoutMain.audiobookshelf_podcast_right_area` stays
    load-bearing for `is_wide_podcast_active()` (layout.rs:204)
  - the Book sibling seam `sync_audiobookshelf_book` +
    `push_audiobookshelf_book_content` + `abs_book_*` — separate row (5.3d.13)
- file scope: 3 production files — src/app/shell.rs (writer pushes + drop
  nothing at :1028), src/app/shell_audiobookshelf_podcast.rs (slim sync,
  extract push, drop `abs_podcast_component_id`), src/app/types_audiobookshelf_browse.rs
  (drop `enter_episode_selection`); plus test repaints in the files above.
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv`
  (existing coverage; podcast shell/component/render/modal tests repainted);
  `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`;
  `rtk cargo fmt --all -- --check`
- last-verified HEAD: 5ca1b0990410edf78fbf4d267e2cf899418af371 (U6 landed; PREVIOUS entry recorded pre-landing d63819dc)

## 5.3d.18a

- seam symbol(s): `TvWorkspaceComponent::handle_key` (typed-key emission),
  new `ShellRequest` TV-key variant(s) (payload-carrying pane/season/episode),
  new shell `Msg::Shell` TV-key arm routing to App legacy cursor ops
- definition:
  - `TvWorkspaceComponent::handle_key` — src/app/components/tv_workspace.rs:166–199
    (mutates local cursor/season_cursor/episode_cursor/pane AND always forwards
    `Msg::Legacy(LegacyTerminalEvent::Key(to_crossterm_key_event(key)))` — the
    double-movement 18a removes)
  - component struct fields — tv_workspace.rs:36–54 (`cursor`, `scroll`,
    `season_cursor`, `episode_cursor: Option<usize>`, `pane: Pane`
    Series/Episodes :31–34, mirror pins `last_mirrored_cursor/scroll/season/episode`,
    `last_series_id`, `layout`, `context: TvWideRenderCtx`, `initialized`)
  - `set_content(&mut self, context: TvWideRenderCtx)` — tv_workspace.rs:63–110
    (preserves local cursor across projections, ABS-podcast-style: series change
    resets season/episode/pane :64–70; first sync adopts App :71–77; later syncs
    adopt only where unmoved :78–96; clamps + stores mirror pins :97–110)
  - `move_episode(delta)` tv_workspace.rs:122–134; `move_season(delta)`
    tv_workspace.rs:136–143; `handle_mouse` :203–266 (already typed
    `TvClick`/`TvScroll`); `resolve_hit` :271–300; `view` :365–377
  - existing typed TV variants: `ShellRequest::TvScroll { delta }`
    src/app/components/msg.rs:405–407, `TvClick { region: TvHitRegion, col, row }`
    :415–419, `TvHit` :649–659, `TvHitRegion` :669–678 — NO TvKey/TvMove variant exists
- callers:
  - component handle_key: `TvWorkspaceComponent::on` tv_workspace.rs:352–354
    (`Event::Keyboard(key) => self.handle_key(key)`)
  - shell legacy routing (the path 18a bypasses): shell.rs:508
    `Msg::Legacy(LegacyTerminalEvent::Key(key))` → `App::handle_key_with_home_context`
    (input.rs:91–106, CONTEXT_STACK loop) → `handle_key_view_dispatch`
    (input.rs:178–223) → `handle_key_browse_dispatch` (input_browse_dispatch.rs:40–76)
    → `handle_key_emby_library` (input_browse_dispatch.rs:89–158) →
    series-Enter intercept :133–138 (`activate_selected_series` :162–173 → wide
    `enter_series_selection` lib_cursor_actions.rs:317 / narrow
    `open_series_selection_modal`) + `[`/`]` pill arms :101–113
    (`is_music_group_view` music_actions.rs:8, `is_feed_home_video_group_view`
    feed_actions.rs:289, `should_show_letter_pills` music_actions.rs:154) →
    `handle_lib_key` input_lib_keys.rs:94–254 (Esc/Backspace go_back :108; Up/Down
    rows ±1 :109–122; j/k :139–160; h/l 2-col :162–164; PageUp/Down
    `move_lib_cursor_rows(±lib_page_size)` :165–172; Home/End jump :173–174; Enter
    select :181; Ctrl+P play :175–180; r refresh :197)
  - App methods the typed requests will call (existing signatures):
    `move_lib_cursor_rows(lib_idx, item_rows: i64)` lib_cursor_actions.rs:93,
    `move_lib_cursor(lib_idx, delta: i64)` :175, `jump_lib_cursor(lib_idx, to_end)`
    :241, `select(lib_idx)` actions_navigation.rs:8, `go_back(lib_idx)` :196,
    `cycle_letter_pill(lib_idx, delta)` music_actions.rs:218
  - Emby typed-key template (the 18a model): `BrowserComponent::handle_crossterm_key`
    browser.rs:100–291 — Up/k Down/j `BrowserMoveRows { rows: ±1 }` :137–144,
    PageUp/Down `BrowserMoveRows { rows: ±page_rows }` :145–154, Home/End
    `BrowserJumpCursor { to_end }` :155–164, h/l guarded `columns() > 1`
    `BrowserMoveColumn { delta }` :167–184, Enter `BrowserActivate { item }`
    (resolved EmbyItem) :196–199, Esc/Backspace `BrowserBack` :240–245,
    `[`/`]` `BrowserCycleLetterPill { delta }` :246–262, fallthrough `Msg::Legacy(NoOp)`
    :263–267; shell routing `handle_browser_request` shell_browser.rs:22–81 +
    re-project arm shell.rs:817–831
  - podcast show-move template (closer for carried cursors):
    `AudiobookshelfPodcastShowMove(PodcastShowMove)` msg.rs:343, `PodcastShowMove`
    enum msg.rs:147–160, shell arm shell.rs:747–791; `PodcastEpisodeIntent`
    msg.rs:177–187 is the 18f intent template
- tests:
  - src/app/components/tv_workspace.rs inline tests :408–466:
    `tv_workspace_keeps_episode_pane_cursor_local_between_syncs` :414–434
    (drives handle_key Down — BREAKS when movement goes typed),
    `tv_workspace_series_change_resets_local_selection` :436–459
    (uses move_season directly), `tv_workspace_renders_the_wide_workspace_without_app`
    :461–466 (render smoke, safe)
  - src/app/components/tv_workspace_component_tests.rs :9–64
    (`tv_series_clicks_use_the_rendered_series_row_for_left_and_right_clicks`,
    mouse-only, SAFE)
  - src/app/shell_tv_workspace.rs: NO test module today — new shell-routing tests
    go here (browser template precedent: shell_browser.rs:283–444
    `shell_emby_browser_effects_honor_component_target`, :477–616
    `shell_emby_browser_movement_drives_app_cursor_via_typed_requests`)
  - indirect render tests (unaffected, wide output unchanged):
    render/components/tv_wide_tests.rs, render/tests_library_characterization.rs:119,
    render/tests_conformance_matrix.rs:73, render/tests_non_music.rs:123,216,
    render/components/list_tests.rs:200,245, render/components/movies_tv_header_fit_tests.rs:201–219;
    actions_tests_letter.rs:273–296 (`should_show_letter_pills` true for large tvshows)
- zero-reference gate: no `Msg::Legacy(Key)` forward from `TvWorkspaceComponent::handle_key`
  for the converted cursor keys (series-list Up/k Down/j Left/h Right/l
  PageUp/Down Home/End Enter-on-series Esc/Backspace, per-pane `[`/`]`); the
  component returns `Msg::Shell(TvMoveCursor { .. })` after its single local
  mutation. App-side legacy arms (input_lib_keys.rs / input_browse_dispatch.rs)
  stay UNTOUCHED per D14 (group-5 deletion) — they become unreachable while the
  wide TV workspace is mounted (browser-template pattern). Enter-on-Episodes-pane
  stays raw-forwarded for 18f.
- do-not-touch:
  - `sync_tv_workspace` mount gate (shell_tv_workspace.rs:13, `collection_type ==
    "tvshows" && is_wide_tv_active`) + per-frame `set_content` mirror pins
    (shell_tv_workspace.rs:24–66; season/episode pushed hardcoded 0/None :53–54 —
    App has no season/episode state; typed request may carry them or leave
    shell-side inert matching today)
  - mouse path: `TvClick` arms shell.rs:953–974 →
    `handle_mouse_double_click_tv`/`single_click_tv`/`right_click_tv`
    (mouse_gestures.rs:209–245), component handle_mouse tv_workspace.rs:203–266
  - context-menu action path (context_menu_actions.rs:25–67)
  - component Enter-on-series arm tv_workspace.rs:175–179 (feeds TvClick-style
    activation; keep)
  - episode play/enqueue: NO keyboard path exists today (18f net-new via
    `series_detail.episodes[season_id][episode_cursor]`); 18a leaves
    Enter-on-Episodes raw for 18f
  - `series_detail_cache` reader shell_tv_workspace.rs:51 (B2)
  - legacy renderer `render/components/tv_wide.rs` + list.rs wide-TV branch
    (18d unit — geometry/underpaint later)
  - D14 mirror-first: do NOT delete App fields/handlers
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv`
  (tv_workspace inline test repainted to typed emission + new shell routing tests
  in shell_tv_workspace.rs); `rtk cargo clippy --workspace --all-targets`;
  `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`
- separation: 18b (mirror-pin + per-frame set_content removal, component cursor
  authoritative, drop dual-write with move_lib_cursor_rows) is a LATER unit —
  18a keeps the mirror pins + per-frame projection in place (double-movement
  removal only, via typed requests); 18c writer pushes; 18d geometry/underpaint;
  18e teardown; 18f episode play/enqueue (net-new, needs App methods). 18a's
  `[`/`]` on Series pane: decide letter-pills vs nothing — component currently
  consumes only in Episodes pane (tv_workspace.rs:182–183) with a no-op
  Series-pane arm (:193) that still raw-forwards (letter-pill legacy risk).
  SCOUT CORRECTION to task assumption: `should_show_letter_pills` CAN be true
  for a tvshows library at root (actions_tests_letter.rs:273–296), so `[`/`]`
  letter cycling is live legacy behavior at wide-TV root — 18a must gate
  bracket keys per-pane in the component. Honest file count: 3–5 production
  files (tv_workspace.rs, msg.rs, shell.rs mandatory; input_lib_keys.rs /
  input_browse_dispatch.rs only if trimming — not required under D14;
  shell_tv_workspace.rs if a handle_tv_request helper is extracted for symmetry
  with shell_browser.rs) + test additions.
- payload-variant verdict (scout 2026-08-27, HEAD 5ca1b099): NO payload-carrying
  pane+season+episode variant needed. Every App-side series-list method needs
  ONLY `lib_idx` (shell derives from `self.app.tab.emby_library_index()`, as
  shell_browser.rs:9 does): Up/k Down/j PageUp/Down → `move_lib_cursor_rows`;
  Home/End → `jump_lib_cursor`; Esc/Backspace → `go_back`; Enter-on-series →
  `activate_selected_series`/`enter_series_selection`; h/l/Left/Right → App
  NO-OPs on wide TV (no season-grid branch — nav root is Series; columns()==1
  for is_wide_tv_active). Episodes-pane keys (`[`/`]`, Up/Down, Enter) have NO
  App-side counterpart — `move_season`/`move_episode` are component-local only
  (tv_workspace.rs:138–163), so a shell arm can only no-op there; carrying
  pane+season+episode would resolve nothing. Structural match = BrowserMoveRows
  delta-only template (msg.rs:548–558, shell_browser.rs:92) + named jump/back/
  activate variants, NOT PodcastShowMove shell-resolution (that would require
  TvWorkspaceComponent season/episode/pane accessors — NONE exist beyond
  `cursor()` tv_workspace.rs:130 — and a `tv_workspace_component_mut` helper
  that does not exist; App has zero TV season/episode state, sync pushes
  hardcoded 0/None shell_tv_workspace.rs:53–54). One real decision: legacy
  `[`/`]` double-effect today (component move_season AND App cycle_letter_pill
  both fire — tv_workspace.rs:180–181 + input_browse_dispatch.rs:94–113);
  typed conversion must pick which survives.
- last-verified HEAD: 5ca1b0990410edf78fbf4d267e2cf899418af371 (drift check +
  scout; PREVIOUS entry recorded pre-U6 d63819dc — only shell.rs changed since,
  all hunks U6 ABS writer sites, none touch TV seam)

## 5.3d.19a

- seam symbol(s): `Model::sync_music_workspace` (RETAINED; extract per-frame
  projection → new `push_music_workspace_content`), App cursor setters
  (`move_music_group_display_cursor`/`jump_music_group_display_cursor`/
  `page_grouped_album_cursor`), `Model::focused_music_track`
- definition:
  - `sync_music_workspace` — src/app/shell_music_workspace.rs:49–98 (id-compare
    mount/unmount + per-frame projection `set_content`/`set_album_columns`/
    `set_page_rows`/`set_inline_track_focus_enabled`; called every tick from
    shell.rs:1067 — the TV pre-18a shape; line shifted +14 under 18a)
  - `music_workspace_component_id` — shell_music_workspace.rs:30–48 (gate:
    `is_music_group_view(lib_idx)` + `is_viewing_album_folders(lib_idx)`;
    **`is_wide_music_active` NOT in gate** — narrow mounts by design, wide only
    gates track focus; contrast Browser child gate shell_library.rs:63–74 which
    requires wide)
  - `render_music_workspace_component` — shell_music_workspace.rs:~100
  - component (src/app/components/music_workspace.rs): local album/track
    cursors; `set_content` with `last_mirrored_*` pin (preserves local cursor
    across projections, ABS-podcast style); accessors incl. `cursor()`;
    `set_album_columns`/`set_page_rows`/`set_inline_track_focus_enabled`
  - App cursor setters (src/app/render/screens/album_cursor.rs — 3 entry
    points, 5.3d.P1): `move_music_group_display_cursor(lib_idx, target)` :13–31
    (gate `is_viewing_album_folders`; writes `nav_stack.last().cursor = idx`),
    `jump_music_group_display_cursor(lib_idx, target)` :34–53 (gate
    `is_music_group_view`; writes cursor), `page_grouped_album_cursor(lib_idx,
    target)` :56–86 (gate tab emby + PanelFocus::Library +
    is_viewing_album_folders; writes cursor + `maybe_fetch_next_page` when idle)
  - `Model::focused_music_track(lib_idx) -> Option<(String, EmbyItem)>` —
    shell_music_workspace.rs:13 (shell-side read; App passes None track cursor)
  - round-trip: component `Msg::Shell(MusicAlbumCursor{target,kind})` → shell
    arm shell.rs:570–640 → App setters → next sync's set_content (pin-mediated)
- callers (line numbers verified at HEAD f21dca98):
  - `sync_music_workspace`: shell.rs:1067 (per-tick sync block; +14 under 18a)
  - `MusicAlbumCursor` shell arm: shell.rs:570 (was :560 pre-18a)
  - cursor setters: shell.rs:570–640 MusicAlbumCursor shell arm
  - `focused_music_track`: shell.rs:605, :612, :622 (Music arms — unchanged)
  - component `MusicAlbumCursor` variant msg.rs:194–213, `AlbumCursorKind`
    msg.rs:136–140 (stable)
  - precedent: `abs_podcast_component_mut` shell_audiobookshelf_podcast.rs:75–98,
    `feeds_manage_component_mut` shell_feeds_manage.rs:62–74 — NO
    `music_workspace_component_mut` exists yet
- tests:
  - shell_music_workspace.rs inline tests (8 total, :100–455); the differential
    legacy test 19e deletes: `grouped_music_cursor_routing_matches_legacy_after_each_key`
    (:151–231) — 19a must PRESERVE it (19e deletes)
  - components/music_workspace_component_tests.rs (11 tests, 341 lines)
  - render/tests_music_characterization.rs, render/tests_music_groups.rs,
    render/components/music_wide.rs inline (:345–366), input_music_track_scope_tests.rs,
    input_music_track_navigation_tests.rs
  - 3 conformance-matrix failures PRE-EXISTING at HEAD (implementer + scout
    independently verified); all Music tests pass
- zero-reference gate: NONE for 19a (no deletions; sync RETAINED mount-only +
  projection extracted to push fn; `MusicWorkspaceComponent` + App fields stay)
- do-not-touch:
  - 19b geometry pre-pass (`wide_music_area`/`wide_music_right_area`/`left`/`hero`/
    `wide_music_art_area` before component view — chicken-and-egg R1, later row)
  - 19c legacy underpaint deletion (list.rs wide-music branch — after 19d)
  - 19d `fetch_album_tracks` relocation (images.rs, triggered only by legacy
    branch, R2 — later row)
  - 19e sync adapter removal + differential test deletion (later row)
  - `is_wide_music_active` gating semantics (wide gates track focus only)
  - component local album/track cursor ownership + `set_content` pin semantics
  - D14 mirror-first: do NOT delete App fields/handlers
- convention/probe answers (for parent decision):
  a. Idempotent mount: ALREADY idempotent under id-compare (line 51; `new()`
    only on id change) — "idempotent mount" in the row = preserve/extend the
    id-compare, no re-mount-on-every-tick bug to fix. Precedent guards:
    shell_queue.rs:8–14, shell_playback.rs:9–13, shell_inline_search.rs:100
    (`inline_search_id.is_some()` early-return), shell_feeds_manage.rs:35,62–74
  b. Content mirror: the row's "content mirror" = EXTRACT the existing per-frame
    projection (already inside sync) into `push_music_workspace_content` at
    writer seams — the U6/podcast analog. "No behavioural change" permits
    delta-only typing, no ownership change (D14 keeps App fields). Mirror is
    NOT missing — it exists per-frame today
  c. `music_workspace_component_mut` helper: does NOT exist; U6 precedent
    `abs_podcast_component_mut` (tab-guarded get_component_mut+downcast)
    suggests 19a likely needs one for the push fn — flag for implementer
  d. Track focus: component-authoritative; App passes None — the push fn must
    preserve `set_inline_track_focus_enabled` + `None` track cursor semantics
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (Music
  tests + differential test must still pass; 3 conformance failures pre-existing);
  `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`;
  `rtk cargo fmt --all -- --check`
- separation: 19a = U6-analog (mount-only sync + push extraction at writers, no
  behavioural change); 19b–19e are LATER rows (geometry, underpaint, fetch,
  teardown). 19a's honest file count: ≤3 production files (shell_music_workspace.rs,
  shell.rs writer seams, maybe components/music_workspace.rs) + test additions
- last-verified HEAD: f21dca98e5a50cb5f59a1d41a05e9cb984bc9662 (drift-checked;
  18a touched tv_workspace.rs/msg.rs/shell.rs/shell_tv_workspace.rs but NOT
  Music files — only shell.rs line numbers shifted: sync call :1053→:1067,
  MusicAlbumCursor arm :560→:570; focused_music_track :605/612/622 unchanged;
  shell_music_workspace.rs unchanged)

## 5.3d.20a

- seam symbol(s): `Model::inline_search_id: Option<ComponentId>` (DROP),
  `Model::inline_search_component_id` (rework: `==` guard → `mounted()` guard),
  `shell_library.rs:41` mount-id precedence branch (probe → `mounted()`)
- definition:
  - field decl src/app/shell.rs:59 (`pub(super) inline_search_id: Option<ComponentId>`,
    sibling block 56–61: emby_browser_id/tv_workspace_id/music_workspace_id/
    inline_search_id/abs_podcast_id/abs_book_id); init :103 (`None` in Model::new)
  - `inline_search_component_id(&self, index) -> Option<ComponentId>`
    shell_inline_search.rs:8–17 — derives `ComponentId::InlineSearch(BrowserKey{
    service: Emby, library_id: libs[index].id, kind})`; fuses derivation with
    `(self.inline_search_id.as_ref() == Some(&expected))` check; sole caller
    push_inline_search_content :54 (stale-mount release gate)
  - `open_inline_search` shell_inline_search.rs:92–144 — EmbyLibrary gate :97–99,
    double-mount gate `if self.inline_search_id.is_some() { return; }` :100–102,
    derive id :103–112, mount+active :113–117, `inline_search_id = Some(id)` :118,
    load spawn :119–131, initial push :133, optional set_loading :135–141
  - `dismiss_inline_search` :146–150 — `if let Some(id) = inline_search_id.take()
    { umount(&id) }`
  - shell_library.rs:39–55 `emby_library_child_id` — the branch at :41–47:
    `if self.inline_search_id.is_some() { return Some(ComponentId::InlineSearch(...)) }`
    (pre-empts Browser child; consumed by sync_active_destination shell_library.rs:15–27,
    called per tick shell.rs:1079)
  - all 10 production readers of the field (exhaustive): shell_inline_search.rs
    :16 (:8–17 fn), :47 (push — `let Some(id) = ... as_ref().cloned()`),
    :100 (gate), :118 (write), :147 (dismiss), :156 (activate_inline_search_item),
    :188 (set_inline_search_loading), :242 (apply_inline_search_items),
    :257 (render_inline_search_component); shell_library.rs:41 (branch). No
    render seam / run-loop sync / App / browser.rs trigger reads it
- callers: see the 10 readers above. `inline_search_component_id` has exactly 1
  caller today (:54); the reworked :41 branch could become its 2nd (zero-ref
  gate: it must keep ≥1 caller after the drop)
- tests (4 break — all become `application.mounted(&derived)` assertions):
  - shell_inline_search.rs:275–303 `inline_library_search_shell_mounts_and_routes`
    (reads `.inline_search_id.clone()` :280 + downcast)
  - actions_tests_letter.rs:153–157 `inline_search_mounts_at_the_component_boundary`
    (`assert!(model.inline_search_id.is_some())` :156)
  - input_movie_detail_tests.rs:626–633
    `opening_search_with_an_active_letter_pill_always_needs_a_full_library_fetch`
    (:630)
  - render/components/list_late_tests.rs:149–154
    `inline_search_mounts_at_the_component_boundary` (music-group app) (:153)
  - unaffected: tv_wide_tests.rs:116 `wide_tv_selected_series_follows_inline_search_cursor`
    (drives component directly); shell_library.rs tests (no field/branch reads)
- zero-reference gate: `inline_search_id` field → zero refs after drop;
  `inline_search_component_id` must keep ≥1 caller (its `==` guard becomes a
  `mounted()` guard)
- do-not-touch:
  - 20b duplicate pushes (:181 in activate_inline_search_item + :218 in
    handle_inline_search_lib_event — LATER row), 20c `apply_inline_search_items`
    (:242 reader + parent_id guard), 20d recursive Albums pool branch,
    20e `/` trigger re-host (browser.rs:289–291 `ShellRequest::OpenInlineSearch`
    → shell.rs:702 — untouched by 20a), 20f mouse left_area quirk
  - `scroll` written inside view() (render side-effect) — preserved risk, 20f
  - sibling stored-id fields (emby_browser_id/tv_workspace_id/music_workspace_id/
    abs_podcast_id/abs_book_id) — they stay; their mount/unmount is
    diff-vs-reconcile and needs the stored previous id; Inline Search is the
    FIRST surface to drop its mount-id field
- convention/probe answers (for parent decision):
  a. Shape: 20a follows the inline-search DELETION precedent (sync_inline_search
    was the first mirror deletion, f35ed7f6; mount = explicit open/dismiss pair,
    NOT a reconcile loop — single mount site, single unmount site, no per-frame
    reconcile, id a pure fn of (tab, libs[index])). This is a GENUINE new
    shape (no sibling has dropped its mount-id field yet) but the row text is
    LITERAL and mechanically real — unlike U6's "drop sync" (which conflicted
    with convention and was re-interpreted as retain-mount-only), 20a's field
    is genuinely replaceable: derive id + `application.mounted()` covers every
    read. No convention conflict to escalate.
  b. Redundant vs load-bearing: the id VALUE is redundant (derivable — the fn
    body :8–17 and mount :103–112 both construct it from tab+libs). The field
    is load-bearing only as the "is a search mounted" probe + mounted-id for
    application calls. Replacement per read: :100 gate → `mounted(&derived)`;
    :41 branch → `mounted(&derived)` (or inline_search_component_id reworked to
    probe mounted()); :47/:156/:188/:242/:257 → derive id then use as today;
    :147 dismiss → derive + umount(&derived). Caveat: a pure mounted()-gate at
    :100 is behaviorally identical to the current is_some() gate (stale mounts
    already release at :54–55 on the next push)
  c. :41 branch becomes: `if self.inline_search_component_id(index).is_some()`
    reworked to query mounted(), or `self.application.mounted(&derived_id)`;
    the returned child id is unchanged (already built from libs[index] in-fn)
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (4 test
  rewrites to mounted() assertions); `rtk cargo clippy --workspace --all-targets`;
  `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`
- separation: 20a = field drop only (shell.rs field+init, shell_inline_search.rs
  fn rework + reader derivations, shell_library.rs:41 probe); 20b–20f LATER rows
  (re-push merge, apply_inline_search_items, recursive pool, `/` trigger, mouse
  quirk). Honest file count: 3 production files (shell.rs, shell_inline_search.rs,
  shell_library.rs) + 4 test files (assertion rewrites — the row's "≤3 files"
  counts production)
- last-verified HEAD: 56e06248b899599fe19cfbdf3f2f2275ae595869 (scout; 19a +
  18a-corr commits touched shell.rs below line 1100 — all 20a lines re-verified
  current; shell_inline_search.rs + shell_library.rs unaffected by those commits)

## 5.3d.19b

- seam symbol(s): geometry fields `wide_music_area`/`wide_music_right_area`/
  `wide_music_art_area` (+ `left_area`/`hero_area` Music writes),
  `render_wide_music_group_with_ctx` (only writer), `MusicWorkspaceComponent::view`,
  `Model::render_music_workspace_component` (render seam), resize P2 ordering
- definition:
  - layout.rs fields: `wide_music_art_area: Rect` :124, `wide_music_area: Rect` :126,
    `wide_music_right_area: Rect` :129, `left_area` :81, `hero_area` :93,
    `wide_music_browser_area` :150, `wide_music_track_hitmap: Vec<(Rect,usize)>` :121;
    `is_wide_music_active()` = right_area.width>0 && height>0 :184–186
  - ONLY writer of the 3 wide_music_* fields (exhaustive grep confirmed):
    `render_wide_music_group_with_ctx` music_wide.rs:176–301 (writes wide_music_area
    :182, wide_music_art_area :184/:231, wide_music_right_area :204, left_area :213,
    hero_area :214, hitmap; wide_music_browser_area via music_wide_browser.rs:26,
    left_row_targets/left_sorted_indices :125–132). Geometry interleaved with paint.
  - 2 callers: legacy `App::render_list` wide branch list.rs:78–108 (:102) AND
    `MusicWorkspaceComponent::view` music_workspace.rs:393–400 (:397). Legacy writes
    into frame's draft LayoutMain (swap root.rs:412); component writes into its
    PRIVATE self.layout (:399) — never reaches App layout.main
  - `wide_music_render_ctx` (App method) music_wide.rs:161–213 — builds ctx from
    libs[lib_idx] nav stack; track_cursor ALWAYS None (:209–213, component
    repaints with local cursor)
  - component view music_workspace.rs:393–400 — needs NOTHING pre-paint (area
    passed in; resets own LayoutMain; geometry is OUTPUT). What breaks with stale
    App fields = the shell seam, not the paint: render_music_workspace_component
    skips when layout.main.wide_music_area is zero
  - `render_music_workspace_component` shell_music_workspace.rs:116–132 — reads
    `self.app.layout.main.wide_music_area` :122, zero-guard :123–124, application.view
    :125, take_image_paint + paint_music_image :127–130
  - legacy wide branch list.rs:78–108 — fetch_album_tracks trigger :93–98 (19d
    target, NOT 19b), ctx :100, render_wide_music_group_with_ctx :102,
    `level.scroll = output.final_scroll` write-back :103–105 (no component-side
    equivalent), paint_music_image :106. Self-contained early-return.
  - resize flow: `Msg::Legacy(LegacyTerminalEvent::Resize)` shell.rs:554–562 —
    force_clear + clear card images + push_inline_search_content +
    push_music_workspace_content (EVENT-time, pre-render). Layout rebuilt EVERY
    frame in App::render from f.area() (root.rs:27–28,129); zero-size guard leaves
    self.layout untouched (root.rs:27–31)
- callers / readers:
  - `wide_music_area` read: shell_music_workspace.rs:122 only
  - `wide_music_art_area`: NO readers (only doc comment; mouse uses hitmap +
    browser_area) — legacy-paint bookkeeping
  - App-side input reads of layout.main geometry (must survive 19c):
    `current_library_columns` lib_cursor_actions.rs:69–89 (left_area.width),
    `lib_page_size` actions.rs:121 (left_area.height), `is_wide_music_active`
    shell_library.rs:54 + actions_navigation.rs:188, album_cursor.rs gates
  - push path reads (PREVIOUS frame's installed layout, event-time):
    push_music_workspace_content shell_music_workspace.rs:79–113 —
    `is_wide_music_active()` :97, `left_area.height` :102 (set_page_rows),
    plus set_album_columns(current_library_columns) :98
- tests:
  - render/tests_music_characterization.rs:52–75 `music_group_pill_row_and_targets_are_characterized_end_to_end`
    (reads draft layout wide_music_right_area — breaks if legacy branch stops
    writing geometry); :109–114 `narrow_grouped_music_publishes_no_wide_track_targets`
    (asserts !is_wide_music_active at 60-wide — depends on legacy branch NOT firing)
  - shell_music_workspace.rs tests set wide_music_* manually then sync/push:
    :147–148 shell_mounts_and_syncs, :308–317 shell_mounts_music_workspace_in_narrow_mode
    (asserts wide_music_area stays 0×0 — **breaks if a pre-pass writes nonzero
    geometry on mount**), :356–357 wide_enter_track_focus, :417–418/:444–445
    recursive_album / position_restore
  - input_music_track_navigation_tests.rs:25–30 + input_music_track_scope_tests.rs:24–25
    (set rects manually — encode the push-needs-App-rects contract)
  - render/tests_music_groups.rs narrow assertions (unaffected if narrow branch
    untouched); render/tests_conformance_matrix.rs Music rows 270/312/419/472
    (break only on 19c legacy output change); component tests
    music_workspace_component_tests.rs:144–157 (self-contained, safe)
- zero-reference gate: NONE (19b adds/moves geometry writes; no deletions)
- do-not-touch:
  - fetch_album_tracks trigger (list.rs:93–98) — 19d owns it, must NOT move with
    geometry
  - `level.scroll` write-back (list.rs:103–105) — decide who persists scroll
    (component album_scroll internal; set_content re-mirrors per push)
  - narrow music rendering (list.rs narrow branch, tests_music_groups.rs) —
    untouched
  - 19c legacy underpaint deletion — AFTER 19b + 19d; 19e teardown
- convention/probe answers (for parent decision):
  a. Geometry is a PURE FUNCTION of `area` (same area → same values) — a 19b
    pre-pass + the legacy branch running = IDEMPOTENT DUPLICATE mid-migration,
    harmless. 19b does not need to delete anything.
  b. The pre-pass must write the **App frame's** layout.main (not component-private):
    render_music_workspace_component reads layout.main.wide_music_area :122, and
    App-side input (current_library_columns/lib_page_size/is_wide_music_active/
    album_cursor) reads layout.main after 19c. If 19b writes only the component's
    private layout, 19c breaks input. Pre-pass can reuse existing
    `wide_library_panes` + `wide_music_left_layout` arrangements (no new
    arrangement code).
  c. R1 chicken-and-egg: today legacy branch runs first → writes draft layout →
    swap → component view reads same-frame geometry. 19b must run the
    computation before render_music_workspace_component (shell.rs:1130) writing
    the App layout.main; then 19c deletes the legacy branch cleanly.
  d. RESIZE-SEQUENCING P2 (19a reviewer, structurally 19b's): the Resize event
    push (shell.rs:554–562) runs PRE-render reading LAST frame's layout.main —
    set_inline_track_focus_enabled(wide)/set_page_rows can be stale for one
    frame across wide↔narrow. Fixing properly = shell.rs ordering change (push
    after the frame's layout rebuild) — pulls shell.rs into 19b's scope beyond
    the 2 named files. SCOPE QUESTION for parent: defer P2 to a 19b sub-slice /
    separate unit, or fold into 19b (3-4 files)?
  e. Honest file count: 2–4 production (music_wide.rs, shell_music_workspace.rs,
    + root.rs or list.rs for pre-pass invocation, + shell.rs if resize P2 lands)
    + 2 test files (shell_music_workspace.rs, tests_music_characterization.rs).
    "≤3 files" only achievable if P2 deferred or handled inside
    shell_music_workspace.rs
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (music
  characterization + shell_music_workspace + input_music_track_* + conformance
  Music rows); `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`;
  `rtk cargo fmt --all -- --check`
- separation: 19b = geometry pre-pass + resize P2 decision; 19c (underpaint
  delete, after 19d), 19d (fetch relocation — trigger stays in list.rs for 19b),
  19e (teardown + differential test delete) are LATER rows
- last-verified HEAD: fc5cefa7420de5264c7df52e344ce487f12a81c9 (scout; 20a touched
  shell_inline_search.rs/shell_library.rs only; 19a reshaped
  shell_music_workspace.rs — shell.rs lines may shift, re-verify on handoff)

## 5.3d.19d

- seam symbol(s): `App::fetch_album_tracks` (album_id trigger), fields
  `album_tracks_cache`/`album_tracks_loading`, `push_music_workspace_content`
  (writer seam for relocated trigger), legacy wide-music fetch block list.rs:93-99
- definition:
  - `fetch_album_tracks(&mut self, album_id: String)` src/app/images.rs:91-118 —
    guard: skip if `album_tracks_loading.contains` OR `album_tracks_cache.contains_key`;
    insert loading, emby_snapshot else remove+return, spawn thread → get_items_sorted →
    `LibEvent::AlbumTracksFetched { album_id, tracks }` on lib_tx.
    Completion handler lib_event_actions.rs:556-567 (removes loading, sorts, inserts
    cache, refresh_selection_modal(Album,...))
  - **ROW-TEXT CORRECTION (scout): the premise "list.rs is the ONLY caller" is
    WRONG at HEAD.** 3 production callers: (1) list.rs:97 legacy wide-music
    per-frame trigger (19d target), (2) album_plan.rs:268/299/391 narrow
    grouped-album inline-detail plan (`build_grouped_album_display_plan`, gated
    `fetch_missing_tracks` always true via render_grouped_album_rows album.rs:
    115-134/145-160; fires `!hero_handles_detail && idx==cursor` :253 or
    expand_selected), (3) selection_modal_actions.rs:118 open_album_selection_modal
    (narrow Enter/dbl-click). 19d = WIDE-MUSIC trigger only; narrow paths keep
    their own triggers
  - legacy trigger list.rs:78-108 branch, fetch block :93-99 (independently
    removable — self-contained `if let Some(album)` between gate and
    `let ctx = self.wide_music_render_ctx(lib_idx)` :100; deletion leaves
    ctx-build/paint/scroll/hitmap intact; 19c whole-branch deletion absorbs it if
    left)
  - `push_music_workspace_content` shell_music_workspace.rs:79-113 — has
    `selected_album: Option<EmbyItem>` via `self.app.wide_music_render_ctx(index)`
    :95 (music_wide.rs:135-138 nav_stack.last().items.get(cursor); ctx also reads
    album_tracks/album_tracks_loading music_wide.rs:165-173). Call sites: shell.rs
    272/286/344/417/428/552/607/619/627/638/1128 (resize-deferred, post-App::render
    inside terminal.draw) + shell_music_workspace.rs:63 (sync mount) + tests
    shell_music_workspace.rs:339/367/462/479/524 + input_music_track_navigation_tests.rs
    :156/178. render_music_workspace_component (shell_music_workspace.rs:118-140)
    called only shell.rs:1137 (inside same draw closure, AFTER resize push :1128)
- callers: 3 production callers (above); direct tests actions_tests_routes.rs:111-135
  (`fetch_album_tracks_is_a_no_op_when_already_cached`/`_loading` — break only if
  fn signature/guard changes, not trigger move). NO existing test asserts a wide
  render fires the fetch (render tests seed cache first: tests_music_characterization.rs
  :88, tests_music_groups.rs:149/272/480, input push_tracks support) — 19d should
  ADD one on the push path
- zero-reference gate: NONE for fetch_album_tracks (2 other callers stay). Gate =
  push-path trigger must be cache/loading-guarded (no double-fire vs legacy which
  is removed)
- do-not-touch: narrow triggers album_plan.rs + selection_modal_actions.rs (stay);
  fetch_album_tracks stays an App method (NOT moved into the component); legacy
  branch's geometry/paint/scroll (19c's deletion); 19b's pre-pass + resize
  ordering; shell.rs resize arm :1128 (already post-render)
- convention/probe answers (for parent decision):
  a. Row text is LITERALLY WRONG ("only caller") — real scope = wide-music trigger
    relocation; the fetch is already fired from narrow paths. 19d = drop list.rs
    :93-99 + add guarded trigger in push_music_workspace_content (uses ctx.selected_
    album.id, same cache/loading guard). Implementer must not "relocate the fetch"
    wholesale (album_plan/selection_modal are narrow and correct)
  b. Mount-time push (shell_music_workspace.rs:63, sync_music_workspace) fires
    pre-render on zero/stale layout; legacy wide gate shared_hero_presentation(area)
    has NO exact push equivalent (push uses is_wide_music_active() from layout.main,
    fresh only post-19b for pushes after App::render). Fetch is cache/loading-guarded
    so early push at mount just starts the fetch slightly early — no double-fire;
    no behavior regression
  c. Retry semantics change (R2 residual): legacy per-frame retried failed fetches
    every frame; push fires on cursor-move/event boundaries — failed fetch retried
    on next interaction, not per-frame. Acceptable (matches sibling surfaces'
    event-driven patterns)
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (new
  push-path test + actions_tests_routes + shell_selection_modal + music suites);
  `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`;
  `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures
  tests_conformance_matrix.rs:227/291/339
- separation: 19d = wide-music fetch trigger only (list.rs :93-99 drop + push-path
  guarded trigger + 1 new test); 19c (underpaint delete — NOW unblocked by 19b+19d)
  LATER; 19e teardown LATER. Honest files: 2 production (list.rs, shell_music_
  workspace.rs) + 1 test file (≤3 row bound holds)
- last-verified HEAD: 4f85b92ed6f646e61c6a7e4ac3e12316141f4348 (scout; 19b corr)

## 5.3d.18b

- seam symbol(s): `TvWorkspaceComponent` pins `last_mirrored_cursor`/`last_mirrored_scroll`/
  `last_mirrored_season`/`last_mirrored_episode` (DROP), `TvWorkspaceComponent::set_content`
  (mirror rework), `Model::sync_tv_workspace` (per-frame mirror), `handle_tv_request`
  TvMoveRows arm (dual-write removal), `App::move_lib_cursor_rows` (App-side write, KEEPS
  other callers)
- definition:
  - pins tv_workspace.rs:37-40 (decl), :64-67 (init), single read each :96/99/102/105,
    single write each :124-127 — ALL inside set_content (:73-130). Pin logic :96-105:
    `if self.cursor == self.last_mirrored_cursor { self.cursor = context.list.cursor(); }`
    — App mirror wins UNLESS the component cursor moved since last frame. Scroll pin is
    a NO-OP today (component never mutates scroll)
  - `set_content` writes cursor/scroll/season/episode/pane/context; component self-mutates
    cursor (:163-173), season (:140-148), episode (:132-138 Enter/Esc), pane (:213-226,
    mouse :291-320)
  - `sync_tv_workspace` shell_tv_workspace.rs:46-92; per-frame `tv.set_content(context)` :86;
    SOLE caller shell.rs:1079 (per main-loop tick, ~50ms/8ms poll — per-frame, not per-event)
  - `handle_tv_request` shell_tv_workspace.rs:8-35: `TvMoveRows => app.move_lib_cursor_rows` :13
  - `move_lib_cursor_rows(&mut self, lib_idx, item_rows)` lib_cursor_actions.rs:93 →
    move_lib_cursor :188 → move_lib_cursor_inner :196-242 — mutates `nav_stack.last().cursor`
    (+ position save, idle page-fetch). Callers: shell_tv_workspace.rs:13 (TV arm),
    shell_browser.rs:92, input_lib_keys.rs :113/120/149/156/167/171 (legacy series-list
    arms), list_tests.rs (tests)
- callers / dual-write (per keypress, TWO files):
  series-list Down → component handle_key moves LOCAL cursor + emits `TvMoveRows` →
  shell.rs:986 → handle_tv_request:13 → move_lib_cursor_rows (App write #1) → same frame
  shell.rs:1079 → sync_tv_workspace → set_content :86 (App mirror write #2, pin-gated).
  Authority TODAY = App cursor (pin lets mirror win); component cursor is between-frame only
- tests:
  - shell_tv_workspace.rs:110-127 `typed_tv_requests_route_series_effects_through_app` —
    ONLY dual-write test (TvMoveRows→cursor==1, TvJumpCursor→0, column/episode/season→
    unchanged). BREAKS under (a) component-authoritative (premise becomes false); green
    under (b)
  - tv_workspace_component_tests.rs — 5 STANDALONE fns (:125 tv_grouped_cursor_mirrors_
    rendered_sorted_rows cursor==2, :161 typed-request emission, :69 queue-focus fallthrough
    cursor==0, :13 mouse, :100 bracket modifiers) — green under both; drive component directly
  - NO test references last_mirrored_* or sync_tv_workspace — the mirror seam is ENTIRELY
    untested (single set_content per test → pin branch never exercised; pin removal breaks
    ZERO tests)
  - lib_cursor_actions.rs: NO test module. Narrow: tests_non_music.rs:210,
    movies_tv_header_fit_tests.rs:191/208, conformance-matrix TV cases (legacy render_library)
  - pre-existing baseline red (ignore): tests_conformance_matrix.rs:227/291/339
- zero-reference gate: the 4 pins → zero refs post-drop (all reads/writes inside
  set_content, which 18b reworks); move_lib_cursor_rows KEEPS callers (shell_browser.rs:92 +
  input_lib_keys arms stay — legacy path until 18e); sync_tv_workspace stays (18e removes);
  NO deletion of typed requests (18c-f own those)
- do-not-touch: render/; shared move_lib_cursor_rows/jump_lib_cursor (other callers);
  mount/umount + context build shell_tv_workspace.rs; series_detail_cache reader :51 (B2,
  18c); episode/season/pane typed requests + request routing (18c-f); input_lib_keys legacy
  arms (unreachable-while-wide, kept until group-5 per D14); 19d in-flight files (list.rs,
  shell_music_workspace.rs, 1 Music test)
- convention/probe answers → SUPERVISOR DECISION (CalmArrow, 2026-08-27, option 1):
  18b removes TV cursor pins/mirroring + App-side TV-arm cursor authority (move rows AND
  jump), while RETAINING the temporary per-frame `set_content` sync for NON-cursor
  content only (season/episode/pane/context) until 18c's writer push replaces it.
  `sync_tv_workspace` is NOT deleted (18e owns teardown). TvJumpCursor arm included in
  the TV-arm authority removal (coherent transition; row text named only move rows).
  Implementer: do NOT add push_tv_workspace_content in 18b (that's 18c); do NOT touch
  episode/season/pane request routing; do NOT delete sync_tv_workspace
  b. scroll pin is a NO-OP (component never mutates scroll) — safe to drop with the others
  c. RESOLVED: TvJumpCursor App-side TV-arm removal lands WITH the cursor arm (decision above)
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (rewrite
  typed_tv_requests_route_series_effects_through_app for component-authoritative; tv
  component tests + tv_wide_tests + narrow TV tests stay green); `rtk cargo clippy
  --workspace --all-targets`; `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`
- separation: 18b = pin drop + set_content cursor-mirror removal + TV-arm dual-write
  removal (move rows + jump) + RETAIN non-cursor set_content until 18c (≤3 files:
  tv_workspace.rs, shell_tv_workspace.rs, shell.rs arm) + 1 test rewrite
  (typed_tv_requests_route_series_effects_through_app → component-authoritative:
  TvMoveRows/TvJumpCursor no longer move the App cursor; season/episode/column arms
  still assert unchanged App cursor); 18c writer pushes, 18d geometry/underpaint,
  18e teardown (sync_tv_workspace), 18f episode play/enqueue — LATER rows
- last-verified HEAD: 4f85b92ed6f646e61c6a7e4ac3e12316141f4348 (scouts A+B; 19d writer
  mid-edit on list.rs/shell_music_workspace.rs — disjoint from 18b, excluded from reads;
  re-verify shell.rs:986/1079 on handoff since 19d may shift them)

## 5.3d.20b

- seam symbol(s): `Model::push_inline_search_content` (call sites), event-scoped
  re-projection; suspected "redundant re-pushes" at (pre-20a) :184/:218
- definition (verified at HEAD ee28c78c):
  - `push_inline_search_content` shell_inline_search.rs:69-117 — derives expected id
    from tab, `filter(|id| application.mounted(id))` else `unmount_stale_inline_searches`;
    builds pool (SearchPool::Albums if recursive else nav_stack items/all_items) +
    loading + focused (`effective_panel_focus()==Library`); set_content + optional
    set_loading. Deterministic in App state; idempotent re-push
  - THREE call sites (unchanged since 20a): :153 open_inline_search (120-163) —
    INITIAL push after mount/activate + initial-load spawns (comment :151-153
    "the deleted mirror's first-frame projection, at the open event"); :206
    activate_inline_search_item (174-214) — re-push after synchronous select_item/
    activate_recursive_album (comment :203-206 "exactly as the deleted per-frame
    mirror did on the following tick"); :241 handle_inline_search_lib_event
    (231-244) — guarded by `pushes_inline_search` match
  - `pushes_inline_search` match :233-238 = `Refreshed | AllItemsPrefetched |
    AlbumIndexBuilt | NavigateTo` (4 variants). `Loaded`, `RecursiveAlbumActivated`,
    `SearchItemsLoaded` are NOT in it
  - shell.rs push sites: :317 RestoreLibraryPosition arm (direct push + handle_lib_event;
    RestoreLibraryPosition NOT in the match — third-way, position-restore at startup),
    :561 terminal Resize arm (direct push — only reachable focus/layout change while
    mounted). Both separate from the 3 shell_inline_search.rs sites
  - apply_inline_search_items (shell.rs:297-301 → shell_inline_search.rs:245-270)
    = direct set_content(SearchPool::Items, false, true) + set_loading(false) —
    bypasses push_inline_search_content; NOT redundant with the 3 sites
- callers: NO test file references push_inline_search_content/activate_inline_search_item/
  open_inline_search/handle_inline_search_lib_event (grep src/app/**/*tests*.rs = zero).
  activate_inline_search_item's only caller = shell.rs:710 (Msg::Shell(InlineSearchActivate)).
  open_inline_search ← shell.rs:703 (OpenInlineSearch). handle_inline_search_lib_event ←
  shell.rs:331 (lib_rx drain default arm)
- tests (B): break-list = NONE. All green: actions_tests_letter.rs:153
  inline_search_mounts_at_the_component_boundary (mounted only), input_movie_detail_tests.rs:625
  (mounted only), shell_inline_search.rs:302 inline_library_search_shell_mounts_and_routes
  (mounted+routed), :331 inline_search_tab_switch_unmounts_stale_component_before_open
  (unmount assert — fires from unmount_stale branch :78-81, LOAD-BEARING, any merge must
  keep it reachable), actions_tests.rs:177/208/227 + render/tests_album_listing.rs:10
  (direct component use). NO test counts push invocations / asserts content projection
- zero-reference gate: NONE for 20b (push_inline_search_content keeps callers; no symbol
  deleted). Gate = if the row is re-scoped to a real merge, unmount_stale branch :78-81
  must stay reachable (test :331) + playable/recursive activation re-projection must
  survive
- do-not-touch: apply_inline_search_items + parent_id guard (20c); recursive Albums pool
  branch (20d); `/` trigger re-host browser.rs:289-291 (20e); mouse left_area quirk (20f);
  `scroll` in view() (preserved risk); shell.rs:317 RestoreLibraryPosition + :561 Resize
  pushes (distinct event paths, not duplicates); music/19d files (list.rs,
  shell_music_workspace.rs, music_workspace.rs) — 19d landed, not in 20b scope
- convention/probe → FACT + parity-preserving recommendation (per standing correction,
  NOT a user decision):
  a. FACT: the row's "duplicate re-pushes at 184/218" premise is CONTRADICTED at HEAD.
    :184/:218 (pre-20a f35ed7f6) = today's :206/:241, but they are DIFFERENT event
    paths, not a same-event double: :206 is the activation's synchronous re-projection
    (playable-item activation + activate_recursive_album send NO matching LibEvent — a
    delete loses their re-projection entirely), and :241's 4 variants cover distinct
    async completions :206 does not. The only genuine same-activation double-push is
    the CHAIN :206 → folder nav `Loaded` → pill `Refreshed` → :241 (and Refreshed→
    AllItemsPrefetched → :241) — one push per distinct content change (pill-filtered
    pool, warm-up pool), not a duplicate. RECOMMENDATION: 20b is a NO-OP / re-word
    row (close as "not redundant" or re-scope to the shell.rs:317 vs :241
    RestoreLibraryPosition pairing, which IS a same-arm-adjacent redundancy and is
    behavior-preserving to merge into handle_inline_search_lib_event). Parity
    preserved either way — no visible behavior change
  b. If a merge is nonetheless pursued: the merged single push must preserve :241's
    4-variant coverage INCLUDING the stale-mount release on tab moves AND :206's
    synchronous coverage (without :206, playable + recursive activations get NO
    re-projection)
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (inline search
  suites stay green — shell_inline_search.rs:302/:331, actions_tests_letter, letter
  pill, input_movie_detail); `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep
  scan`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures
  tests_conformance_matrix.rs:227/291/339
- separation: 20b as scoped = NO-OP or re-word (fact above); if re-scoped to
  shell.rs:317 RestoreLibraryPosition push merge → 2 files (shell.rs arm,
  shell_inline_search.rs match add) + tests. 20c-f LATER rows
- last-verified HEAD: ee28c78c8439a19772f89299deb90ba25bd03244 (scouts A+B; 19d corr
  ee28c78c touched only music_workspace.rs + shell_music_workspace.rs — 20b lines
  unaffected)

## 5.3d.20c

- seam symbol(s): `Model::apply_inline_search_items` (DROP/re-home), its `parent_id`
  guard (load-bearing), `LibEvent::SearchItemsLoaded` route, App-side no-op arm
  lib_event_actions.rs:451-456
- definition (verified at HEAD ecd28f9b):
  - `apply_inline_search_items(&mut self, lib_idx, parent_id: String, items: Vec<EmbyItem>)`
    shell_inline_search.rs:245-280 — guards current_idx==lib_idx (:254) AND
    `nav_stack.last().parent_id == parent_id` (:260-261), then inline_search_component_id
    (:17) → `search.set_content(SearchPool::Items(items), false, true)` (:273) +
    set_loading(false) (:274). pub(super); NO App-state writes; the ONLY consumer of
    SearchItemsLoaded
  - SOLE caller shell.rs:298-302 (lib_rx drain arm, grep-verified @dce4389d; +1
    shift from 18c SeriesDetailFetched arm) — passes event payload verbatim
    (parent_id captured at spawn from nav_stack.last().parent_id
    library_browse_actions.rs:600); fn re-derives CURRENT level parent_id internally
  - sender: spawn_search_items_load library_browse_actions.rs:595-631 (only called
    shell_inline_search.rs:148 open_inline_search non-recursive needs_full_load path);
    sends SearchItemsLoaded :624. App-side arm lib_event_actions.rs:451-456 is a NO-OP
    (`let _ = (lib_idx, parent_id, items)`) — the fetched items exist ONLY in the event
  - SearchItemsLoaded NOT in pushes_inline_search match (:233-238 Refreshed/
    AllItemsPrefetched/AlbumIndexBuilt/NavigateTo). Push flat branch :99-105 derives
    `nav_stack.last().all_items.clone().unwrap_or_else(|| level.items.clone())` — the
    partial/letter-filtered items, NOT the full fetched set
- callers: 1 production (shell.rs:302) + doc-comment shell_inline_search.rs:62. Zero
  test-file refs to apply_inline_search_items / SearchItemsLoaded / spawn_search_items_load
  (actions_tests_letter.rs:222 is a comment; actions_tests.rs:211 + render/tests_album_listing.rs:12
  + components/inline_search.rs:72/231/237/251/272 are component-level set_content(SearchPool::Items)
  — no shell/event)
- tests (B): break-list = NONE (test-invisible, 20b precedent). No test constructs/pumps
  SearchItemsLoaded; no test pins flat completion (selected_item/query/loading) via shell
  path; no test exercises parent_id guard. Green: shell_inline_search.rs:301/334 (mount/
  routing/unmount), actions_tests.rs:211, render/tests_album_listing.rs:12,
  actions_tests_letter.rs full_library_fetch_limit_* (:231/257) + inline_search_mounts_*
  (:155), input_movie_detail_tests.rs:628 (pins needs_full_load → spawn_search_items_load
  TRIGGER decision at open, not completion), list_late_tests.rs:152, components/inline_search.rs
  unit tests. tv_wide_tests.rs:121 SearchPool::Items — 18b-writer-owned, not read
- zero-reference gate: apply_inline_search_items → zero refs post-drop (fn :245-280,
  caller shell.rs:302, doc :62). Structural gate: the flat-search completion MUST still
  project SearchPool::Items + loading=false + stale-parent skip
- do-not-touch: push_inline_search_content pool build (:99-105); pushes_inline_search
  match (:233-238 — 20c re-homes INTO it only if replacement writes all_items first);
  App-side no-op arm lib_event_actions.rs:451-456 (the write goes here or the arm is
  replaced); inline_search_component_id :17; set_inline_search_loading (:161 open-time
  loading=true — the wedge); 20d-f / 18b (in-flight TV files: tv_workspace.rs,
  shell_tv_workspace.rs, shell.rs, tasks.md — DO NOT READ)
- convention/probe → FACT + parity-preserving recommendation (not a user decision):
  a. FACT: parent_id guard is LOAD-BEARING (stale-completion skip). The push's
    mounted-guard (:70-78) is pool-agnostic (tab/mount only) — cannot detect a flat
    fetch arriving for a PREVIOUS level's parent. The event is the only carrier of the
    fetched items (App discards them). Deleting the fn without a replacement leaves the
    flat search on set_inline_search_loading(true) with partial items — a visible
    wedge. RECOMMENDATION (parity-preserving, ≤2 files): re-home the completion INTO
    the push path — write the fetched items into level.all_items under the same
    parent_id guard (defensive-parity with AllItemsPrefetched arm lib_event_actions.rs:518:
    `if last.parent_id == parent_id { last.all_items = Some(items) }`) + add
    SearchItemsLoaded to pushes_inline_search — the push's flat branch then projects
    the full set. Files: lib_event_actions.rs (write) + shell_inline_search.rs
    (drop fn, add match variant). shell.rs:298-302 arm removed. This preserves parity
    (full-library flat projection + loading=false via the push) and removes the
    direct-to-component bypass
  b. FACT: deletion is TEST-INVISIBLE — no test would catch a broken flat search.
    Implementer SHOULD add one (spawn flat search → pump SearchItemsLoaded → assert
    selected_item/all_items projected + loading cleared)
  c. reachability of the stale-parent race (D16 blocks user nav while mounted, but
    lib_rx events can still arrive) — guard is defensive either way; preserving it is
    the parity-safe choice
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (inline search
  suites + new flat-completion test); `rtk cargo clippy --workspace --all-targets`;
  `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing
  conformance failures tests_conformance_matrix.rs:227/291/339
- separation: 20c = drop apply_inline_search_items + re-home SearchItemsLoaded into the
  push path (2 production files: lib_event_actions.rs, shell_inline_search.rs; shell.rs
  arm removal) + optional 1 new test; 20d recursive pool, 20e `/` trigger, 20f mouse
  quirk — LATER rows
### REVALIDATED @3935c1ac (scouts A+B+C, 2026-08-27) — supersedes line numbers above
- HEAD verified by all three scouts: 3935c1aca322aea350c76e34679aa820b3c41013 (19c dce4389d + stale-test correction 3935c1ac on top)
- def: `apply_inline_search_items(&mut self, lib_idx: usize, parent_id: String, items: Vec<EmbyItem>)`
  shell_inline_search.rs:245-277 — guards: TabSelection::EmbyLibrary(current_idx) :251-253;
  current_idx==lib_idx && nav_stack.last().parent_id==captured :254-264; inline_search_component_id
  mounted gate :265-267; write set_content(SearchPool::Items(items), false, true) + set_loading(false)
  :273-274. THE ONLY place clearing component loading on flat completion (wedges if re-home skips it)
- sole caller: shell.rs:298-302 drain arm (`LibEvent::SearchItemsLoaded { lib_idx, parent_id, items }`
  => self.apply_inline_search_items(...)); drain opens :295 (`try_recv` loop); fallthrough arm :331
- doc-comment ref shell_inline_search.rs:62 (push_inline_search_content doc :58-67) must be rewritten
- route: variant types_events.rs:27-31; sender spawn_search_items_load library_browse_actions.rs:595-631
  (captures parent_id :601 from nav_stack.last(), sends :624-628); sole spawn caller
  shell_inline_search.rs:148 in open_inline_search needs_full_load path :143-149; NO-OP arm
  lib_event_actions.rs:451-457 (`let _ = (lib_idx, parent_id, items)`) — 20c converts to the
  AllItemsPrefetched-style guarded write (template lib_event_actions.rs:510-521: if last.parent_id==parent_id
  { last.all_items = Some(items) })
- matches! list handle_inline_search_lib_event shell_inline_search.rs:231-243: arms Refreshed :234 /
  AllItemsPrefetched :235 / AlbumIndexBuilt :236 / NavigateTo :237; then app.handle_lib_event :239;
  if pushes_inline_search { push_inline_search_content() } :240-242. SearchItemsLoaded NOT an arm — ADD
  `LibEvent::SearchItemsLoaded { .. }` here so the drain fallthrough (:331) re-projects (parity with
  AllItemsPrefetched — NOT a new explicit shell.rs arm)
- flat branch push_inline_search_content :69-116; pool derivation :95-105
  (all_items.clone().unwrap_or_else(|| level.items.clone())); set_content(pool, loading, focused) :112;
  set_loading(loading) :114 fires for recursive only — FLAT PATH DOES NOT CLEAR LOADING; loading computed
  flat=false by construction :84-87. focused = effective_panel_focus()==PanelFocus::Library (deleted fn
  passed true — equivalent while mounted, D16; note only)
- LOADING WEDGE (first-class risk, scouts A/B): component set_content is asymmetric — `if loading` at
  components/inline_search.rs:82 sets but never clears; deleted fn was the ONLY clearer via explicit
  set_loading(false) :274. Re-home MUST explicitly clear loading on flat completion + the new test must
  assert loading false
- tests (B @3935c1ac): zero real test refs to apply_inline_search_items / SearchItemsLoaded /
  spawn_search_items_load (comments only: actions_tests_letter.rs:222, library_browse_actions.rs:30/561,
  shell_inline_search.rs:62). No test pumps SearchItemsLoaded through the shell event path; closest harness
  shell_inline_search.rs:334-358 (handle_inline_search_lib_event(LibEvent::NavigateTo) + mounted() assert).
  NO component content/loading accessor exists — new test needs one (19d precedent test-only accessor
  music_workspace.rs:174): components/inline_search.rs (#[cfg(test)] or pub(super)) = possible 4th file.
  Component-level pinning: actions_tests.rs:211, render/tests_album_listing.rs:12, inline_search.rs:231/237/272
  (old cite inline_search.rs:72 stale @HEAD); input_movie_detail_tests.rs:606-636 pins needs_full_load via
  mount only, never asserts loading. Break-list on delete-without-re-home: ONLY compile error shell.rs:302
  (no test statically asserts the direct-to-component push)
- NEW TEST shape (B): extend shell_inline_search.rs test mod; (a) deliver hand-built SearchItemsLoaded with
  STALE parent_id (!= nav_stack.last().parent_id) via handle_inline_search_lib_event → assert all_items NOT
  written + loading STILL true (accessor); (b) replay with correct parent_id → all_items written + full
  items projected + loading cleared
- precedent (C): DELETION convention confirmed — f35ed7f6 deleted sync_inline_search outright (shell.rs
  -sync_inline_search(); event-scoped push_inline_search_content; apply_inline_search_items guard kept
  verbatim), fc5cefa7/5caef7a5 (20a) dropped inline_search_id; zero sync_* retained. Writer-push at drain
  sites: push_inline_search_content :153/:206/:241 + shell.rs:318/:567; sibling push_tv_workspace_content
  shell.rs:327, push_music_workspace_content (18c/19a). 20c mirrors the AllItemsPrefetched arm template
- do-not-touch @HEAD: 20d recursive Albums branch shell_inline_search.rs:89-93 (mutually exclusive with flat
  else :94-106); 20e components/browser.rs:299-301 '/' → ShellRequest::OpenInlineSearch (input-only);
  20f components/inline_search.rs:180 layout=Default in view() (painting-only); set_inline_search_loading
  :209-218 + open caller :161; spawn_search_items_load/spawn_all_items_prefetch library_browse_actions.rs:550/595;
  AllItemsPrefetched arm lib_event_actions.rs:510-521; RecursiveAlbumActivated/RestoreLibraryPosition arms;
  recursive branch :88-94; activate/dismiss; enum variant types_events.rs:27-31 STAYS (carrier)
- zero-ref gate: apply_inline_search_items → zero refs (code def :245-277 + doc :62 rewritten); no-op
  `let _ = (…)` gone from lib_event_actions; shell.rs:298-302 arm REMOVED (SearchItemsLoaded flows via
  fallthrough :331 + matches! arm). Legal SearchItemsLoaded refs post: types_events.rs:27,
  library_browse_actions.rs:624, lib_event_actions.rs:451 (write arm), shell_inline_search.rs matches! arm
- visibility: BrowseLevel.all_items pub(super) types_browse.rs:45 — reachable from lib_event_actions (sibling
  crate::app file); Model.application pub(super) shell.rs:55 — component push MUST stay on the SHELL side
  (matches! push); lib_event_actions (impl App) cannot touch Model.application
- expected scope: 3 core production files (lib_event_actions.rs, shell_inline_search.rs, shell.rs) + the new
  event-path/stale-parent test; possible 4th file components/inline_search.rs for the test-only accessor (B)
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (actions_tests.rs,
  actions_tests_letter.rs inline_search_mounts_*, render/tests_album_listing.rs, input_movie_detail_tests.rs,
  list_late_tests.rs, components/inline_search.rs units + NEW event-path test);
  `rtk cargo clippy --workspace --all-targets`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing
  conformance failures tests_conformance_matrix.rs:227/291/339
- final-verified HEAD: 3935c1aca322aea350c76e34679aa820b3c41013 (scouts A+B+C, all report same HEAD)

## 5.3d.19c

- seam symbol(s): legacy wide-music branch list.rs:85-102 (DELETE), `render_list`
  (branch host), `level.scroll` write-back :96-98 (orphaned), legacy `paint_music_image`
  :99 (dead double-paint); geometry publishers (KEEP) widgets.rs:545-549 pre-pass +
  shell_music_workspace.rs:143 re-publish
- definition (verified at HEAD bbf97516):
  - branch list.rs:85-102 — gate `is_music_group_view && is_viewing_album_folders &&
    shared_hero_presentation(area).is_some()` (≥82-col); ctx :93 →
    render_wide_music_group_with_ctx :95 → `level.scroll = output.final_scroll` :96-98 →
    paint_music_image :99 → early-return :100. NO fetch block (19d removed it)
  - double-paint VERDICT: the branch STILL RUNS as dead paint when the component is
    mounted — render_library (widgets.rs:536-550) pre-pass publishes geometry
    (:545-549 publish_geometry, identical gate) then UNCONDITIONALLY render_list :550;
    shell then renders the component (shell.rs:1137 → shell_music_workspace.rs:126-150,
    area = layout.main.wide_music_area, re-publish_geometry :143, take_image_paint →
    paint_music_image :147-150). Same frame, same area. Component mounted exactly when
    branch gate holds (music_workspace_component_id shell_music_workspace.rs:31-47
    identical predicate) — branch + component can never be simultaneously absent
  - scroll: component owns it (album_scroll music_workspace.rs:26, seeded from pushed
    ctx on first mount :88, kept while unchanged :99-100, written per-frame in view
    :403). Branch write-back :96-98 writes from the LEGACY ctx's output — orphaned;
    deleting loses NO visible scroll (App level.scroll becomes write-stale, not
    read-dead; component album_scroll authoritative)
  - image: paint_music_image album_art.rs:69-85 (sync, respects images_enabled/area
    guards :91-93); shell ALSO calls it :147-150 — legacy :99 is redundant (paints
    same art a moment before the component repaints)
- callers / gates: branch is the ONLY list.rs wide-music paint path; input routing
  reads geometry from the PRE-PASS (widgets.rs:545-549, music_wide.rs:79-96 — NOT the
  branch): is_wide_music_active layout.rs:184-189 (wide_music_right_area),
  current_library_columns lib_cursor_actions.rs:69-89 (left_area.width),
  lib_page_size actions.rs:117-123 (left_area.height), shell_library.rs:51-57 +
  actions_navigation.rs:188 (is_wide_music_active). Geometry published 3×/frame today
  (pre-pass → legacy render → shell render); after 19c = 2 publishers, both intact.
  fetch_album_tracks callers at HEAD: shell_music_workspace.rs:100 (push) +
  selection_modal_actions.rs:118 + album_plan.rs:268/299/391 (narrow) — none in list.rs
- tests: NO test asserts the legacy branch output (render tests seed cache + use the
  pre-pass/direct geometry; music_characterization + shell_music_workspace tests read
  layout.main which the pre-pass + shell re-publish; conformance-matrix Music rows
  (270/312/419/472) run legacy render_library — 19c removes ONLY the wide branch, the
  narrow renderer stays, so they stay green; 3 pre-existing conformance failures
  :227/291/339 are baseline). post-19d test push_music_workspace_fetches_selected_
  album_tracks (shell_music_workspace.rs) pins the re-homed fetch — unaffected
- zero-reference gate: the branch's code (list.rs:85-102) → deleted; narrow-path
  list.rs refs (music_hero_placeholder :264, show_music_pills :299, show_grouped :398,
  is_music_group_view empty-state :436) STAY; helper fns is_music_group_view/
  is_viewing_album_folders STAY (used by shell + pre-pass + narrow); publish_geometry
  + render_wide_music_group_with_ctx STAY (pre-pass + component + shell re-publish)
- do-not-touch: widgets.rs:545-549 pre-pass + render_list call :550; shell
  render_music_workspace_component (:126-150) + its publish/image; music_workspace.rs
  album_scroll + view; narrow path (list.rs :250-279/:296-302/:395-402/:433-440/
  :575-583); TV branch list.rs:132 (18b-owned, outside scope); fetch_album_tracks
  push-path trigger; 19e teardown (sync_music_workspace + differential test) LATER
- convention/probe → FACT + parity-preserving recommendation (not a user decision):
  a. FACT: 19c is a CLEAN whole-branch deletion — fetch re-homed (19d), geometry
    pre-pass-published (19b, widgets.rs:545-549) + shell re-published
    (shell_music_workspace.rs:143), scroll + image component-owned. NOTHING from the
    branch must be preserved/relocated (scroll write-back orphaned, legacy paint dead).
    Deleting also removes one of 3 redundant publish_geometry invocations/frame
  b. 19c does NOT touch 19e's teardown targets (sync_music_workspace adapter +
    differential test shell_music_workspace.rs:151-231 grouped_music_cursor_routing_
    matches_legacy_after_each_key) — that's 19e
  c. one benign seam to note: App level.scroll stays write-stale for wide Music after
    19c (never consulted for wide rendering/input; component album_scroll authoritative)
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (music
  characterization + shell_music_workspace + conformance Music rows + push fetch test);
  `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`;
  `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures
- separation: 19c = delete list.rs:85-102 only (1 production file) + no test changes
  expected; 19e teardown (sync adapter + differential test) LATER; 18b landed
  (bbf97516) — TV branch list.rs:132 untouched by 19c
- last-verified HEAD: bbf97516025ae3fcf8d478dc1232d7cad04572d5 (scout; 18b landed —
  list.rs clean; shell.rs committed, safe)

## 5.3d.18c (CORRECTION PATH for 18b P1s)

- seam symbol(s): `TvWorkspaceComponent` cursor (index-only today → must expose the
  SELECTED ITEM id), `TvWideRenderCtx.selected_series` (currently stale App item),
  `Model::sync_tv_workspace` (per-frame content mirror → split into mount-only +
  writer push), `push_tv_workspace_content` (NEW, Music analog), TvMoveRows/TvJumpCursor
  (may gain target field), TvActivate/TvClick/TvScroll source
- definition (verified at HEAD bbf97516; scouts A+B+C):
  - component holds `cursor: usize` ONLY (no item id; tv_workspace.rs field, cursor()
    getter :100-102); App nav_stack cursor is the SOLE item source today:
    selected_series_item (render/components/detail.rs:137-160, ctx.items[ctx.cursor]),
    library_list_render_ctx (list_context.rs:5-28, nav_stack.last() cursor),
    sync_tv_workspace ctx build (shell_tv_workspace.rs:68-84), enter_series_selection
    (lib_cursor_actions.rs:317-324 → fetch_series_detail(item.id))
  - P1 mechanism: 18b froze the App cursor (handle_tv_request no-ops TvMoveRows/
    TvMoveColumn/TvJumpCursor, shell_tv_workspace.rs:15-19) but actions still resolve
    via it: TvActivate → activate_selected_series (input_browse_dispatch.rs:162-173 →
    selected_series_item stale); mouse TvClick → handle_mouse_single/double_click_tv
    WRITE App cursor from hit but component cursor never adopts (tv_workspace.rs:363
    sets pane only); TvScroll → handle_mouse_scroll_browse → move_lib_cursor (App
    cursor only, component untouched); sync_tv_workspace (per-frame, shell.rs:1079)
    re-projects selected_series/series_detail from stale App cursor → series_changed
    reset wipes local pane state (P1 #4)
  - Music precedent (A): push_music_workspace_content shell_music_workspace.rs:79-124 —
    id guard → tab gate → ctx via wide_music_render_ctx → get_component_mut →
    set_content; writers: shell.rs 272/286/344/417/428/552/607/619/627/638/1128 +
    mount :63. MusicAlbumCursor carries {target,kind} (shell.rs:607 arm) — shell writes
    App from the TARGET, component holds cursor between keys (parity test
    grouped_music_cursor_routing_matches_legacy_after_each_key :234)
  - TV mount truth: tv_workspace_component_id shell_tv_workspace.rs:31-45 gates
    collection_type=="tvshows" && is_wide_tv_active (layout.tv_wide_right_area>0);
    mounts ONLY in wide (unlike Music); sync_tv_workspace :47-90 is the mount reconciler
    + per-frame content mirror (shell.rs:1079); render_tv_workspace_component :92-101
    paints into tv_wide_area
  - wide_tv_render_ctx (tv_wide.rs:75-97) duplicates the sync_tv_workspace content
    build (list→selected_item→series_detail_cache→TvWideRenderCtx::new)
- callers / writers (A): no push_tv_workspace_content exists (zero hits). TV-affecting
  writers LACKING a re-project today: mouse TvClick (shell.rs:1002-1026 →
  mouse_gestures.rs:209-246 writes level.cursor), TvActivate (shell_tv_workspace.rs:20),
  TvCycleLetterPill (→cycle_letter_pill writes letter_filter), TvBack (→go_back writes
  nav_stack), TvScroll (shell.rs:993-996 → move_lib_cursor), SeriesDetailFetched
  (lib_rx, series_modal_actions.rs:5-26 writes series_detail_cache), Resize (NO TV
  analog of music_resize today — shell.rs:563-565/1128 Music deferral pattern is the
  precedent). Pure component-local NO-push writers: TvMoveRows/TvMoveColumn/TvJumpCursor/
  TvEpisodeMove/TvSeasonMove (component cursor stays authoritative)
- tests (C): MUST FLIP: typed_tv_requests_keep_component_cursor_authoritative
  (shell_tv_workspace.rs:113-162 — asserts App cursor stays 0 post-TvMoveRows/TvJumpCursor;
  becomes the TV parity-test analog, asserts App == component cursor post-writer).
  MUST ADD (sync/push seam ZERO test coverage today): push_tv_workspace_content test
  (analog push_music_workspace_fetches_selected_album_tracks :183 — tvshows app + wide
  rects + sync → assert mount projection + fetch) + parity test (analog
  grouped_music_cursor_routing_matches_legacy_after_each_key :234) + optional
  TvActivate source test (untested today). STAY GREEN: tv_workspace_component_tests.rs
  (13/69/100/125/161 — only compile-level if TvMoveRows gains a target field),
  tv_wide_tests.rs (65/80/97/116/138 — legacy render-path via wide_tv_render_ctx,
  untouched), tv_workspace.rs in-module (501/535/577), all Music tests. Untested
  either way: mouse App-cursor write + TvScroll (no pins)
- zero-reference/structural gate: push_tv_workspace_content NEW; sync_tv_workspace
  becomes mount-only (push folded in or sibling); component must expose the selected
  item id (resolve via context.list.items[self.cursor]) so activation/detail/mouse
  resolve the COMPONENT's selection; first-mount still seeds cursor from App
  (initialized branch tv_workspace.rs:74-86 — same shape as Music)
- do-not-touch: 18b's authority decision (component cursor authoritative — no App
  mirror re-introduction INTO the component); series_detail_cache reader (B2) stays at
  the push ctx build; tv_wide_render_ctx (legacy render-path, tests pin it);
  render_tv_workspace_component + tv_wide_area geometry; 18d (geometry/underpaint),
  18e (teardown sync_tv_workspace), 18f (episode play/enqueue) LATER
- convention/probe → FACT + parity-preserving recommendation (not a user decision):
  a. FOLD-vs-ROLLBACK: 18c is the minimal CORRECTION, not a rollback — the 4 P1s are
    ALL fixed by making the component the selected-item source + a TV writer push that
    mirrors component.cursor() (or carries the target, Music-style) into App at
    writers. The 18b authority change (component cursor authoritative) is CORRECT; it
    was incomplete without the writer push. Rollback would re-introduce the mirror
    ills 18b removed. RECOMMEND: fold 18c as the inseparable correction (one commit
    on top of 18b, or amend if not yet accepted)
  b. Smallest files/symbols: tv_workspace.rs (expose selected item id + maybe cursor
    getter), shell_tv_workspace.rs (push_tv_workspace_content + handle_tv_request
    writers + sync becomes mount-only), shell.rs (wire the push at the writers:
    TvClick/TvActivate/TvCycleLetterPill/TvBack/TvScroll/lib_rx/resize) —
    ≤3 production + tests. If TvMoveRows gains a target field: msg.rs (enum) +
    component emit + the typed-request test
  c. Invariants (no visible change): component cursor authoritative for its own
    interaction; App cursor mirrors the component cursor AFTER each writer (key/mouse/
    wheel/letter/back/resize); activation + detail fetch + mouse resolve the
    component's selected item; series_changed reset only on a genuine series change;
    first-mount seeds from App cursor
  d. Mouse/wheel correctness: TvClick writes App cursor from hit (already does) — the
    fix is the component ALSO adopts the clicked row (or the writer mirrors it back);
    TvScroll → the writer must re-project after move_lib_cursor (or the component
    receives the scroll); either way the push at the writer re-syncs both
  e. Activation source: TvActivate resolves the component's selected item id (not
    selected_series_item from App cursor) — the component exposes it; the shell
    passes it to activate_selected_series (which takes an item or resolves via the
    component cursor)
  f. Push ordering: push at the writer AFTER the App mutation (cursor write/letter
    pill/nav back/detail-fetch completion), so the component sees the fresh content;
    resize needs the Music music_resize deferral (current-frame layout)
  g. Task recording: 18b accepted as-is (authority change is correct); 18c lands as
    the correction commit; tasks.md marks 18b done + 18c as the writer-push row (its
    original scope) — the row text's writer set + series_detail_cache reader (B2)
    already describe it
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (rewrite
  typed_tv_requests_keep_component_cursor_authoritative → parity; add push_tv +
  parity tests; tv component/wide/workspace + Music suites stay green);
  `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`;
  `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures
  tests_conformance_matrix.rs:227/291/339
- separation: 18c = component exposes selected item + TV writer push + App-cursor
  mirror at writers (≤3 production + ≤3 tests); 18d geometry/underpaint, 18e
  teardown (mount-only sync → deleted), 18f episode play/enqueue — LATER
- last-verified HEAD: bbf97516025ae3fcf8d478dc1232d7cad04572d5 (scouts A+B+C; 18b
  landed; no newer commits — re-verify shell.rs:1079/993-1026 + tv_workspace.rs lines
  on handoff if HEAD advances)

## 5.3d.20e (RE-VERIFIED @51323c41)
- seam symbol(s): `BrowserComponent::handle_crossterm_key` '/' arm (browser.rs:297-300),
  `ShellRequest::OpenInlineSearch` variant (msg.rs:258), sole consumer shell.rs:705 →
  `open_inline_search` → shell_inline_search.rs:131
- definition (verified at HEAD 51323c41b3f5aeacc9535e118dd0b83cba9d03c0; 20c 96bde782 +
  20d 51323c41 committed on top — shell files readable, NOT in-flight):
  - `browser.rs:297-300` — '/' arm inside `pub(in crate::app) fn handle_crossterm_key`,
    guard `if key.modifiers.is_empty()`:
    `return Some(Msg::Shell(super::msg::ShellRequest::OpenInlineSearch));` (browser.rs:300)
  - **ROW TEXT IS STALE**: task cites browser.rs:90-92; :90-92 is inside `set_content`
    (cursor/scroll sync, browser.rs:85-96), NOT the key handler. Real '/' emitter = :297-300
    (was 299-301 at 3935c1ac — 2-line cosmetic shift, no semantic change).
  - **PRODUCTION ALREADY COMPLETE**: the '/' branch emits the typed `Msg::Shell(ShellRequest::
    OpenInlineSearch)` — TuiRealm request channel, NO legacy/`Msg::Legacy` fork, NO raw-key
    passthrough. Re-host DONE; nothing to build/remove. Request body `()`.
  - Variant **`OpenInlineSearch`** msg.rs:258, enum `ShellRequest` msg.rs:194; sibling
    `InlineSearchActivate{id,item_type}` msg.rs:253 + `InlineSearchDismiss` msg.rs:260
    (NOT owned by 20e). `Msg::Shell(ShellRequest)` wrapper msg.rs:22. Sole consumer
    shell.rs:705 `Msg::Shell(ShellRequest::OpenInlineSearch) => self.open_inline_search()`;
    target fn open_inline_search shell_inline_search.rs:131. Consumer line UNCHANGED
    through 20c/20d (drift-clean).
  - **variant-name check (scout C)**: `ShellRequest::OpenSearchRequest`/`OpenIntSearchRequest`/
    `ShellSearch::OpenSearch` DO NOT EXIST in src/ (grep zero); surface is exactly
    `ShellRequest::OpenInlineSearch`. Prior index already used the correct name.
- callers: production emitter = browser.rs:300 (ONLY — full-literal grep `OpenInlineSearch` =
  exactly 3: browser.rs:300 emitter, shell.rs:705 consumer, msg.rs:258 decl). Consumer =
  shell.rs:705 (committed 20c boundary). 20c/20d did NOT touch components/browser.rs
  (git show --stat for 96bde782/fd1b3517/51323c41: browser.rs absent). tests: ZERO test
  drives '/'→OpenInlineSearch or asserts it; browser_component_tests.rs (tests :22/:136/:198/
  :245/:286/:304) drives cursor/mouse only — no `Char('/')`; inline-search-mount assertions
  via DIRECT model.open_inline_search() (NOT the '/' key): actions_tests_letter.rs:153,
  input_movie_detail_tests.rs:615/:628, render/components/list_late_tests.rs:149;
  render/tests_album_listing.rs:8 drives the component directly (not shell, not key).
- zero-ref gate: browser.rs:300 must STAY the only production emitter of
  ShellRequest::OpenInlineSearch (no second emitter, no re-introduced legacy '/' fork);
  shell.rs:705 arm stays as consumer. `Char('/')` in src = browser.rs:299-300 only
  (production) + input.rs Ctrl-edge (unrelated) + test files.
- do-not-touch (ALL now COMMITTED by 20c/20d — no longer in-flight, still must-not-touch):
  shell_inline_search.rs, lib_event_actions.rs, shell.rs, components/inline_search.rs;
  msg.rs:258 OpenInlineSearch variant + enum msg.rs:194; InlineSearchDismiss/Activate siblings
  (msg.rs:253/260); inline_search.rs view (20f boundary — NOT edited by 20c/20d);
  20f mouse left_area quirk LATER.
- convention/precedent → FACT + parity recommendation (standing correction, not a decision):
  a. Sibling convention confirmed (sessions.rs:84-85, help.rs:70-72): a KEY opening a whole-shell
     surface emits typed `Msg::Shell(ShellRequest::<variant>)` from the component. Browser '/' arm
     follows it.
  b. FACT: row literal WRONG ("re-host", :90-92). Re-host DONE at :297-300 and already emits
     OpenInlineSearch. 20e = NO-OP for production (parity-preserving), like 20b. If the ledger
     row 20e is to be marked [x] on the NO-OP basis, that is the orchestrator's reviewer call —
     report-not-do. Do NOT invent a legacy removal or a new test requirement.
  c. Focus-gating: '/' arm fires BEFORE `if self.focused` — effective focused OR unfocused
     (differs from movement keys). No test pins it; do not gate unless parent directs.
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (browser_component_tests
  + inline-search suites stay green with NO source change); `rtk cargo clippy --workspace --all-targets`;
  `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures
  tests_conformance_matrix.rs:227/291/339.
- separation: 20e NO-OP (production correct) + OPTIONAL new component test driving `Char('/')`
  → Some(Msg::Shell(ShellRequest::OpenInlineSearch)) + cursor untouched (slot: browser_component_tests
  expected_movement_request pattern). Receiving/writing side (shell.rs:705, open_inline_search
  shell_inline_search.rs:131) is committed shared boundary, NOT this row. 20f mouse left_area
  quirk LATER.
- last-verified HEAD: 51323c41b3f5aeacc9535e118dd0b83cba9d03c0 (scouts A+B+C all report;
  observed-head, no source edits by this team)
## 5.3d.20d
- seam symbol(s): `push_inline_search_content` recursive-Albums pool branch (shell_inline_search.rs:112-117)
  + recursive `loading` arm (:98-103) + `let recursive` (:97) / `library_id` locals; `SearchPool::Albums`
  production moved/removed
- definition (verified at HEAD fd1b351793f75c92930f7cc4d240dbe9640ae9cd; committed 20c 96bde782 on top:
  shell_inline_search.rs readable, NOT in-flight):
  - `push_inline_search_content` pub(super) shell_inline_search.rs:84; `let recursive =
    self.app.recursive_album_search_enabled(index);` :97; `if recursive` split into loading arms
    (:98-103 recursive / :104-110 flat) and pool arms (:112-117 recursive Albums / :118-127 flat Items)
  - recursive pool quoted (scout A): `match self.app.album_indexes.get(&library_id) {
    Some(AlbumIndexState::Ready(entries)) => SearchPool::Albums(entries.clone()),
    _ => SearchPool::Albums(Vec::new()) }`
  - `recursive` NOT a fn param — derived per-call from App state `recursive_album_search_enabled` ⇐
    `recursive_album_search_eligible` library_browse_actions.rs:23-27 (collection_type=="music" &&
    music_levels.len()>1 && last=="album"); `library_id` local :97-99 used ONLY by the recursive arms
  - recursive==true IS reachable today (music lib + album-level config drives it at open
    open_inline_search:164-170 + start_album_index:35-36)
  - NO helper fn inside shell_inline_search.rs orpans on the drop; `recursive`/`library_id` become
    write-unused locals → drop too; App fields album_indexes/music_levels stay written elsewhere
    (library_search_actions.rs, lib_event_actions.rs, construct.rs) and remain reachable
- callers (all push take NO recursive arg; recursive derived per-call): open_inline_search
  shell_inline_search.rs:176, activate_inline_search_item :229, handle_inline_search_lib_event :265,
  shell.rs:313 (RestoreLibraryPosition arm), shell.rs:562 (resize). All five stay → flat path still hit
- tests (B): ZERO shell-level test drives the recursive push branch. Component-level only:
  actions_tests.rs:177 `recursive_album_search_matches_ancestor_labels` and :227
  `album_index_completion_updates_the_open_current_query` construct SearchPool::Albums DIRECTLY via
  set_content — they break ONLY if 20d deletes the SearchPool::Albums variant/component arm (it does NOT);
  they stay green. actions_tests.rs:263/:284 (album_indexes state, not inline-search) stay. Flat-path
  survivors to keep green: shell_inline_search.rs:294/:323/:361, components/inline_search.rs:243/:262/:283,
  actions_tests.rs:207, render/tests_album_listing.rs:10, mount trio actions_tests_letter.rs:153 /
  list_late_tests.rs:149 / input_movie_detail_tests.rs:628, tv_wide_tests.rs:121 (Items)
- zero-reference gate: SearchPool::Albums CONSTRUCTION inside push_inline_search_content (:112-121) → zero;
  `recursive`/`library_id` locals (:97-99) → zero if flat arms no longer ref. MUST STAY: flat
  SearchPool::Items path (:104-110,:118-127) + its 5 callers; SearchPool::Albums variant +
  filtered_items Albums arm (components/inline_search.rs) + actions_tests direct construction
- do-not-touch: library_search_actions.rs start_album_index/activate_recursive_album/
  recursive_album_search_enabled; library_browse_actions.rs recursive_album_search_eligible/
  build_album_index_with/fetch_all_album_index_items; lib_event_actions.rs:470-494 album_indexes writes;
  components/inline_search.rs SearchPool enum + filtered_items Albums arm + view; render/components/list.rs:438
  render_album recursive branch (other row); msg.rs shared ShellRequest variants InlineSearchActivate/
  OpenInlineSearch/InlineSearchDismiss; types_events.rs shared LibEvent variants SearchItemsLoaded/
  AlbumIndexBuilt/RecursiveAlbumsActivated/NavigateTo; types_browse.rs BrowseLevel/all_items;
  browser.rs:299-301 (20e) + components/inline_search.rs view (20f); shell.rs:313/:562 callers remain
- precedent (C): 20c (commit 96bde782) dropped apply_inline_search_items + re-homed SearchItemsLoaded
  into all_items write, component reader SearchPool::Items UNCHANGED; 20d is the inverse shape (REMOVE a
  branch inside one file, not a 4-file write-relocation) — precedent confirms read-side surviving untouched
- convention/probe → boundary finding (NOT resolved — parent decides): whether 20d is ONLY the projection
  branch drop (literal row, 1 file) vs. ALSO retiring the recursive start_album_index spawn (open 164-170) /
  activate_recursive_album arm (209-223) — the latter reaches library_search_actions.rs (>2 files). Scout C:
  App retains album index until group 5 (file header) → keep start_album_index, but then it runs a
  potentially expensive full build whose result is no longer projected at push — dead-cost design q for parent
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (shell_inline_search tests
  :294/:323/:361 + actions_tests.rs Albums + components/inline_search + render/tests_album_listing +
  actions_tests_letter/list_late_tests/input_movie_detail); `rtk cargo clippy --workspace --all-targets`;
  `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures
  tests_conformance_matrix.rs:227/291/339
- separation: 20d = remove recursive pool + loading arms, collapse push to flat-only (1 file: shell_inline_search.rs,
  ≤2 satisfied) + optional recursive_music_app mount regression test; Do NOT delete SearchPool::Albums variant/
  arm (later enum cleanup >2 files, or 20f); 20e '/' re-host (browser.rs) + 20f mouse left_area quirk LATER.
  last-verified HEAD: fd1b351793f75c92930f7cc4d240dbe9640ae9cd (scouts A+B+C; my prompt had
  5d1b… for B/C, scouts reported actual fd1b…; orchestrator's fd1b3517… is correct)

## 5.3d.20f
- seam symbol: `InlineSearchComponent.layout` (LayoutMain) field inline_search.rs:65, `LayoutMain.left_area: Rect` layout.rs:81, mouse hit-target inline_search.rs:178-180 (handle_mouse), and the "Default-layout" reset `self.layout = Default::default()` inline_search.rs:195
- definition (verified at HEAD d27eeaaddd89a53fc82e848754dedd6f457a2313, = ledger-only tasks.md commit on top of SOURCE HEAD 51323c41; source identical. Scouts A+B+C report this discrepancy correctly; source to re-verify on handoff):
  - `InlineSearchComponent::new` inline_search.rs:69-79, `layout: Default::default()` :77 (zero Rect at construction)
  - `fn view(&mut self, frame, area: Rect)` inline_search.rs:194-217 — :195 `self.layout = Default::default();` THEN :200-204/:206-215 `render_generic_movies_home_video_rows_with_ctx(frame, area, ..., &mut self.layout)` which re-publishes `layout.left_area = list_area` (= the passed real `area`) — via render/components/list.rs:20-22 (left_area written BEFORE the empty-items early-return :23-24)
  - `fn handle_mouse` inline_search.rs:171-186 — :177 pos=Position, :178 `if self.layout.left_area.contains(position)`, :179 `row = position.y.saturating_sub(left_area.y)`, :180 `move_cursor(row,0,len)`. only Down(Left). No left-vs-right cell (single-column list)
  - **KEY FINDING (scouts A+B+C unanimous): the "Default-layout mismatch" premise does NOT hold at HEAD steady-state** — `view()` resets to Default THEN immediately re-publishes `left_area` from the real per-frame area via the renderer. The ONLY residual quirk = the pre-first-`view()` window (mount→first paint, field still zero from :77) where a mouse click misses; and the pattern trusts a stored field (`self.layout.left_area`) rather than computing hit-test directly from the passed `area`. NOT an always-on bug; the driving repro (reported mis-click) was NOT reproduced.
- callers / readers:
  - Component-local read inline_search.rs:178-179 (`self.layout.left_area`) — private, no external reader of the component's layout
  - the AREA fed to view derives via `inline_search_area()` shell_inline_search.rs:46-57 (falls back main.left_area → tv_wide_right_area → movies_wide_right_area → wide_music_browser_area); `render_inline_search_component` shell_inline_search.rs:254-267 (`area = inline_search_area()` :256; `if area.width==0||area.height==0 { return; }` :261-263; `application.view(&id, frame, area)` :264). Shell ALREADY feeds correct per-frame area — NOT to be touched
- tests: ZERO test drives a mouse event into InlineSearchComponent or asserts `left_area`/hit-target/geometry. In-file `inline_search.rs` tests :242/:261/:282 (keyboard/render only: query/cursor, glyph "O", Enter→InlineSearchActivate). Shell-level :279/:308/:346 (mount/routing/unmount/pool). render/components/tv_wide_tests.rs:119, input_movie_detail_tests.rs:628, actions_tests_letter.rs:153, render/components/list_late_tests.rs:149 — none mouse-driven. Mouse-event test files touch OTHER components (feeds/playlists/queue/selection_modal/tv_workspace/home/browser) — none reach inline search. NO break list: fixing the quirk is zero-risk to the current suite (no test pins degenerate OR nonzero geometry)
- zero-reference/structural gate: `layout.left_area` must STAY the only inline-search mouse-hit region; component's `view()` Default-reset :195 STAYS (normalizes field before renderer re-populates). `left_area` is NOT a zero-ref-deletion row — it's a geometry-correctness seam. Gate = if fixed, no test asserts old or new geometry → add a new mouse-hit test
- do-not-touch: 20e (`components/browser.rs` '/' arm, msg.rs:253/258/260 variants), 20d (recursive pool branch, dropped), SearchPool enum + filtered_items Albums arm inline_search.rs:31-56 (shared), shell.rs dispatch arms + shell_inline_search.rs push_inline_search_content + render_inline_search_component -- committed-correct (20c) — committed-correct (20c); shell_inline_search.rs:46-57 inline_search_area + :254-267 render_inline_search_component; msg.rs + types (no change needed for the mouse quirk)
- convention/precedent → the "sane" sibling convention (scout C + A) that the fix should mirror: siblings recompute the hit area from the per-frame passed area INTO their layout, then hit-test against it — inline_search ALREADY does this (list.rs re-publishes). Examples: browser.rs handle_mouse rebuilds self.layout every view (:191-193 pane→left_area, :682/:698 hit test); feeds.rs:291-299/:316-318 (recompute per frame); tv_workspace.rs:392-394 / hit_test :431-437 (rebuilt every view); home.rs:541-547 list_area from render each frame. So the "fix" is likely a pure component-local tweak (use the passed `area` directly, or reset the zero-window), NOT a shell change
- honest scope: 1 production file (components/inline_search.rs) + optional test file = ≤2 satisfied. shell feeds area correctly; no shared types/msg/shell change. NO shell-side work.
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (inline_search component 3 tests + shell_inline_search 3 + tv_wide:119 + list_late:149 + input_movie_detail:628 — all stay green; NEW mouse-hit test optional); `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures tests_conformance_matrix.rs:227/291/339
- separation: 20f = confirm the quirk repro / fix `left_area` handling (either honor passed area in handle_mouse, or ensure nonzero area pre-first-view) — 1 file + test; do NOT touch shell_inline_search.rs / list.rs geometry-publish / 20e/20d; options for parent: (i) NO-OP if the premise doesn't repro under the shell render seam, or (ii) MINIMAL fix. The scouts FLAG the driving symptom was NOT reproduced — recommend the orchestrator confirm the exact repro (first-frame click, or a zero-area-gate frame) before a diff.
- last-verified HEAD: d27eeaaddd89a53fc82e848754dedd6f457a2313 = ledger-only tasks.md commit; source identical to accepted SOURCE HEAD 51323c41 (verified via git diff --name-only 51323c41 d27eeaad = tasks.md only). Re-verify / inline_search.rs line numbers at accepted source HEAD on assignment.

## Campaign queue map @46328a8c (derived from scout reports A/B on tasks.md; ledger-only, no source)
- verified HEAD: 46328a8c263c4a111855a9e377e8fcb0c120c279 (worktree clean of source; index my only write)
- surface aggregate child state:
  - 5.3d.11 (ABS podcast): U0-U5 [x], **U6 [ ] open = FIRST actionable leaf** (green only after U0-U5; "First open executable checkbox" tasks.md:220; 18a lists U6 as its campaign predecessor). Aggregate stays open until U6.
  - 5.3d.18 (TV): 18a/b/c [x], **18d [ ]** (next; geometry/underpaint), 18e [ ] (teardown), 18f [ ] (episode play/enqueue). Safe order 18c→18e already honored.
  - 5.3d.19 (Music): 19a/b/c/d [x], **19e [ ]** (framework teardown: remove sync_music_workspace adapter + differential test). Order 19d→19c honored.
  - 5.3d.20 (inline search): 20a-20f ALL [x] (20e/20f were verified NO-OPs) → **aggregate = ledger-only flip candidate** (top-level 5.3d.20 box tasks.md:310).
- classification: 11=(b), 18=(b), 19=(b), 20=(a)
- earliest actionable: **U6 (5.3d.11)** — deps U0-U5 all [x]; aggregate deps 5.3a/b/c + 4.1/4.10 are campaign-level (satisfied). Strictly earlier than 18d/19e per ledger's stated predecessor chain.
- global gates (all [ ], depend on all surface rows): 5.3d aggregate, 5.3d.21 (re-inventory), 5.3d.22 (delete CONTEXT_STACK per-surface), 5.3d.23 (delete LegacyInput/msg/event/adapters), 5.3d.24 (verify no mirror/legacy-paint/router). None executable now; tail order 5.3d→21→22→23→24→5.5→5.6.
- 5.5: ledger flip (no legacy AND no component surface rows) — end-campaign.
- 5.6: final gate (cargo check/nextest/clippy/ast-grep + final-only `rtk make check-code-file-lines`) — end-campaign.
- ledger-only no-ops already recorded this campaign slice: 20b, 20e, 20f (all [x] with "no source change"/"verified no-op" notes).
- next recommended bounded unit (PER SCOUT REPORT, not a manager verdict to implement): the 5.3d.11-U6 row (ABS podcast Book-style split; 3 files: shell.rs, shell_audiobookshelf_podcast.rs, types_audiobookshelf_browse.rs). U6 index entry already exists at top of this file (## 5.3d.11-U6).
- separation: 18d/18e/18f (TV) and 19e (Music teardown) are later surface units; 5.3d.13 ABS-Book report-only gate open; global gates are the campaign tail, not now.


- **U6 RECONCILIATION 2026-08-27 (scouts A+B, git ancestry + ledger):** commit 5ca1b099
  'fix(5.3d.11 U6): retain ABS podcast sync as mount-only, split projection to push-content'
  IS the U6 implementation and IS live at HEAD 46328a8c (ancestor, un-reverted, exact U6 file set:
  shell.rs, shell_audiobookshelf_podcast.rs, types_audiobookshelf_browse.rs + tests_podcast.rs +
  tasks.md; the two core shell_audiobookshelf_podcast.rs / types_audiobookshelf_browse.rs are
  BYTE-IDENTICAL to 5ca1b099 at HEAD +84/-41; push/sync split intact at HEAD). NO reassignment
  to 'next actionable' until ledger corrected.
  - CONTRADICTION CONFIRMED: tasks.md header (:32/:33/:37) + U6 row annotation (:220) still call U6 the
    'first open executable checkbox', yet the row glyph renders [x] and U6 carries NO 'Commits: 5ca1b099'
    line (U0-U5 all carry one). tasks.md never references 5ca1b099.
  - index ## 5.3d.11-U6 entry (:114) already marks U6 landed at 5ca1b099 (last-verified HEAD).
  - SAFE DISPOSITION (fact-based, orchestrator decides): U6 is IMPLEMENTED; the gap is LEDGER BOOKKEEPING
    only. Recommend a ledger-only correction (record 'Commits: 5ca1b099' + align the :220 annotation / header
    :32) rather than any re-implementation. After correction the next actionable ACTIVE leaf is 5.3d.18d
    (TV geometry/underpaint) then 5.3d.19e (Music teardown).
  - do-not-resolve: source-of-truth (tasks.md vs index) is the orchestrator's call.

## 5.3d.18d
- seam symbol(s): legacy wide-TV branch render/components/list.rs:103-114 (DELETE target),
  `render_wide_tv_with_ctx` tv_wide.rs:102 (writes tv_wide_* into any &mut LayoutMain — the ONLY publisher),
  component `TvWorkspaceComponent::view` tv_workspace.rs:467-471 (resets+rebuilds its OWN self.layout + hit-tests),
  NEW `layout_main` geometry pre-pass (widgets.rs ancestor of Music publish_geometry)
- definition (verified at HEAD 564dd1ef4abc3dce261893719bc7646b6398a558; scouts A+B+C):
  - `render_wide_tv_with_ctx` tv_wide.rs:102 — writes tv_wide_area :110, tv_wide_left_area :119,
    tv_wide_right_area :120 (from wide_library_panes(area,PANE_PAD_X,Y) :113-120),
    tv_wide_list_area :173, tv_wide_season_tabs :243, tv_wide_episode_rows :313. Called from TWO sites:
    (a) LEGACY render_list branch list.rs:103-114 (gate is_wide_tv_library||is_podcast_library &&
    shared_hero_presentation is_some, i.e. ≥82-col) — paints FULL underpaint into App layout + `level.scroll`
    write-back :110-112 + double-paint (component repaints same frame);
    (b) COMPONENT view tv_workspace.rs:475 — into its OWN private self.layout.
  - component ALREADY owns hit-test geometry: view() :467-471 self.layout=Default()→render_wide_tv_with_ctx→scroll;
    resolve_hit :419 (tv_wide_season_tabs), :427 (tv_wide_episode_rows), :433 (tv_wide_left_area.contains),
    :436-441 (tv_wide_right_area + tv_wide_list_area.y); page keys :287/:297 (tv_wide_list_area.height);
    wheel :447 (left_area). B4 precondition SATISFIED.
  - App layout.main tv_wide_* rects consumed (must survive via pre-pass, NOT the legacy branch):
    shell_tv_workspace.rs:135-137 render_tv_workspace_component reads app.layout.main.tv_wide_area;
    :44/:146 tv_workspace_component_id + push_tv_workspace_content gate on is_wide_tv_active()
    (layout.rs:196-198 tv_wide_right_area>0); shell_overlays_menus.rs:104-107 menu placement reads
    tv_wide_left_area/right_area; shell_inline_search.rs:50-51 reads main.tv_wide_right_area;
    mounted_tv_model test helper sets app.layout.main.tv_wide_right_area (:157-159)
  - precedent: Music 19b (music_wide.rs:79 publish_geometry ; widgets.rs:545-549 pre-pass into App layout.main)
    then 19c (delete legacy list.rs branch) EXACTLY mirrors TV shape. Movies wide branch list.rs:85-92 is the
    "already publish-only, no rows" precedent (comment :87-92).
- callers / tests: 4 legacy-pinning prod tests break on branch-delete unless pre-pass re-routes them:
  tv_wide_tests.rs:65 wide_tv_persists_series_workspace_and_separate_targets, :80 loading, :97 blank child,
  :138 focused bg at tv_wide_right_area; :116 selected_series_follows_inline_search_cursor = component path
  (safe). component/shell tests do NOT break (tv_workspace_component_tests :13/:69/:100/:125/:161;
  shell_tv_workspace.rs:166/:183). Conformance TV rows (width 60 <82, no hero) do NOT reach branch → safe:
  tests_conformance_matrix.rs:269/:418/:471 (all narrow). NO test requires legacy pre-publish before component
  hit-test (music R1 analog absent — component self-publishes).
- zero-reference/structural gate: DELETING legacy list.rs:103-114 branch → `is_wide_tv_library`/
  wide render only via component path; render_wide_tv_with_ctx STAYS (public component painter);
  component self.layout geometry STAYS; the App layout.main tv_wide_* rects STAY (pre-pass must publish them).
  `level.scroll` write-back (:112) orphaned → drop with branch (component scroll authoritative).
- do-not-touch: 18e teardown (sync_tv_workspace removal shell_tv_workspace.rs:63-90 + CONTEXT_STACK arms),
  18f (TvEpisodeActivate/TvEnqueue msg + tv_workspace.rs:238 TvActivate, series_detail_cache reader
  shell_tv_workspace.rs:119-133), 19e Music teardown (shell_music_workspace sync :151-231 differential test),
  tv_wide_render_ctx legacy (tv_wide.rs:75-97, pinned by spit tests), shared msg/types.
- honest scope: ≤3 production files (tv_wide.rs add layout_main pre-pass,
  list.rs delete branch :103-114, widgets.rs host pre-pass in render_library before render_list :555) + 1-2 tests
  (is_wide_tv_active/pre-pass geometry + tv_wide_tests re-route). shell/shell.rs stay OUT (resize-P2 deferred like Music).
- verification: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` (tv_wide_tests + shell_tv_workspace +
  tv_workspace_component_tests + conformance TV 269/418/471 — re-route the 4 legacy tests); clippy --workspace
  --all-targets; ast-grep scan; fmt --check; ignore 3 pre-existing conformance failures tests_conformance_matrix.rs:227/291/339.
- separation: 18d = App layout.main geometry pre-pass (tv_wide.rs + widgets.rs) + delete legacy list.rs:103-114
  ONLY after component owns geometry (it does); 18e teardown / 18f episode play / 19e Music teardown LATER.
- last-verified HEAD: 564dd1ef4abc3dce261d893719bc7646b6398a558 (scouts A+B+C; product unchanged to assign).

## 5.3d.19e (recon @ accept HEAD 15e7138e; scouts A+B+C on 2026-08-27)
- seam symbol(s): sync_music_workspace (shell adapter), music_workspace_component_id (adapter-only private ComponentId), differential test grouped_music_cursor_routing_matches_legacy_after_each_key
- definition (verified at HEAD 15e7138ea241bd835d73db6176d5caae25620bf2; scouts A+B+C):
  - pub(super) fn sync_music_workspace(&mut self) shell_music_workspace.rs:49-74 — NOT EMPTY despite row text: computes next_id via music_workspace_component_id (:50), unmounts old ID when changed (:51-54), mounts+activates MusicWorkspaceComponent::new() (:56-61), stores music_workspace_id (:62), calls push_music_workspace_content() (:63), clears music_track_focus_request when no workspace mounted (:66-73).
  - music_workspace_component_id: private, only definition/use shell_music_workspace.rs:31/:50 → zero-ref if adapter deleted.
  - music_workspace_id (shell.rs:58 field, init :101): written ONLY by the adapter (:62); read by focused-track lookup shell_music_workspace.rs:14, content writer :80, renderer :127, shell render shell.rs:1150. Deleting the adapter orphans the write → mount must be re-homed or the field's writers moved; cannot be dropped with the adapter alone.
  - music_track_focus_request (shell.rs:68, init :104): adapter clears :72; push_music_workspace_content consumes :115-120; shell event handling writes shell.rs:305/:312. Independent of adapter deletion.
  - writer path: push_music_workspace_content shell_music_workspace.rs:79-124 (set_content :100-120, set_album_columns, set_page_rows) — ALREADY routes independently from shell event/layout paths: shell.rs:273,287,344,418-419,429,553,609,621,629,640,1138 (19a mirror behavior, adapter not required after mount).
  - component: MusicWorkspaceComponent::new() components/music_workspace.rs:41-78; set_content :80-125 (mirrors context, resets focus on album identity change).
- callers / tests:
  - SOLE production caller: src/app/shell.rs:1090 — CONTEXT_STACK/layout-sync dispatch arm calling self.sync_music_workspace() alongside sync_tv_workspace() and sync_active_destination(). shell.rs:62 = doc-comment only, not a call.
  - DIFFERENTIAL DELETION TARGET: shell_music_workspace.rs:214-294 grouped_music_cursor_routing_matches_legacy_after_each_key — drives legacy App::handle_key + TuiRealm component keys in parallel, asserts cursor equality after each key; pins the two-path legacy-vs-component shape; delete WITH teardown (per row text).
  - 9 shell tests use sync_music_workspace as MOUNT SETUP (not differential coverage) and break on pure deletion unless setup re-homed: shell_mounts_and_syncs_music_workspace :159-180; push_music_workspace_fetches_selected_album_tracks :183-211 (sync at :197); grouped_music_cursor_no_fallthrough_when_left_sorted_indices_empty :297-344; shell_mounts_music_workspace_in_narrow_mode :347-364; music_resize_push_uses_current_frame_geometry :366-417; narrow_music_workspace_ignores_enter_for_inline_track_focus :419-444; wide_music_workspace_allows_enter_for_inline_track_focus :446-479; recursive_album_activation_enters_track_focus_only_in_wide :481-532; position_restore_request_clears_track_focus_at_next_sync :534-570.
  - NON-differential characterization tests that MUST SURVIVE (render/tests_music_characterization.rs): music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states :21-45; narrow_grouped_music_hero_shows_only_title_meta_no_track_table_or_action_hint :48-74; wide_grouped_music_publishes_same_frame_layout_geometry :77-88; narrow_grouped_music_publishes_no_wide_track_targets :90-97. No sync reference in tests_music_characterization.rs or component tests.
  - Component tests survive (components/music_workspace_component_tests.rs :60-70..:315+, incl. :144-158 music_workspace_renders_without_app). NO test pins the literal existence of sync_music_workspace; only the differential test pins the two-path shape.
- zero-reference gate: sync_music_workspace must have zero refs outside its (deleted) def + removed shell.rs:1090 arm; music_workspace_component_id zero-ref (adapter-only); the shell.rs:1090 dispatch arm must not call a dead fn. music_workspace_id is NOT a zero-ref deletion target — it stays live (writer/renderer readers) and needs a new writer if the adapter goes.
- do-not-touch: 18e/18f (sync_tv_workspace shell_tv_workspace.rs:77-92 + push_tv_workspace_content :94-163, shell.rs:1089 caller, CONTEXT_STACK TV arms, TvEpisodeActivate/TvEpisodeEnqueue), 18d TV geometry (tv_wide.rs publish_geometry, widgets.rs pre-pass, list.rs legacy branch), 20-series inline-search files, shared imports shell_music_workspace.rs:1 (BrowserKey, BrowserKind, ComponentId, MusicWorkspaceComponent, MusicWideRenderCtx, ServiceKind — do NOT bulk-delete; ComponentId/MusicWorkspaceComponent/MusicWideRenderCtx still used by push/render/focused paths; ServiceKind/Browser* only if verified), tasks.md/ledger, App fields music_workspace_id + music_track_focus_request (live readers/writers outside adapter).
- precedent (scout C): 18d (fdffbd4e) deleted legacy wide-TV underpaint from list.rs + added publish_geometry to tv_wide.rs but DID NOT touch sync_tv_workspace — left it intact, scheduling adapter deletion for the later explicit 18e row ("remove the empty sync_tv_workspace adapter + CONTEXT_STACK TV arms"). sync_tv_workspace is CURRENTLY non-empty at HEAD (mount/unmount/activate + push, called shell.rs:1089, test shell_tv_workspace.rs:161) — so the TV sibling has the SAME non-empty-adapter reality. Earlier ABS precedent (5.3d.11-U6, commit 5ca1b099) = retain sync as MOUNT-ONLY, split projection to push-content. → 19e row text ("remove the EMPTY adapter") CONFLICTS with HEAD reality (adapter non-empty, sole music_workspace_id writer, 9 tests' mount setup). Orchestrator decision needed: (i) delete now + re-home mount/writer + migrate 9 setups, (ii) mirror U6 (retain mount-only, defer full deletion), or (iii) other. Manager reports conflict; does NOT pick.
- verification: package mbv (src/). `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv` with filter group: deletion target `-E 'test(grouped_music_cursor_routing_matches_legacy_after_each_key)'` (to confirm it disappears), stay-green: shell music suite (`-E 'test(music_workspace_)'` + `-E 'test(grouped_music_)'` file-isolated shell_music_workspace.rs), characterization (`-E 'test(music_buffer_characterization_|grouped_music_)'` file-isolated tests_music_characterization.rs), component (`-E 'test(music_workspace_)'` file-isolated music_workspace_component_tests.rs); `rtk cargo clippy --workspace --all-targets`; `rtk ast-grep scan`; `rtk cargo fmt --all -- --check`; ignore 3 pre-existing conformance failures tests_conformance_matrix.rs:227/291/339.
- instrumentation caveat (preserve, do not upgrade): scouts could NOT activate pi-lens ast-grep in this session (override extensions:[] disabled the tool); all caller/symbol mapping is rg/grep literal-identifier based, cross-checked. Exact-identifier matches are low-risk; any structural-pattern claim would need re-verification with ast-grep.
- honest scope: row says ≤3 files but a literal pure-deletion diff is not viable at HEAD: deleting the adapter alone removes the only music_workspace_id writer and breaks 9 tests' mount setups + the differential test. Realistic footprint ≥2 files (shell_music_workspace.rs + shell.rs:1090 arm) + 9 test-setup migrations + mount/writer re-home decision. If orchestrator chooses a minimal path, re-scope BEFORE assigning.
- last-verified HEAD: 15e7138ea241bd835d73db6176d5caae25620bf2 (scouts A+B+C unanimous)

## 5.3d.18e (recon @ accept HEAD 76d05841; scouts A+B+C on 2026-08-27)
- seam symbol(s): sync_tv_workspace (shell adapter), tv_workspace_component_id (adapter helper), tv_workspace_id (App Model field, stays), "CONTEXT_STACK TV arms" (row text — DO NOT EXIST at HEAD), "obsolete mount/sync names" (row text — none provable at HEAD)
- definition (verified at HEAD 76d058414b54db7e05188c0f700caf34312cfc4e; scouts A+B+C):
  - pub(super) fn sync_tv_workspace(&mut self) shell_tv_workspace.rs:77-92 — NOT EMPTY despite row text: computes next_id via tv_workspace_component_id (:78), unmounts old component when ID changes (:80-82), mounts TvWorkspaceComponent (:84-86), activates (:87), stores tv_workspace_id (:88), calls push_tv_workspace_content() (:89).
  - tv_workspace_component_id helper shell_tv_workspace.rs:61-75 — used by the adapter at :78, not uncalled.
  - tv_workspace_id Model field shell.rs:57 (init :100) — read/written by mirror_tv_workspace_cursor :43, push_tv_workspace_content :95-96/:129, render_tv_workspace_component :133-134; NOT vestigial; survives (requires mounted component state).
  - CONTEXT_STACK: zero textual occurrences in src/app/shell.rs at verified HEAD (scout A grep; ast-grep unavailable — see caveat). The TV run-loop sync is the ordinary layout/effect call shell.rs:1089 — NOT a CONTEXT_STACK arm. Row's "CONTEXT_STACK TV arms" deletion target does not exist.
  - obsolete mount/sync names: none provable at HEAD — tv_workspace_component_id and sync_tv_workspace are both live (callers :1089 / :78). shell.rs has NO other TV mount/sync identifier beyond field :57, init :100, call :1089.
- callers / tests:
  - SOLE production caller: src/app/shell.rs:1089 — run-loop effect handoff calls self.sync_tv_workspace() between sync_emby_browser() and sync_music_workspace(). (NOT a CONTEXT_STACK arm.)
  - Test-only caller: shell_tv_workspace.rs:161 mounted_tv_model() (setup fixture :153-163) calls model.sync_tv_workspace() to mount.
  - NO differential/characterization legacy-vs-component test for TV exists (no analogue to Music's grouped_music_cursor_routing_matches_legacy_after_each_key, which 19e deleted) — there is NO TV differential deletion target.
  - The only tests pinning the adapter (via mounted_tv_model setup): shell_tv_workspace.rs:165-180 push_tv_workspace_content_projects_selected_series_on_mount (asserts adapter mount+push behavior); :182-209 typed_tv_requests_keep_component_cursor_authoritative (mounted behavior via setup). Both break if the adapter + :161 setup are removed.
  - SURVIVE (no sync reference, direct component usage): components/tv_workspace_component_tests.rs :10-55,:58-88,:91-112,:115-165; render/components/tv_wide_tests.rs :88-101,:103-117,:120-136,:139-159,:161-194 (helper :23-34 instantiates component directly, app.render_library only publishes geometry); render/tests_non_music.rs:177-213 tv_series_list_computes_sorted_indices_when_above_threshold (direct create/set/view).
  - NO test asserts an obsolete mount/sync NAME (only component-lookup error strings :171,:175,:178,:185,:189,:192). NO test asserts CONTEXT_STACK arms. NO test exercises push_tv_workspace_content directly (both shell tests reach it via sync setup) — residual gap: direct push has no surviving test if adapter+fixture go.
- zero-reference gate: row as written (delete adapter + CONTEXT_STACK arms + obsolete names) has NOTHING to delete that is provably zero-ref: adapter is live (caller shell.rs:1089 + fixture :161), CONTEXT_STACK absent, no obsolete names. Deleting adapter alone WITHOUT removing shell.rs:1089 call leaves a caller → gate fails. If orchestrator routes full adapter deletion, the gate = sync_tv_workspace + tv_workspace_component_id zero refs outside deleted def + remove shell.rs:1089 + re-home mounted_tv_model fixture (:161) + address tv_workspace_id writer (adapter :88 is a writer; writer/renderer :95/:133 still read it).
- do-not-touch: 18d accepted geometry (render_wide_tv_with_ctx tv_wide.rs:102, tv_wide_* publishers, App layout.main tv_wide_* rects consumed by shell_tv_workspace.rs:135-137 + shell_overlays_menus.rs:104-107 + shell_inline_search.rs:50-51, component-owned view geometry/hit-tests tv_workspace.rs:467-471); 18f (TvEpisodeActivate/TvEpisodeEnqueue msg + App methods B3, series_detail_cache reader shell_tv_workspace.rs:115, index 18d entry cites :119-133); Music (shell_music_workspace.rs, music_workspace.rs, tests_music_characterization.rs); 20-series inline-search; shared component/render types; tasks.md. 19e commit 76d05841 touched ONLY tasks.md + shell_music_workspace.rs — no TV test file changed (scout B git show --stat).
- precedent (scout C, decisive): accepted 19e (76d05841) had the SAME "remove the empty sync_music_workspace adapter" wording, scouts had flagged the adapter NON-empty at 15e7138e (mount/activate, sole music_workspace_id writer, initial push, focus-request clear). The ACCEPTED 19e resolution: RETAINED the live adapter as mount-only lifecycle ownership, DELETED only the differential test (grouped_music_cursor_routing_matches_legacy_after_each_key, 83-line #[test] hunk), DEFERRED full lifecycle teardown to aggregate teardown (5.3d.21-24), and recorded the re-scope note in tasks.md. Convention = when row text's "empty adapter" conflicts with HEAD reality, keep the live adapter, delete only what is provably removable. 18e therefore mirrors 19e: adapter is live, CONTEXT_STACK arms absent, NO differential test exists → the literal 18e "teardown" targets nothing provably deletable at HEAD. Orchestrator convention-vs-literal decision needed (manager does not decide); factual options emerged from reports: (i) re-scope 18e to ledger-only (record no-op + differ) mirroring 19e's retention without the test deletion (no TV differential exists), (ii) full teardown = bigger unit (remove :1089 caller, re-home mounted_tv_model, decide tv_workspace_id writer fate — exceeds the ≤3 "empty adapter" frame and overlaps 18f's cache reader boundary), or (iii) other. No verdict issued.
- verification (per scout B exact filters, pkg = mbv, --lib): `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv --lib shell_tv_workspace`; `rtk cargo nextest run -p mbv --lib tv_workspace_component_tests`; `rtk cargo nextest run -p mbv --lib tv_wide_tests`; `rtk cargo nextest run -p mbv --lib tv_series_list_computes_sorted_indices_when_above_threshold`; clippy --workspace --all-targets; fmt --check; ignore 3 pre-existing conformance failures tests_conformance_matrix.rs:227/291/339.
- instrumentation caveat (preserve, do NOT upgrade): pi-lens allowlist config did NOT reach the child runtime this round — scouts A+B report pi_lens_activate_tools/ast_grep_search UNAVAILABLE; all mapping is exact-identifier grep + cross-check (low FP risk for exact fn names; CONTEXT_STACK-zero and obsolete-name-zero claims are textual). Structural-pattern claims would need ast-grep re-verification. Config fix verified present at the manager level (.pi/settings.json scout extensions = [pi-lens dist path]) but child sessions did not load it — flag to orchestrator before the next recon if ast-grep-first is mandatory.
- honest scope: as-written row is effectively a NO-OP candidate at HEAD (nothing provably deletable); a full teardown exceeds the row's frame and needs an orchestrator decision first. Options are reported, not chosen.
- last-verified HEAD: 76d058414b54db7e05188c0f700caf34312cfc4e (scouts A+B+C unanimous)

## 5.3d.18f (recon @ accept HEAD e1793bf3; scouts A+B+C on 2026-08-27)
- seam symbol(s): NEW (zero refs at HEAD): TvEpisodeActivate / TvEpisodeEnqueue ShellRequest variants (msg.rs:420-452 neighborhood — TvMoveRows :423-425, TvJumpCursor :432-434, TvActivate :437, TvEpisodeMove :446-448, TvSeasonMove :451-452); + new App methods (B3) to resolve episodes[season_id][episode_cursor]. This is a NEW-FEATURE row, not a teardown — the gap is real at HEAD.
- the raw play/enqueue paths the row implies: DO NOT EXIST at HEAD (scout A). Episode-pane Enter = Key::Enter => None (components/tv_workspace.rs:235-240); unhandled keys fall to Msg::Legacy unmapped-key path (:321-326, via :473-480 legacy forwarding); legacy browse Enter (input_browse_dispatch.rs:139-166) = activate_selected_series → ENTERS the episode workspace (:162-172), never plays an episode. Enqueue: generic Ctrl+A (input_lib_keys.rs:73-83 handle_enqueue_selected_key → enqueue_selected(Some(lib_idx))) resolves App/library selection, NOT component episode_cursor — no TV-specific episode enqueue handler exists anywhere in tv_workspace.rs / shell_tv_workspace.rs / dispatch.
- episode resolution source (scout A): series_detail_cache: HashMap<String, SeriesDetail> src/app/app_struct.rs:362; SeriesDetail { seasons: Vec<EmbyItem>, episodes: HashMap<String, Vec<EmbyItem>> } src/app/types_browse.rs:24-31 (episodes keyed by season id); season_id derivation = detail.seasons[season_index].id (same established shape src/app/lib_cursor_actions.rs:351-389, esp. :376-389); writer/projection push_tv_workspace_content shell_tv_workspace.rs:94-129 (cache read :115, TvWideRenderCtx::new :117-129, season index 0 + cursor None at :121-126); component stores episode_cursor: Option<usize> tv_workspace.rs:34, season_cursor near :33, season selection component-local move_season :108-142; episode rows from detail.episodes[season_id] in component dataset helpers :335-367/:420-434.
- typed seam 18f extends (scout A+C): component emits Some(Msg::Shell(ShellRequest::...)) tv_workspace.rs:276-310 (move_rows :276-302 / jump_cursor :304-310, wrapper :321-326); shell routes shell.rs:975-988 → Model::handle_tv_request shell_tv_workspace.rs:8-38 (mirror_tv_workspace_cursor(lib_idx) :21/:33; TvActivate → self.app.activate_selected_series(lib_idx) :23-25 (App method input_browse_dispatch.rs:162-172, series-only); TvBack/letter :26-29; TvEpisodeMove/TvSeasonMove = deliberate no-ops :35-38 pending this slice).
- callers / tests (scout B):
  - Raw-path pin 18f CHANGES: components/tv_workspace_component_tests.rs:158-217 tv_keyboard_uses_typed_requests_and_routes_brackets_by_pane — final Enter assertions are explicitly Msg::Legacy(LegacyTerminalEvent::Key(_)) (episode activation not typed yet); 18f replaces that expectation with the typed request. Non-episode assertions in the same test survive.
  - NO test exercises episode enqueue (no TvEpisodeEnqueue anywhere under src/); NO test asserts episode cursor/index resolved from series_detail.episodes[season_id][cursor]; NO TV-specific enqueue dispatch test exists in the allowed scope — new coverage needed for the enqueue arm.
  - SURVIVE (direct component usage, no sync): tv_workspace_component_tests.rs:131-155 tv_grouped_cursor_mirrors_rendered_sorted_rows; :100-126 tv_episode_brackets_with_modifiers_fall_through_to_legacy; shell_tv_workspace.rs:166-180 push_tv_workspace_content_projects_selected_series_on_mount; :183-209 typed_tv_requests_keep_component_cursor_authoritative (asserts Msg::Shell(ShellRequest::TvMoveRows{rows:1}) + handle_tv_request — the 18f assertion PATTERN to copy); render/components/tv_wide_tests.rs:90-99 (episode rows Pilot/1h rendered), :101-113 (loading fan-out), :116-136 (blank child); render/tests_non_music.rs:117-153 tv_series_list_computes_sorted_indices_when_above_threshold. tv_wide_tests.rs:75-83 builds series_detail_cache episodes["season-1"] fixture.
- zero-reference gate: INVERTED for this row — the new symbols must be WIRED, not zeroed: variants added to ShellRequest, emitted by the component on the episode pane, dispatched through handle_tv_request, resolved by new App methods reading the cache by selected_series.id → season_id (from active season) → episodes[season_id][episode_cursor]. The legacy Enter fall-through on the episode pane (Msg::Legacy path) must shrink as the typed request takes over; no existing symbol is deleted.
- do-not-touch: 18d accepted geometry (render_wide_tv_with_ctx render/components/tv_wide.rs:102, tv_wide_* publishers + App layout.main consumers shell_tv_workspace.rs:135-137, shell_overlays_menus.rs:104-107, shell_inline_search.rs:50-51, component view geometry/hit-tests tv_workspace.rs:467-471); 18e mount-only lifecycle (sync_tv_workspace shell_tv_workspace.rs:77-92, push_tv_workspace_content :94-129 — the writer MUST keep feeding the component; do not break the cache read :115); Music 19e (shell_music_workspace.rs, music_workspace.rs, tests_music_characterization.rs); inline-search 20c-20f; existing TvMoveRows/TvJumpCursor/TvBack/TvCycleLetterPill semantics; shared component/render types; tasks.md.
- precedent (scout C): 18a/18b typed conversions define the exact pattern — add ShellRequest variants, emit component-side as Some(Msg::Shell(req)), resolve in handle_tv_request via App method, assert with the mounted_tv_model fixture pattern (shell_tv_workspace.rs:153-163 fixture, :183-209 typed assertion). 18a tasks.md wording ("keep episode play/enqueue raw for 18f") confirms the current raw fall-through is intentional until 18f — this row closes it. TvEpisodeActivate/TvEpisodeEnqueue are ADDITIONS, not renames (zero refs at HEAD, only prose mentions).
- honest scope: ≤3 file box likely exceeded by a faithful implementation — msg.rs (2 new variants) + tv_workspace.rs (episode-pane emission + enqueue key) + shell_tv_workspace.rs (handle_tv_request arms + App methods) + test file(s) (tv_workspace_component_tests.rs:158-217 expectation + new enqueue coverage) = 4+ files, plus the row itself says "needs new App methods (B3)". Also the ROW DOES NOT NAME the enqueue trigger key (Ctrl+A is currently library-global; the episode-pane enqueue key is a design/UX choice) and does not specify episode-play side effects (mpv play request? play queue? navigate?) — both are product decisions for the orchestrator, reported here, not invented by scouts.
- verification: pkg = mbv (--lib). `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv --lib -E 'test(tv_keyboard_uses_typed_requests_and_routes_brackets_by_pane)'` (expectation change) + `-E 'test(tv_episode_brackets_with_modifiers_fall_through_to_legacy)'`, `-E 'test(tv_grouped_cursor_mirrors_rendered_sorted_rows)'`, `-E 'test(push_tv_workspace_content_projects_selected_series_on_mount)'`, `-E 'test(typed_tv_requests_keep_component_cursor_authoritative)'`, `-E 'test(wide_tv_persists_series_workspace_and_separate_targets)'`, `-E 'test(wide_series_render_keeps_loading_treatment_during_season_fan_out)'`, `-E 'test(wide_series_with_no_seasons_keeps_the_child_region_blank)'`, `-E 'test(tv_series_list_computes_sorted_indices_when_above_threshold)'`; clippy --workspace --all-targets; fmt --check; ignore 3 pre-existing conformance failures tests_conformance_matrix.rs:227/291/339.
- instrumentation caveat (preserve, do NOT upgrade — 3rd consecutive round): pi-lens/ast-grep STILL unavailable in the scout child runtime despite .pi/settings.json scout override allowlisting /home/slatkin/.pi/agent/npm/node_modules/pi-lens/dist/index.js (scouts A+B+C all report "pi_lens inactive / ast-grep not exposed"). Mapping = exact-identifier grep/read cross-check (low FP risk for exact symbols; zero-ref claims for the new variants are textual absence within searched src). Flag to orchestrator: config at manager level is correct, but children do not load it — if ast-grep-first is mandatory for later rows, the runtime/config needs diagnosis (agent session caching? allowlist path format? subagentOnlyExtensions?).
- last-verified HEAD: e1793bf36b3ee808b7f9b3246908110ae9a0ae82 (scouts A+B+C unanimous)

## 5.3d.13 (recon @ HEAD 838306ba = accepted 18f commit; scouts A+B+C on 2026-08-27, REPORT-ONLY gate)
- SEAM SCOPE: ABS BOOK surface symbol-level map for a Phase-B report gate. Literal row: "Scout the ABS book typed-input, interaction-reader, legacy-render, image, and layout teardown at symbol level; add the resulting bounded rows here before any Phase-B book writer starts. Open report-only gate before any ABS Book Phase-B writer; no Phase-B production tasks are invented below it. (The Phase-A push helper at 5.3d.12 is checked.)"
- ABS BOOK owned file set at HEAD (scout A+C): src/app/shell_audiobookshelf_book.rs (sync :32-61, push :76-108, render :110-126, key bridge :8-12), src/app/components/audiobookshelf_book.rs (component; handle_key :107-129, handle_mouse :151, set_content :42-66), src/app/render/components/audiobookshelf_book.rs (:1-480 component painter), src/app/render/components/audiobookshelf_books.rs (:1-24 BookHeroPlan; comment: legacy App renderer removed), src/app/audiobookshelf_browse_actions.rs (App actions :381-568), src/app/types_audiobookshelf_browse.rs (AudiobookshelfBookBrowseState :315-342; cursor() :365-373, select() :375-386), src/app/app_struct.rs (:62-63 audiobookshelf_book_browse Vec), src/app/audiobookshelf_book_modal_actions.rs, src/app/audiobookshelf_book_seek_tests.rs, component tests, plus readers in library_position_state.rs:256-345, lib_event_actions.rs:192-273/763-786, run_loop_drains.rs:63-107, selection_modal_actions.rs:202-217. Podcast files (shell/components/render/types podcase) are ADJACENT, not book logic.
- typed-input seam (scout A): ShellRequest ::194; ONLY ABS-book variant = AudiobookshelfBookKey(crossterm KeyEvent) msg.rs:355-357 (raw-key carry-all). Component handle_key emits this RAW variant for every key — book is NOT typed like podcast; ALL book keys still legacy/raw-bridged. handle_key :107 (Event::Keyboard), handle_mouse :151 (Event::Mouse), both via AppComponent::on :223-229. Shell bridge handle_audiobookshelf_book_key shell_audiobookshelf_book.rs:8-12 → self.app.handle_key(key); shell.rs:822-831 receives ShellRequest::AudiobookshelfBookKey → bridge → push. LEGACY App key reader handle_key_audiobookshelf_book_library input_browse_dispatch.rs:182-250 still dispatches all legacy keys (chapter_selection, cursor movement, layout focus).
- interaction-reader / mirror seam: NO *_reader / mirror_* named fn for book; the content mirror is push_audiobookshelf_book_content (Phase-A). sync_audiobookshelf_book :32-61 = mount lifecycle only (guarded by active AudiobookshelfLibrary(Book); mounts/activates component; unmount stale; fresh-mount push call :63). Production callers: sync → shell.rs:1085 (1 call); push → shell.rs:272,286,343,428,552,831 + fresh-mount :63 (7 call sites); render → shell.rs:1148. NO second sync-like adapter.
- legacy-render seam (scout B): NO legacy Book painter remains. widgets.rs:581-600 render_audiobookshelf_library — Book predicate branch writes ONLY layout.audiobookshelf_book_area and returns (reserves overlay area; comments :583-588 say legacy App renderer removed). Component painter render_audiobookshelf_book_content audiobookshelf_book.rs:37-146 (narrow :172+, browser :172-273) writes component-local geometry fields (decl :21-34 selector_tabs/book_rows/chapter_rows/hero_area/selected_item_rect; writes :58,:90,:125,:210,:216,:228-229,:239,:255-256,:423-439). audiobookshelf_book_area = LIVE shell placement contract (widgets writer :594-599, shell reader shell_audiobookshelf_book.rs:114-116) — NOT zero-ref.
- image/cover seam (scout B): images.rs:342-349 fetch_audiobookshelf_book_cover (cache key :22-26) → fetch_audiobookshelf_image (:351+). SOLE caller render/components/card.rs:149 (is_book card branch); sibling :151 fetch_audiobookshelf_cover. Book surface itself is component-driven via HomeImagePaint::AudiobookshelfCover emitted audiobookshelf_book.rs:355-359 (writer path, not legacy fetch). No book-surface fetch caller found.
- zero-ref gate / classification (scouts A+B): NO provable zero-production-caller ABS-book symbol at HEAD — sync (1 prod caller), push (7), key bridge (shell :822-831), component handle_key/handle_mouse (via on :223-229), legacy App key reader (input_browse_dispatch.rs:182 via shell bridge), render adapter (shell.rs:1148), audiobookshelf_book_area (widgets writer + shell reader) ALL live. Legacy App renderer already deleted (comment evidence; no legacy Book render/test file remains — former tests_audiobookshelf_books.rs deleted per test_helpers.rs:636-637 comment). 5.3d.13 gate outcome = the Phase-B bounded-row plan, NOT a deletion diff: candidate bounded rows for orchestrator (facts only, not invented): (R1) convert book keys from raw AudiobookshelfBookKey to typed requests mirroring podcast U6 convention; (R2) delete legacy handle_key_audiobookshelf_book_library + shell bridge after R1 (they become zero-ref only after typed seam lands); (R3) App-state/reader cleanup after R2. render/image/layout: NO teardown rows needed (already componentized); audiobookshelf_book_area stays.
- do-not-touch (scout C): podcast U6 do-not-touch (index :87-105): shared AudiobookshelfBrowseState + selected_id/episode_filter/episode_selection/scroll, U0 accessors, U5 playback target, U3 modal filter, U4 position persistence, render_audiobookshelf_podcast_component, podcast layout area; AND the cross-sibling note explicitly keeps Book seams (sync/push/abs_book_*) inside 5.3d.13. TV 18f files (accepted 838306ba), Music 19e, inline 20c-20f OUT. crates/, shell.rs beyond exact ABS-book identifiers, openspec/docs OUT.
- precedent (scout C): 5.3d.12 checked at 4f5df745 + 354fc5c0 = the ABS BOOK Phase-A two-file push helper (mount-only reconciliation + writer-seam pushes; App writers/component/renderer unchanged). Podcast U6 (5ca1b099) = sibling convention: sync stays mount-only, projection split to push_* writer; deleted abs_podcast_component_id + dead enter_episode_selection; shell.rs writer-site push calls repointed; NO App-field deletion. Book Phase-B should mirror: typed requests + legacy-reader teardown AFTER the seam exists.
- verification (report-only gate: no diff expected): pkg = mbv. If Phase-B rows land: `rtk cargo check -p mbv`; `rtk cargo nextest run -p mbv --test render -E 'test(render_library_sets_book_area_before_component_overlay)'` + conformance matrix file-isolated (named Book cases at tests_conformance_matrix.rs:272-297, :314-352 — NOT name-isolated, matrix file shares other surfaces; use file isolation + the named book-area test); component tests audiobookshelf_book_component_tests.rs; clippy --workspace --all-targets; fmt --check; ignore 3 pre-existing conformance failures tests_conformance_matrix.rs:227/291/339.
- instrumentation caveat (preserve, do NOT upgrade — 4th consecutive round): pi-lens/ast-grep STILL unavailable in scout child runtime despite .pi/settings.json allowlisting pi-lens dist path (scouts A+B+C "pi_lens/ast-grep unavailable"). All mapping = exact-identifier grep/find/read cross-checks (low FP for exact symbols; zero-ref/new-symbol claims textual within searched scope, not AST proof). Config at manager level correct but children do not load it — needs diagnosis before any row requiring ast-grep-first.
- last-verified HEAD: 838306baa9eb308882378e3e988f32662baa53ab (scouts A+B+C unanimous; = accepted 18f commit, TV out of their scope)

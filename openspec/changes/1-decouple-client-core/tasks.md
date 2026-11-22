# Tasks

## Execution rules

Dispatch one row to one fresh worker. Rows are edit-only: a row is complete when its named edits exist and its own `rg` check, if it has one, passes. The workspace may stay broken between rows. A row edits at most about six files by hand. A row marked **Sweep** is a mechanical rename or import change across any number of files: do it with search-and-replace, follow the compiler errors that it causes, and make no other change. The last row of each group is a **Gate**. It runs the group's checks and fixes the errors that remain. If errors remain in more than about six files, the gate does not fix them. It reports the failing files and the row that owns each one, and that row is dispatched again. Before you dispatch a row, check its premises (the symbols and files it names) against the code. If a file list grew past the six-file bound, split the row before you dispatch it.

Run every cargo command in the foreground and wait for it to finish. Do not background it, sleep, or poll. Run `cargo fmt` after each Rust change. Move files with `git mv` so history follows them. Keep the module layout rules in `AGENTS.md` (no `mod.rs`, no `#[path]`, no `include!`). Add no lint suppression. A test moves with the code it owns. Add a test only where a row names its contract. No row changes a test expectation. Every gate runs `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and the nextest packages it names.

## 1. TUI-only leaf crate and colour conversion (design D7)

- [ ] 1.1 Create `crates/mbv-tui-support` (workspace version, edition, lints) with dependencies `ratatui` (workspace, 0.30), `ratatui-image` (workspace, 11, `crossterm` feature), and `mbv-theme`, `mbv-images` path dependencies. Add it to `members` and `default-members` in the root `Cargo.toml`.
- [ ] 1.2 Add a `ToRatatui` extension trait in `mbv-tui-support` with the same results as `mbv-theme`'s `to_ratatui`, and move the conversion's tests to it. Delete `to_ratatui` and the `ratatui` feature from `mbv-theme`.
- [ ] 1.3 **Sweep.** Make the 37 `mbv-render` files that call `to_ratatui` import `mbv_tui_support::ToRatatui`. Change `mbv-render`'s `Cargo.toml` (drop the `mbv-theme` `ratatui` feature, add `mbv-tui-support`).
- [ ] 1.4 **Sweep.** Do the same for the 12 `mbv-components` files and the 11 root-package files, and for their manifests.
- [ ] 1.5 **Gate.** nextest `-p mbv-tui-support -p mbv-theme -p mbv-render -p mbv-components`. `cargo tree -p mbv-theme -e normal --prefix none` lists no `ratatui`.

## 2. Terminal image protocol (design D7)

- [ ] 2.1 Move `mbv-images/src/protocol.rs` and the terminal parts of `lib.rs` and `cache.rs` (the picker, protocol storage, and every field that holds a `ratatui-image` type) into `mbv-tui-support`, with their tests in `cache/tests.rs`. Decode, cache keys, and pixel processing stay in `mbv-images`.
- [ ] 2.2 Move the terminal parts of `resize.rs` and `title_overlay.rs` (with `title_overlay/tests.rs`) into `mbv-tui-support`. Remove `ratatui`, `ratatui-image`, and `crossterm` from `mbv-images/Cargo.toml`.
- [ ] 2.3 **Sweep.** Change the protocol consumers to the new paths: `crates/mbv-render/src/components/card.rs`, `src/app.rs`, `src/app/shell/run/drains.rs`, `src/app/infra/image_fetch/protocol.rs`, and their tests (`protocol/protocol_tests.rs`, `state/projection/card/{title_site,queue_slot}_tests.rs`, `tests/tick_integration/queue_playback.rs`).
- [ ] 2.4 **Gate.** nextest `-p mbv-images -p mbv-tui-support -p mbv-render -p mbv`. `cargo tree -p mbv-images -e normal --prefix none` lists no `ratatui`, `ratatui-image`, or `crossterm`.

## 3. Shared items move down to `mbv-ui-model` (design D13 Rule A)

Each definition row leaves a transitional `pub use mbv_ui_model::...` at the old path. Rows 3.10 and 3.11 change the callers, and row 3.11 deletes the re-exports. Where an item's type holds a ratatui value, the data moves and the ratatui conversion stays in its TUI crate.

- [ ] 3.1 Move `current_time_secs` (`screens/feeds_model.rs`), `book_rows` (`components/audiobookshelf_book.rs`), and `MUSIC_ALBUM_IMAGE_TYPES` (`components/widgets.rs`) from `mbv-render`, with their tests.
- [ ] 3.2 Move `HeroImageState` (`components/tv_wide.rs`) and `TransportAvailability` (`components/chrome_player.rs`) from `mbv-render`, with their tests.
- [ ] 3.3 Move `group_album_plan`, `sorted_group_album_order`, `initial_group_artist_sort_key`, and `effective_sort_str` (`screens/album_plan.rs` and the files that `mbv-render/src/lib.rs` re-exports them from), with their tests.
- [ ] 3.4 Split `mbv-render/src/components/indicators.rs`: the data of `IndicatorStyle`, `StatusIndicator`, `IndicatorData`, `IndicatorFlags`, and `short_resolution_label` move. The colour and `Style` methods stay in `mbv-render` as free functions or an extension trait over the moved types.
- [ ] 3.5 Move `MusicWideRenderCtx` (`components/music_wide.rs`) and `LibraryListRenderCtx` (`components/list_rows.rs`), struct definitions and constructors only, and the constants `LIBRARY_PILL_THRESHOLD`, `LEFT_WIDTH_DEFAULT`, and `LEFT_WIDTH_STEP` (`layout.rs`).
- [ ] 3.6 Move the data of `HeroArtwork` (with `painted_shape`), `ArtworkSource`, `ArtworkShape`, `HeroFacts`, `PanelHeroImagePaint`, and `painted_shape_for_pixels` from `mbv-components/src/library_panel/content.rs`, with their tests. The paint code stays.
- [ ] 3.7 Move `hero_content_queue`, `hero_content_feed`, `hero_content_emby`, `HeroContentData`, and `emby_artwork_policy` and `feed_artwork_policy` from `mbv-components/src/library_panel/hero.rs`, with their tests.
- [ ] 3.8 Move `hero_content_abs_episode`, `hero_content_abs_book`, `hero_content_series_with_episode`, `abs_book_artwork_policy`, and `abs_episode_artwork_policy` from the same file, with their tests.
- [ ] 3.9 Move `premiere_number_label` (`tv_content/episode_rows.rs`) and `sanitize_url` (`library_panel/overview_box.rs`) from `mbv-components`, with their tests.
- [ ] 3.10 **Sweep.** Make the root package and `mbv-gui` import every item of 3.1–3.9 from `mbv-ui-model`. Import `LetterFilter` and `LetterFilterKind` from `mbv_ui_model::sort_filter` in place of the `mbv-render` re-export.
- [ ] 3.11 **Sweep.** Make `mbv-render` and `mbv-components` import the same items from `mbv-ui-model`, and delete the transitional re-exports.
- [ ] 3.12 **Gate.** nextest `-p mbv-ui-model -p mbv-render -p mbv-components -p mbv -p mbv-gui`. `cargo tree -p mbv-ui-model -e normal --prefix none` lists no `ratatui`, `crossterm`, or `tuirealm`. `rg -n "mbv_(components|render)::" src/app/gui_* src/app/input/gui*` finds nothing.

## 4. TUI projections and infrastructure under the shell (design D3)

- [ ] 4.1 Move the ratatui part of `src/app/state/projection/card.rs` (745 lines) under `src/app/shell/` as shell functions over `App` reads, with the tests in `projection/card/` that own it. Keep any part that builds no ratatui value in `state/`.
- [ ] 4.2 Do the same for `state/projection/chrome_status.rs` (647 lines).
- [ ] 4.3 Do the same for `state/projection/tv_wide.rs` and `state/projection/visualizer.rs`.
- [ ] 4.4 Make the `impl App` block of `src/app/shell/draw.rs` (`compute_frame_layout`, `compute_chrome_geometry`, `queue_panel_placement`, `compose_root_frame`) into shell free functions over `App` reads. Change their callers in `src/app/shell/`.
- [ ] 4.5 Move `src/app/infra/terminal.rs` and `src/app/infra/image_fetch/protocol.rs` (with `protocol/protocol_tests.rs`) under `src/app/shell/`. Change the module registrations and imports.
- [ ] 4.6 Move the ratatui parts of `src/app/infra/image_fetch.rs` (the `wide_hero_fits`, `PANE_PAD_X`/`PANE_PAD_Y`, and `HeroArtwork` geometry) under `src/app/shell/`. Fetch and decode stay in `infra`.
- [ ] 4.7 Split `src/app/dispatch/run_loop/teardown.rs`: the shared teardown over `App` stays in `dispatch`, and the part that names `shell::Model` moves under `src/app/shell/`.
- [ ] 4.8 **Gate.** nextest `-p mbv`. `rg -l "ratatui|crossterm|tuirealm" src/app/state src/app/infra --glob '!**/tests/**' --glob '!**/*tests*'` finds nothing. `rg -n "shell::" src/app --glob '!src/app/shell/**' --glob '!src/app/shell.rs' --glob '!**/tests/**'` finds only doc comments.

## 5. Neutral context-menu anchors (design D6)

- [ ] 5.1 In `src/app/dispatch/context_menu/menu.rs` and `menu/emby.rs`, fold `build_queue_selection_menu_at` into `build_queue_selection_menu` and remove every `anchor: Option<(u16, u16)>` parameter.
- [ ] 5.2 Remove the `x`/`y` (`col`/`row`) parameters from the definitions of `open_context_menu_for_at`, `open_context_menu_at`, `open_context_menu_at_for_item`, `open_visual_slot_context_menu`, and `handle_mouse_right_click_queue`, and from their core caller `src/app/dispatch/mouse_gestures.rs`.
- [ ] 5.3 Make `ContextMenuAnchor::Pointer` in `crates/mbv-ui-model/src/context_menu.rs` a unit variant. Change its readers in `mbv-components` (`home_content/keyboard.rs`, `podcast_content.rs`, `book_content.rs`).
- [ ] 5.4 Add `menu_pointer: Option<(u16, u16)>` to the shell `Model`. Make the shell callers in `shell/messages.rs` and `shell/home.rs` store the cell there before they call the core. Make the placement in `shell/overlays/menus.rs` read it.
- [ ] 5.5 Change `src/app/tests/context_menu_placement.rs` and `src/app/tests/context_menu_entries.rs` to the new signatures and to `menu_pointer`.
- [ ] 5.6 **Gate.** nextest `-p mbv -p mbv-ui-model -p mbv-components` (the placement tests own the contract). `rg -nP '\b(x|y|col|row|column): u16\b|\(u16, u16\)|Pointer\s*\{' src/app/dispatch src/app/state src/app/input crates/mbv-ui-model/src/context_menu.rs --glob '!**/tests/**' --glob '!**/*tests*'` finds nothing.

## 6. Client capabilities (design D6a)

- [ ] 6.1 Add `ClientCapability` and `ClientCapabilities` in the flag-set shape of design D6a (not `bool` fields) in a new core state module. Add the stored value to `App` (`state/app_struct.rs`, `state/construct.rs`), and add `App::skip_launch_restore`.
- [ ] 6.2 Change `attach_owner` (`src/owner_link.rs`) to take `ClientCapabilities` in place of `ClientKind`: it builds the first Hello from them and stores them on the new `App`. Change the TUI call in `src/startup.rs` to pass `TerminalPanelRules` and `VisualSlotControls`. Delete `src/client_kind.rs` and its exports in `src/lib.rs`, and the `ClientKind` branch in `src/pin/swap.rs`.
- [ ] 6.3 Make the GUI (`crates/mbv-gui/src/attach.rs`, `core_thread.rs`) pass `ShownQueueEntries` and `TransportChipKeys`, call `pin::claim_pin_swap_participant(false)` itself before `attach_owner`, and call `skip_launch_restore` in place of `enable_gui_client`. Make `client_hello_claims` and the reconnect Hello (`dispatch/session/connect.rs`, `dispatch/session/daemon_restart.rs`) read the stored capabilities.
- [ ] 6.4 Map the panel and visual-slot readers by the D6a table: `is_mini_view` and `queue_column_hidden_empty` (`state/panel_focus.rs`) read `TerminalPanelRules`. The visualizer and visual-slot setters (`infra/visualizer.rs`, `state/playback_target.rs`, `shell/arbitration.rs`, `state/app_struct/waker.rs`) read `VisualSlotControls`.
- [ ] 6.5 Delete `gui_client`, `enable_gui_client`, `client_arranges_panels`, `visual_slot_controls()`, `App::client_kind`, and `set_client_kind` from `state/app_struct.rs`, `state/construct.rs`, `gui_client_state.rs`, and `gui_client_state/queue_controls.rs`.
- [ ] 6.6 **Sweep.** Rename `gui_shown_entry` to `shown_entry` in every file.
- [ ] 6.7 Make the shown-queue gates in `state/named_queues.rs`, `state/queue_link.rs`, and `state/playback.rs` read `ShownQueueEntries`. Keep the closed-queue fallback in `adopt_named_queue_list` in the core.
- [ ] 6.8 In `input/key_policy.rs`, `input/key_policy/snapshot.rs`, and `input/gui.rs`, rename `KeyPolicyGate::GuiClient` and `GuiQueueTarget` to `TransportChipKeys` and `TransportChipQueueTarget`, and make the router snapshot carry `ClientCapabilities` in place of `gui_client`.
- [ ] 6.9 **Sweep.** Change the test files that set `gui_client`, call `enable_gui_client`, or name `ClientKind` to build `ClientCapabilities` or call `skip_launch_restore`.
- [ ] 6.10 **Gate.** nextest `-p mbv -p mbv-gui` (contract: spec `gui-queue-chooser` and ADR 0041). `rg -n "gui_client\b|enable_gui_client|client_arranges_panels|ClientKind|gui_shown_entry|KeyPolicyGate::Gui" src crates/mbv-gui/src` finds nothing.

## 7. Terminal size, single-panel fact, and the tab strip (design D2, D4, D6a, D6b)

- [ ] 7.1 Add `App::set_single_panel(bool)` and make `is_mini_view` (`state/panel_focus.rs`) read that fact. Make the TUI push it, from its width and `MINI_VIEW_THRESHOLD`, at every point where it records the terminal size today: on resize, and in the per-frame size on the draw path (`shell/draw.rs`), so the first frame already reads it. Make `dispatch/action.rs` read the fact in place of `MINI_VIEW_THRESHOLD` and `terminal_width`.
- [ ] 7.2 Move the tab strip to the shell (design D6b): `tab_scroll`, `ensure_tab_visible`, `visible_tab_range`, and the tab-strip width rule of `src/app/input.rs`. Add `tab_reveal_requested` to `App`, set it in `dispatch/library/cw_library_tab.rs` in place of the call, and make the shell take it after every update step and tick drain, before it paints or resolves a pointer press.
- [ ] 7.3 Move `terminal_width` and `terminal_height` from `state/app_struct.rs` and `state/construct.rs` to the shell `Model`. Move the reader `normalize_queue_column_width` of `app_struct.rs` to the shell. Change the readers in `gui_client_state.rs`. A reader outside the shell that rows 4.x, 7.1, or 7.2 did not remove is a premise error: stop and report it.
- [ ] 7.4 **Sweep.** Change every shell site from `app.terminal_width` / `app.terminal_height` to the `Model` fields.
- [ ] 7.5 Move the other `App` fields that only `src/app/shell/` reads or writes onto the shell `Model` (design D2, D4). Before dispatch, list these fields. If the list holds more than six fields, split this row by field.
- [ ] 7.6 Add `save_client_pref` to the core (design D4). Move the prefs keys of the fields that 7.3 and 7.5 moved to the shell, so the shell writes them with `save_client_pref`, and remove them from the core's `save_prefs`.
- [ ] 7.7 **Sweep.** Change the tests that set `terminal_width` or `terminal_height` on `App` (`tests/panel_focus.rs`, `tests/narrow_browse_migration.rs`, `tests/pinned_view_toggle.rs`, `tests/tick_integration/**`, `input/music_track_scope_tests.rs`, and the others that `rg` finds) to set the size on the `Model` or through `set_single_panel`.
- [ ] 7.8 **Gate.** nextest `-p mbv -p mbv-gui`, with the Mini view tests and the prefs round-trip tests unchanged. `rg -n "terminal_width|terminal_height" src/app --glob '!src/app/shell/**' --glob '!src/app/shell.rs'` finds nothing.

## 8. Layout and the remaining geometry readers (design D13 Rule B)

- [ ] 8.1 Move `App::layout` (`AppLayout`) from `state/app_struct.rs` and `state/construct.rs` to the shell `Model`. Add `left_panel_area: mbv_ui_model::geom::Rect` with `App::set_left_panel_area`. `compose_root_frame` (`shell/draw.rs`) sets it with the value it writes to `layout.left_area` (zero under 4 rows). Make `lib_page_size` (`dispatch/actions.rs`) and the visualizer menu bounds (`dispatch/context_menu/menu.rs`, which then drops `rect_to_presentation`) read it.
- [ ] 8.2 **Sweep.** Change the tests that read `app.layout` (`tests/context_menu_placement.rs`, `tests/tick_integration.rs`, `tests/tick_integration/{mouse,queue_playback}.rs`, `tests/tick_integration/mouse/panels.rs`) to the `Model` field.
- [ ] 8.3 **Gate.** nextest `-p mbv`. `rg -n "AppLayout|\.layout\b" src/app --glob '!src/app/shell/**' --glob '!src/app/shell.rs' --glob '!**/tests/**' --glob '!**/*tests*'` finds nothing. `rg -l "mbv_render|mbv_components" src/app --glob '!src/app/shell/**' --glob '!src/app/shell.rs' --glob '!**/tests/**' --glob '!**/*tests*'` finds only `src/app/test_seams.rs`.

## 9. Neutral events at the router (design D3)

- [ ] 9.1 Move the crossterm key and mouse translation out of `src/app/input/resolver.rs` and `src/app/input/key_policy/test_support.rs` to the TUI edge in `src/app/shell/`, so the router takes only neutral events.
- [ ] 9.2 Split `src/app/test_seams.rs`: `crossterm_key_to_neutral` and every other seam that names crossterm, tuirealm, or `mbv-components` move to a seam module under `src/app/shell/`. The toolkit-free seams stay.
- [ ] 9.3 Split `src/app/tests.rs`: its ratatui and crossterm helpers (`TestBackend`, `Terminal`, the crossterm `confirm_replace_queue`) move to a shell test-support module. The toolkit-free helpers stay.
- [ ] 9.4 **Sweep.** Make the core tests that build keys through `crossterm_key_to_neutral` (`src/app/input/**` tests, `dispatch/action/tests.rs`, `dispatch/actions/tests/replacement_gate.rs`, `dispatch/actions/route_tests/playback.rs`, and the other users that `rg` finds) build neutral keys directly. Change the shell-driving tests to the moved seam path.
- [ ] 9.5 **Gate.** nextest `-p mbv`. `rg -l "crossterm|tuirealm|mbv_components|ratatui" src/app --glob '!src/app/shell/**' --glob '!src/app/shell.rs' --glob '!**/tests/**' --glob '!**/*tests*'` finds nothing.

## 10. The core calls no GUI function (design D4a)

- [ ] 10.1 GUI half, commands. Make the D4a *GUI half* arms of `App::dispatch` (`dispatch/action.rs`: `toggle_gui_queue_chooser`, `toggle_gui_target_chooser`, `toggle_gui_track_popover`, `toggle_panel_collapse`, `toggle_queue_fold`) documented core no-ops. Make the `Command::App` arm of `input/gui.rs` call the core first and then the GUI method.
- [ ] 10.2 GUI half, playback requests. Do the same for the arms of `App::handle_playback_request` (`shell/playback.rs`: `open_gui_player_menu`, `select_gui_player_menu_entry`, `select_gui_track_popover_track`, `show_gui_queue_chooser_entry`, `toggle_panel_collapse`, `toggle_queue_fold`) and the `Msg::Playback` arm of `input/gui.rs`.
- [ ] 10.3 Core, named queues. Move `request_new_gui_queue` (renamed `request_new_queue`), `request_rename_shown_queue`, `request_close_shown_queue`, `take_new_queue_answer`, `report_named_queue_rejection`, and `send_named_queue_close` from `gui_client_state/queue_controls.rs` to `state/named_queues.rs`. Give each one the GUI fact that it reads as a parameter or a core field. Change the core callers (`dispatch/action.rs`, `dispatch/session/player_event.rs`, `client_drains.rs`, `input/confirm_keys.rs`).
- [ ] 10.4 Core, filter and target. Move `activate_queue_filter` (`gui_client_state.rs`) and `select_chooser_target` (`gui_client_state/target.rs`) to core modules beside the code they serve. Change the callers in `dispatch/action.rs`, `shell/playback.rs`, and `shell/messages/navigation.rs`.
- [ ] 10.5 Add `ClientLibraryStores` and `ClientLibraryEffect` to the core (design D4a (c)). Give `handle_lib_event`, `drain_lib_event`, `handle_browse_event`, and `handle_music_event` the `stores` parameter. Collect each event's effects and apply them after that event and before the next. Add the transitional unit implementor in a GUI module, which reads and writes the `gui_*` fields through its `app` parameter. Make the GUI drain pass it, and make the TUI drain pass `None`.
- [ ] 10.6 Replace the `gui_queue_in_view_row_fetches` calls in `dispatch/library/event.rs` with `ClientLibraryEffect::RowArtSourcesChanged`. Make the transitional implementor call the method.
- [ ] 10.7 Feed target. Make `feed_home_video_selection_wanted` (`dispatch/feeds/feed.rs`) ask `wants_feed_target` in place of `gui_emby_feed_wants_target`. Split `handle_feed_home_video_selection_failed` (`gui_library/emby.rs`): the fetch-mark retirement and the log stay in the core, and the claim insert becomes `FeedTargetFailed`. The claim retirement in `handle_feed_home_video_selection_loaded` becomes `FeedTargetLanded`.
- [ ] 10.8 Move the App half of the artist-name lookup (`record_artist_item_id`, `handle_gui_artist_name_searched` in `gui_library/artist_lookup.rs`) to `dispatch/library/` as `ArtistNameLookup`, with its answer still gated on client presence.
- [ ] 10.9 **Gate.** nextest `-p mbv -p mbv-gui`. `rg -n "\.(toggle_gui_queue_chooser|toggle_gui_target_chooser|toggle_gui_track_popover|toggle_panel_collapse|toggle_queue_fold|open_gui_player_menu|select_gui_player_menu_entry|select_gui_track_popover_track|show_gui_queue_chooser_entry)\(" src/app --glob '!src/app/gui_*' --glob '!src/app/gui_*/**' --glob '!src/app/input/gui*' --glob '!src/app/input/gui/**' --glob '!**/tests/**' --glob '!**/*tests*'` finds nothing. `rg -n "\bgui_[a-z_]+|\bGui[A-Z]" src/app/dispatch src/app/client_drains.rs src/app/state --glob '!**/tests/**' --glob '!**/*tests*'` finds only the `gui_*` fields in `app_struct.rs` and `construct.rs` (group 11 moves them), the three D5 event variants in `events.rs` with their arms in `dispatch/library/event.rs` (row 15.6 moves them), and `gui_queue_end_offsets` (`2-split-tui-gui-clients` renames it).

## 11. `GuiClientState` (design D4)

- [ ] 11.1 Define `GuiClientState` in a GUI module with the 24 `gui_*` fields that remain after group 6, plus `panel_collapsed` and `queue_folded`. Put it on `App` as one transitional field `gui`. Delete the fields from `state/app_struct.rs` and `state/construct.rs`. Move the `gui_overlay` accessors onto `GuiClientState`.
- [ ] 11.2 **Sweep.** Change every `self.gui_<name>` / `app.gui_<name>` site to `self.gui.<name>` / `app.gui.<name>`, and do the same for `panel_collapsed` and `queue_folded`, in `src` and `crates/mbv-gui/src`.
- [ ] 11.3 **Gate.** nextest `-p mbv -p mbv-gui`. `rg -n "\b(self|app)\.gui_[a-z]|\b(self|app)\.(panel_collapsed|queue_folded)\b" src crates/mbv-gui/src` finds nothing.

## 12. `GuiCore`, overlays, and the GUI input entry (design D4)

In the rows of groups 12–14, convert the `impl App` blocks of the named files to `impl GuiCore`. Inside a converted block, GUI state is `self.app.gui.<name>` (transitional) and core state is `self.app.<...>`. The files stay where they are. Move each block's tests with it. A `test`-feature seam in `app_stub.rs` or `app_stub/test_reads*` that calls a converted method or reads GUI state moves to a seam module beside `GuiCore` in the row that converts its callee.

- [ ] 12.1 Add `GuiCore { app: App }` in `src/gui.rs` (outside `crate::app`) and export it from `src/lib.rs`. Make the GUI's `CoreThread` (`crates/mbv-gui/src/core_thread.rs`, `attach.rs`) hold it in place of `App`.
- [ ] 12.2 **Sweep.** Make the `mbv-gui` tests and benches that build an `App` stub (`tests/core_thread*.rs`, `tests/core_thread/**`, `tests/attach.rs`, `tests/startup.rs`, `tests/bridge/frame*.rs`, `benches/sections.rs`) wrap it in `GuiCore`.
- [ ] 12.3 Convert `input/gui.rs`, `gui_client_state.rs`, `gui_client_state/overlay.rs`, `gui_client_state/popover.rs`, and `gui_client_state/player_menu.rs`.
- [ ] 12.4 Convert `gui_client_state/{queue_chooser,target,row_menu,confirm,daemon_lost,queue_controls,projection}.rs`.
- [ ] 12.5 **Gate.** nextest `-p mbv -p mbv-gui`.

## 13. `GuiCore`, settings, playlists, and the library root

- [ ] 13.1 Convert `gui_settings.rs`, `gui_settings/command.rs`, `gui_settings/projection.rs`, and `gui_settings/sections.rs`.
- [ ] 13.2 Convert `gui_settings/{keys,reset,services,sessions,feeds}.rs`, `gui_playlists.rs`, and `gui_playlists/commands.rs`.
- [ ] 13.3 Convert `gui_library.rs` (`apply_gui_library_command`, `open_gui_library_position`, the tab step), `gui_library/launch.rs`, and `gui_library/ensure.rs`.
- [ ] 13.4 **Gate.** nextest `-p mbv -p mbv-gui`.

## 14. `GuiCore`, library destinations

- [ ] 14.1 Convert `gui_library/shows_projection.rs`, `gui_library/shows_projection/hero.rs`, and `gui_library/audiobooks_projection.rs`.
- [ ] 14.2 Convert `gui_library/{emby_projection,item_menu,rows_in_view,marks}.rs`.
- [ ] 14.3 Convert `gui_library/{podcasts_projection,podcasts,item_actions}.rs`.
- [ ] 14.4 Convert `gui_library/{feeds_projection,feeds,audiobooks}.rs`.
- [ ] 14.5 Convert `gui_library/{shows,tv,emby,search}.rs`.
- [ ] 14.6 Convert `gui_library/{projection,store,list_sort}.rs`.
- [ ] 14.7 Convert `gui_library/{home,artwork,collection_command,tabs,artist_lookup}.rs` and `gui_library/artwork/rows.rs`.
- [ ] 14.8 **Gate.** nextest `-p mbv -p mbv-gui`. `rg -n "^impl (crate::app::)?App\b" src/app/gui_* src/app/input/gui*` finds nothing.

## 15. GUI state leaves `App`, client events, and the router entry (design D4, D5)

- [ ] 15.1 Move the `gui` field from `App` to `GuiCore { app, gui }` and delete it from `App`.
- [ ] 15.2 **Sweep.** Change `self.app.gui.` to `self.gui.` in every converted block and seam.
- [ ] 15.3 Make `GuiClientState` implement `ClientLibraryStores`. Delete the transitional implementor, and make the GUI's `drain_lib_events` pass `Some(&mut self.gui)`.
- [ ] 15.4 Make the GUI read `gui_player_auto_hide` from `load_prefs` at start and write it with `save_client_pref`. Remove that key from the core's `save_prefs`, and move the existing auto-hide prefs round-trip test with it.
- [ ] 15.5 Add `drain_client_events(&mut self) -> bool` to `ClientTickHost`, called in the shared tick after `drain_lib_events`. Implement it in the TUI shell as a documented no-op that returns `false`.
- [ ] 15.6 Remove `GuiGroupAlbums`, `GuiEmbyRootItems`, and `GuiShowsNew` from the shared event enum (`state/events.rs`) and from `dispatch/library/event.rs`. Send those worker results on a channel that `GuiCore` owns and its `CoreThread` drains in `drain_client_events`, with the same `ClientWaker`. Move the tests of those events with them.
- [ ] 15.7 Give the router a `pub` entry that takes a snapshot of facts and returns a `RouterOutcome`, with the prefix state read through `pub` methods, so the GUI routing entry no longer reads router fields. Precedence, order, and chords stay only in `src/app/input/router.rs`, `key_policy.rs`, and `resolver.rs`.
- [ ] 15.8 **Gate.** nextest `-p mbv -p mbv-gui`. `rg -n "\.app\.gui\b|gui: GuiClientState" src/app`, `rg -n "GuiGroupAlbums|GuiEmbyRootItems|GuiShowsNew" src/app/state/events.rs src/app/dispatch`, and `rg -n "self\.app\.(router|key_policy|prefix)" src/app/input/gui.rs src/app/input/gui` find nothing.

## 16. Docs

- [ ] 16.1 Write `docs/adr/0042-the-client-core-names-no-client.md` (follow `docs/adr/README.md`). Record design D4 (each Client wraps `App`), D4a (the core calls no GUI function), D5, D6 (neutral signatures), and D6a (capabilities, not a Client kind). Add *Client capability* to `CONTEXT.md` with its *Avoid* line (`client kind`, `gui_client`). Verify that `rg -n "ClientKind|gui_client\b" CONTEXT.md docs --glob '!docs/adr/**' --glob '!docs/plans/**'` finds nothing.
- [ ] 16.2 Update `docs/architecture/gui-client.md` for `GuiCore`, `GuiClientState`, `ClientLibraryStores`, and the client event channel.
- [ ] 16.3 Update `docs/architecture/tui-frontend.md` for `menu_pointer`, the single-panel push, `left_panel_area`, and the tab-reveal request, and `docs/recipes.md` where a site list names a moved symbol. Verify that `rg -n "gui_shown_entry|enable_gui_client|client_arranges_panels|ensure_tab_visible" docs/architecture docs/recipes.md AGENTS.md` finds nothing that names the old core site.

## 17. Integration checks

- [ ] 17.1 Run `cargo tree -e normal --prefix none` for `-p mbv-theme`, `-p mbv-images`, and `-p mbv-ui-model`, and confirm that none lists `ratatui`, `ratatui-image`, `crossterm`, or `tuirealm`.
- [ ] 17.2 Run the full gate set: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --release --test-threads=4`, and `cargo nextest run --release --test-threads=4 -p mbv-gui`. Run `make check-code-file-lines` just before the push, not as row acceptance.
- [ ] 17.3 Run `openspec validate 1-decouple-client-core` and confirm the delta specs still match the built behaviour.

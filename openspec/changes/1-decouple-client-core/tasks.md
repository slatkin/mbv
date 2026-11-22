# Tasks

## Execution rules

Each checkbox is a bounded edit task, not a compile-green unit. A task is complete when its named edits are present and inspected. Compilation, tests, lint gates, and a green workspace are NOT completion conditions for edit tasks or groups. Dependent callers and tests may remain temporarily broken between tasks. Record remaining dependent work by task ID; never expand the current assignment to repair it.

Dispatch one row or less to a fresh worker. Never combine rows to make a build pass. A directory name limits where to look; it never authorizes converting the whole directory. Where a row names one module, edit that production module and its direct module registration or export only. A caller migration, test migration, or dependency cleanup is separate unless the row explicitly includes it. Before dispatch, list exact permitted files and symbols from the current source. Split a row further if that inventory contains more than three substantive files or more than one independent responsibility. Do not hide multiple dispatches inside one worker assignment.

Verification for edit rows is source inspection of the named deliverable and its diff, as stated in each row. A search result inventories work; it does not authorize fixing every match. If inspection finds an unlisted caller, field, or module, add a separate bounded follow-up row here before dispatching it. Do not turn a remainder audit into a repair task. These follow-ups must obey the same sizing rule.

Run every cargo command in the foreground and wait. Do not background it, sleep, or poll. Run `cargo fmt` after Rust edits; formatting does not make compilation a task-completion condition. Move files with `git mv` where applicable. Keep the module layout rules in `AGENTS.md`: no `mod.rs`, `#[path]`, or `include!`. Add no lint suppression. Preserve existing test expectations. Move a test with the private code it owns when they share a file; separate test modules and test consumers have their own rows below. Add no tests except for a named contract that lacks an existing owner.

Source and consumer tasks follow dependency order, but a later task does not require an earlier build or gate to pass. The integration section checks the assembled result; it is not a prerequisite for marking edit rows complete. Do not run `make check-code-file-lines` as task acceptance; run it only immediately before a push, as `AGENTS.md` requires.

## 1. TUI support crate and colour conversion (old 1.1–1.2)

- [x] 1.1 Create the `mbv-tui-support` manifest and library root and register the crate in workspace members and default-members. Use workspace version, edition, and lints; `ratatui` 0.30, `ratatui-image` 11 with `crossterm`, `mbv-theme`, and `mbv-images`, as in D7. Verify the manifest entries and module root by inspection; no build is required.
- [x] 1.2 Define `ToRatatui` in `mbv-tui-support`, preserving the conversion results of `mbv-theme/src/rgb.rs`. Verify the old and new conversion arms correspond; leave removal of the old API to 1.6.
- [x] 1.3 Move the conversion's existing separate tests to the new owner. Verify the assertions are preserved; do not add cases or require execution.
- [x] 1.4 Inventory colour-conversion callers in `mbv-render` and add one migration row per caller module below this row, with at most one manifest-wiring row. Verify every current caller has a named row; do not migrate callers in this inventory task.
- [ ] 1.4a Switch each caller module of `crates/mbv-render/src/components/widgets.rs` to `ToRatatui` and `mbv_tui_support::ToRatatui`. Verify the file's 20 conversion sites resolve to the new owner and the painted colours are unchanged; do not delete the old `mbv-theme` API here.
- [ ] 1.4b Switch each caller module of `crates/mbv-render/src/components/tree_browser.rs` to `ToRatatui`. Verify the file's 18 conversion sites resolve to the new owner and its `ratatui::style::Color` values are unchanged.
- [ ] 1.4c Switch each caller module of `crates/mbv-render/src/components/settings_component.rs` to `ToRatatui`. Verify the file's 15 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4d Switch each caller module of `crates/mbv-render/src/components/playlists.rs` to `ToRatatui`. Verify the file's 15 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4e Switch each caller module of `crates/mbv-render/src/components/media_list/row.rs` to `ToRatatui`. Verify the file's 15 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4f Switch each caller module of `crates/mbv-render/src/components/chrome_player/title/transport.rs` to `ToRatatui`. Verify the file's 13 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4g Switch each caller module of `crates/mbv-render/src/components/search_sidebar.rs` to `ToRatatui`. Verify the file's 11 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4h Switch each caller module of `crates/mbv-render/src/components/indicators.rs` to `ToRatatui`. Verify the file's 11 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4i Switch each caller module of `crates/mbv-render/src/components/feeds_manage.rs` to `ToRatatui`. Verify the file's 11 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4j Switch each caller module of `crates/mbv-render/src/components/library_routes.rs` to `ToRatatui`. Verify the file's 10 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4k Switch each caller module of `crates/mbv-render/src/components/three_line_flat_list.rs` to `ToRatatui`. Verify the file's 9 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4l Switch each caller module of `crates/mbv-render/src/components/chrome_player/title/queue_band.rs` to `ToRatatui`. Verify the file's 9 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4m Switch each caller module of `crates/mbv-render/src/components/chrome_tabs.rs` to `ToRatatui`. Verify the file's 8 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4n Switch each caller module of `crates/mbv-render/src/components/chrome_player.rs` to `ToRatatui`. Verify the file's 7 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4o Switch each caller module of `crates/mbv-render/src/components/multiselect.rs` to `ToRatatui`. Verify the file's 6 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4p Switch each caller module of `crates/mbv-render/src/components/daemon_lost_modal.rs` to `ToRatatui`. Verify the file's 6 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4q Switch each caller module of `crates/mbv-render/src/components/chrome.rs` to `ToRatatui`. Verify the file's 6 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4r Switch each caller module of `crates/mbv-render/src/components/confirm_modal.rs` to `ToRatatui`. Verify the file's 5 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4s Switch each caller module of `crates/mbv-render/src/components/chrome_player/title/header.rs` to `ToRatatui`. Verify the file's 5 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4t Switch each caller module of `crates/mbv-render/src/components/twitch_url.rs` to `ToRatatui`. Verify the file's 4 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4u Switch each caller module of `crates/mbv-render/src/components/sessions.rs` to `ToRatatui`. Verify the file's 4 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4v Switch each caller module of `crates/mbv-render/src/components/context_menu.rs` to `ToRatatui`. Verify the file's 4 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4w Switch each caller module of `crates/mbv-render/src/components/queue.rs` to `ToRatatui`. Verify the file's 3 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4x Switch each caller module of `crates/mbv-render/src/components/queue_playback.rs` to `ToRatatui`. Verify the file's 3 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4y Switch each caller module of `crates/mbv-render/src/components/help.rs` to `ToRatatui`. Verify the file's 3 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4z Switch each caller module of `crates/mbv-render/src/components/chrome_status_bar.rs` to `ToRatatui`. Verify the file's 3 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4aa Switch each caller module of `crates/mbv-render/src/components/media_list/wide.rs` to `ToRatatui`. Verify the file's 2 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4ab Switch each caller module of `crates/mbv-render/src/components/media_list.rs` to `ToRatatui`. Verify the file's 2 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4ac Switch each caller module of `crates/mbv-render/src/components/hero.rs` to `ToRatatui`. Verify the file's 2 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4ad Switch each caller module of `crates/mbv-render/src/components/chrome_player/title/title_row.rs` to `ToRatatui`. Verify the file's 2 conversion sites resolve to the new owner and the painted colours are unchanged.
- [ ] 1.4ae Switch each caller module of `crates/mbv-render/src/components/modal_frame.rs` to `ToRatatui`. Verify the module's 1 conversion site resolves to the new owner and the painted colour is unchanged.
- [ ] 1.4af Switch each caller module of `crates/mbv-render/src/components/library_hero_overlay.rs` to `ToRatatui`. Verify the module's 1 conversion site resolves to the new owner and the painted colour is unchanged.
- [ ] 1.4ag Switch each caller module of `crates/mbv-render/src/components/card.rs` to `ToRatatui`. Verify the module's 1 conversion site resolves to the new owner and the painted colour is unchanged.
- [ ] 1.4ah Switch each caller in `crates/mbv-render/src/components/artwork_placeholder.rs` to `ToRatatui`. Verify the module's 1 conversion site resolves to the new owner and the painted colour is unchanged.
- [ ] 1.4ai Switch each caller in `crates/mbv-render/src/arrangements/wide_hero.rs` to `ToRatatui`. Verify the module's 1 conversion site resolves to the new owner and the painted colour is unchanged.
- [ ] 1.4aj Adapt the separate test module `crates/mbv-render/src/components/chrome_tabs/tests.rs` to `ToRatatui`. Verify its 2 conversion sites resolve to the new owner and the existing assertions are unchanged; do not add cases.
- [ ] 1.4ak Adapt the separate test module `crates/mbv-render/src/components/card/tests.rs` to `ToRatatui`. Verify its 1 conversion site resolves to the new owner and the existing assertions are unchanged; do not add cases.
- [ ] 1.4al Wire the manifest for the `mbv-render` conversions above: add `mbv-tui-support` to `crates/mbv-render/Cargo.toml` and drop the `mbv-theme` `ratatui` feature entry. Verify the new dependency is declared once and no other manifest entry changes; this row follows 1.4a–1.4ak.
- [ ] 1.5 Inventory colour-conversion callers in `mbv-components` and the root package and add one migration row per caller module below this row, with separate manifest-wiring rows where needed. Verify every current caller has a named row; do not migrate callers in this inventory task.
- [ ] 1.6 Remove `to_ratatui`, the ratatui feature, and the ratatui dependency from `mbv-theme` after the caller edit rows. Verify those definitions are absent by inspection; record remaining callers rather than repairing them here.

## 2. Terminal image support (old 1.3)

- [ ] 2.1 Inventory terminal protocol state and methods in `mbv-images/src/{lib,cache,protocol,resize}.rs`, their ownership, and their consumers. Add separate edit rows for any consumer module not already covered below. Verify the inventory separates shared pixels/cache data from terminal protocol data; do not move code in this task.
- [ ] 2.2 Separate the terminal protocol fields from the shared image data in `mbv-images/src/lib.rs`, defining the terminal-side data in `mbv-tui-support`. Verify shared data no longer names `ThreadProtocol`; leave consumer adaptation to separate rows.
- [ ] 2.3 Separate picker and protocol storage from `mbv-images/src/cache.rs` into the terminal support owner. Verify decoded/source cache responsibilities remain in `mbv-images`; do not convert its external callers here.
- [ ] 2.4 Move the terminal methods of `mbv-images/src/protocol.rs` to the terminal support owner. Verify the methods retain their behaviour and now use the split owners; do not fix external callers in this task.
- [ ] 2.5 Move the terminal resize worker in `mbv-images/src/resize.rs` and its colocated tests to terminal support. Verify request handling and existing test assertions are preserved.
- [ ] 2.6 Move `RENDER_FILTER` and remove obsolete terminal module exports and dependencies from `mbv-images`. Verify decode, cache keys, pixel processing, and source-image APIs remain there; do not broaden into consumer fixes.
- [ ] 2.7 Audit image-protocol consumers in `mbv-render` after the moves and ensure one named migration row exists per consumer module. Verify inventory coverage only; migration runs in those separate rows.
- [ ] 2.8 Audit image-protocol consumers in the root package after the moves and ensure one named migration row exists per consumer module. Verify inventory coverage only; migration runs in those separate rows.

## 3. Shared items from mbv-render (old 1.4)

For each definition move below, its inline private tests move with it. Export registration is allowed; caller conversion is not. Task 3.9 schedules callers separately.

- [ ] 3.1 Move `current_time_secs` to `mbv-ui-model`. Verify the implementation and exported signature are preserved.
- [ ] 3.2 Move `group_album_plan` and its plan data to `mbv-ui-model`. Verify its ordering rules are unchanged; leave unrelated sort helpers to 3.3.
- [ ] 3.3 Move `sorted_group_album_order`, `initial_group_artist_sort_key`, and `effective_sort_str` to `mbv-ui-model`. Verify the existing sort semantics are preserved.
- [ ] 3.4 Move `book_rows` to `mbv-ui-model`. Verify the row data and ordering are unchanged.
- [ ] 3.5 Move `HeroImageState` to `mbv-ui-model`. Verify its variants and payloads are preserved.
- [ ] 3.6 Move `MUSIC_ALBUM_IMAGE_TYPES` to `mbv-ui-model`. Verify values and order are unchanged.
- [ ] 3.7 Move the neutral data of `IndicatorStyle` and `StatusIndicator` to `mbv-ui-model`; keep ratatui conversion in `mbv-render`. Verify the new shared definitions name no terminal type.
- [ ] 3.8 Move `TransportAvailability` to `mbv-ui-model`. Verify its fields and semantics are unchanged.
- [ ] 3.9 Inventory consumers of 3.1–3.8 across the root package, `mbv-render`, `mbv-components`, and `mbv-gui`; add one import/call migration row per module and separate manifest rows. Verify every old import has a scheduled owner; do not migrate callers here.
- [ ] 3.10 Inventory separate test modules owned by 3.1–3.8 and add one move/adaptation row per test module. Verify assertions and ownership are recorded; do not run tests or change expectations here.

## 4. Shared hero items from mbv-components (old 1.5)

Caller migrations are separate from these definition moves.

- [ ] 4.1 Move the neutral artwork types `HeroArtwork`, `ArtworkSource`, and `ArtworkShape` to `mbv-ui-model`. Verify no painting or toolkit value moves into the shared crate.
- [ ] 4.2 Move `HeroFacts`, `PanelHeroImagePaint`, and `HeroContentData` to `mbv-ui-model`. Verify their data shape is preserved and any toolkit conversion stays in the TUI crate.
- [ ] 4.3 Move `painted_shape_for_pixels` and `HeroArtwork::painted_shape` to the neutral artwork owner. Verify the shape rules are unchanged.
- [ ] 4.4 Move `hero_content_queue` to `mbv-ui-model`. Verify it builds the same content data.
- [ ] 4.5 Move `hero_content_feed` and `feed_artwork_policy` to `mbv-ui-model`. Verify feed content and artwork policy are unchanged.
- [ ] 4.6 Move `hero_content_emby` and `emby_artwork_policy` to `mbv-ui-model`. Verify Emby content and artwork policy are unchanged.
- [ ] 4.7 Move `hero_content_abs_episode` and `abs_episode_artwork_policy` to `mbv-ui-model`. Verify episode content and artwork policy are unchanged.
- [ ] 4.8 Move `hero_content_abs_book` and `abs_book_artwork_policy` to `mbv-ui-model`. Verify book content and artwork policy are unchanged.
- [ ] 4.9 Move `hero_content_series_with_episode` to `mbv-ui-model`. Verify the series/episode content combination is unchanged.
- [ ] 4.10 Move `premiere_number_label` to `mbv-ui-model`. Verify the formatting rules are unchanged.
- [ ] 4.11 Move `sanitize_url` to `mbv-ui-model`. Verify the filtering rules are unchanged.
- [ ] 4.12 Inventory consumers of 4.1–4.11 and add one import/call migration row per module, plus separate manifest rows. Verify every old import has a scheduled migration; do not convert callers in this inventory task.
- [ ] 4.13 Inventory separate tests owned by 4.1–4.11 and add one move/adaptation row per test module. Verify each existing contract keeps one owner; do not add coverage or require tests to pass here.

## 5. TUI projections and infrastructure (old 2.1–2.3)

Each row moves one source module or responsibility. Direct registrations may change; external caller edits get separate rows through 5.9.

- [ ] 5.1 Move the ratatui part of `state/projection/card.rs` to shell functions over core reads. Verify toolkit-free logic remains in state and inline tests follow their owner.
- [ ] 5.2 Move the ratatui part of `state/projection/chrome_status.rs` to shell functions. Verify toolkit-free logic remains in state.
- [ ] 5.3 Move the ratatui part of `state/projection/tv_wide.rs` to shell functions. Verify toolkit-free logic remains in state.
- [ ] 5.4 Move the ratatui part of `state/projection/visualizer.rs` to shell functions. Verify toolkit-free logic remains in state.
- [ ] 5.5 Convert the `impl App` frame-composition methods in `shell/draw.rs` into shell functions. Verify they retain the same frame composition; field ownership changes are separate tasks below.
- [ ] 5.6 Move `infra/terminal.rs` under the shell. Verify the terminal lifecycle implementation is preserved.
- [ ] 5.7 Move `infra/image_fetch/protocol.rs` under the shell. Verify protocol conversion is no longer defined in shared infrastructure.
- [ ] 5.8 Split the ratatui responsibilities from `infra/image_fetch.rs` into the shell. Verify shared fetch/decode responsibilities remain in infrastructure.
- [ ] 5.9 Inventory callers and separate tests affected by 5.1–5.8; add one adaptation row per module. Verify every affected module is scheduled without converting it here.
- [ ] 5.10 Split `dispatch/run_loop/teardown.rs`: retain shared teardown over `App` and move the `shell::Model` responsibility under the shell. Verify the boundary by source inspection; do not repair all call sites here.
- [ ] 5.11 Adapt the teardown entry caller to the split from 5.10. Verify call order is preserved; schedule any additional caller as its own row.

## 6. Neutral context-menu anchors (old 2.4)

- [ ] 6.1 Add shell-owned `menu_pointer` storage and capture the existing pointer cells at the shell message entry. Verify the stored cells are the ones passed to core calls today; leave shared signatures unchanged in this row.
- [ ] 6.2 Change shell menu placement to read `menu_pointer`. Verify placement uses the same recorded pointer; do not remove shared anchor fields here.
- [ ] 6.3 Make `ContextMenuAnchor::Pointer` a unit variant in `mbv-ui-model`. Verify the enum no longer carries terminal coordinates; consumer breakage is allowed.
- [ ] 6.4 Fold `build_queue_selection_menu_at` into `build_queue_selection_menu` and remove pointer-anchor parameters in `dispatch/context_menu/menu.rs`. Verify menu content and target resolution are preserved.
- [ ] 6.5 Remove the anchor parameter from `dispatch/context_menu/menu/emby.rs`. Verify Emby menu content is unchanged.
- [ ] 6.6 Remove pointer-cell parameters from the shared `open_context_menu_for_at`, `open_context_menu_at`, and `open_context_menu_at_for_item` definitions. Verify these entry points preserve semantic targets; split this row before dispatch if the definitions exceed the file bound.
- [ ] 6.7 Remove pointer-cell parameters from the shared visual-slot and queue-right-click entry definitions. Verify they keep semantic targets and do not store cells.
- [ ] 6.8 Adapt menu calls in `shell/messages.rs` to the neutral signatures. Verify pointer storage occurs before opening the menu.
- [ ] 6.9 Adapt menu calls in `shell/home.rs` to the neutral signatures. Verify pointer storage occurs before opening the menu.
- [ ] 6.10 Inventory remaining anchor consumers, including component consumers, and add one adaptation row per module. Verify every old pointer payload or cell argument is scheduled; do not fix all matches here.
- [ ] 6.11 Adapt `app/tests/context_menu_placement.rs` to the new shell pointer owner. Verify its existing placement expectations are unchanged; passing the test is not required to complete these edits.

## 7. Client capabilities (old 2.5–2.6)

- [ ] 7.1 Define `ClientCapability` and the `ClientCapabilities(u8)` flag set with `with` and `has`, as in D6a. Verify it is not a struct of bool fields and declares the four named capabilities.
- [ ] 7.2 Add stored capabilities to `App` and its constructor. Verify storage is initialized; leave removal of the old Client-kind fields to 7.13.
- [ ] 7.3 Change `attach_owner` to receive capabilities and build its first Hello from them. Verify the stored value is set before attach returns; leave launch callers to 7.4 and 7.5.
- [ ] 7.4 Make the TUI attach caller declare `TerminalPanelRules` and `VisualSlotControls`. Verify the exact capability set in that entry point.
- [ ] 7.5 Make the GUI attach caller declare `ShownQueueEntries` and `TransportChipKeys` and call `pin::claim_pin_swap_participant(false)` before attach. Verify ordering in the GUI entry point.
- [ ] 7.6 Make reconnect Hello claims use the stored capabilities. Verify `named-queues` follows `ShownQueueEntries` under the existing local-link rule.
- [ ] 7.7 Replace panel-arrangement Client-kind reads with `TerminalPanelRules` in the panel-state module. Verify Mini and empty-queue rules retain their conditions; the single-panel input is implemented in group 8.
- [ ] 7.8 Replace visualizer and visual-slot Client-kind reads with `VisualSlotControls`. Verify the existing setters preserve their enabled/disabled behaviour; split by source module before dispatch if needed.
- [ ] 7.9 Rename `gui_shown_entry` to `shown_entry` in its declaration and initialization only. Verify the shared field keeps its type and default; callers are separate rows.
- [ ] 7.10 Migrate shown-entry reads and capability gates in `state/named_queues.rs`. Verify the closed-queue fallback in `adopt_named_queue_list` stays in core.
- [ ] 7.11 Migrate shown-entry reads and capability gates in `state/queue_link.rs`. Verify link resolution retains its existing behaviour.
- [ ] 7.12 Migrate shown-entry reads and capability gates in `state/playback.rs`. Verify playback still follows the shown entry where it did before.
- [ ] 7.13 Remove obsolete Client-kind fields, methods, and enum definitions after their consumer rows: `gui_client`, `client_arranges_panels`, `visual_slot_controls()`, `client_kind`, `set_client_kind`, and `ClientKind`. First inventory their definition files and split this cleanup into one row per module if needed. Verify definition removal only; do not repair residual callers here.
- [ ] 7.14 Add `skip_launch_restore` and replace the GUI launch call to `enable_gui_client`. Verify the launch-restore transition is unchanged and occurs before the first step.
- [ ] 7.15 Change the router snapshot from `gui_client` to `ClientCapabilities`. Verify snapshot construction carries the declared capabilities; consumers may remain broken.
- [ ] 7.16 Rename the router gates to `TransportChipKeys` and `TransportChipQueueTarget` and change their predicates. Verify precedence and gate conditions remain unchanged.
- [ ] 7.17 Inventory remaining shown-entry, Client-kind, and old-gate consumers and tests; add one migration row per module. Verify coverage of the inventory, including `gui_client_state`; do not combine this work with 7.13 or require both to compile as one unit.

## 8. Shell state and pushed size facts (old 2.7)

- [ ] 8.1 Add the neutral single-panel field and `App::set_single_panel(bool)`. Verify its default is false and `is_mini_view` reads it with `TerminalPanelRules`.
- [ ] 8.2 Push the single-panel fact from the TUI resize handler using its width and `MINI_VIEW_THRESHOLD`. Verify the push occurs wherever that handler records terminal size today.
- [ ] 8.3 Push the single-panel fact during frame composition in `shell/draw.rs`. Verify the first frame receives its actual size before Mini-dependent reads.
- [ ] 8.4 Move `terminal_width` and `terminal_height` storage and initialization from `App` to `Model`. Verify the shell owns the dimensions; leave caller adaptation to 8.5.
- [ ] 8.5 Inventory dimension readers/writers and add one migration row per module. Verify no caller is hidden inside the storage move; record core reads that must become neutral inputs or shell rules.
- [ ] 8.6 Inventory other fields read or written only by the shell under D2. Add one storage-move row per related field family and one reader-migration row per module. Verify each proposed move records its consumers and prefs key, if any; do not move fields in this inventory task.
- [ ] 8.7 Add `save_client_pref` using the existing read/change/write format. Verify unrelated JSON keys are retained and existing key names do not change.
- [ ] 8.8 Inventory prefs for fields moved by group 8 and add one ownership-migration row per related key family. Verify each row names its load and save sites; no bulk prefs rewrite is authorized.
- [ ] 8.9 Adapt the existing Mini-view test module to the new shell size owner. Verify its behavioural expectations remain unchanged; identify any additional test module as a separate row.
- [ ] 8.10 Adapt the existing prefs round-trip tests affected by shell field moves, one named test module per dispatch. Before dispatch, split this row if more than one module is involved. Verify assertions preserve the existing file format and key names.

## 9. Tab strip and remaining terminal geometry (old 2.8, 2.10)

- [ ] 9.1 Move `tab_scroll` storage and its initialization to `Model`. Verify only storage ownership changes in this row.
- [ ] 9.2 Move `ensure_tab_visible`, `visible_tab_range`, and the tab-strip width calculation from `input.rs` to the shell. Verify the live geometry rule is preserved rather than cached in core.
- [ ] 9.3 Add the core tab-reveal request and replace the direct call in `dispatch/library/cw_library_tab.rs`. Verify the core requests reveal without naming shell geometry.
- [ ] 9.4 Consume the tab-reveal request after shell update steps. Verify reveal precedes paint or pointer resolution for that path.
- [ ] 9.5 Consume the tab-reveal request after tick drains. Verify reveal precedes paint or pointer resolution for that path.
- [ ] 9.6 Inventory remaining tab-scroll consumers and separate test modules; add one migration row per module. Verify every old consumer has a row; do not bulk-migrate them here.
- [ ] 9.7 Add neutral `left_panel_area` storage and `App::set_left_panel_area`. Verify the default is zero and no ratatui type enters the field or setter.
- [ ] 9.8 Move `AppLayout` storage and initialization from `App` to `Model`. Verify only shell ownership changes; leave core consumers to 9.10 and 9.11.
- [ ] 9.9 Push `left_panel_area` from `compose_root_frame` with the value written to `layout.left_area`. Verify the under-four-row path pushes zero.
- [ ] 9.10 Change `lib_page_size` in `dispatch/actions.rs` to read neutral `left_panel_area`. Verify the zero-area default retains page size 1.
- [ ] 9.11 Change visualizer-menu bounds in `dispatch/context_menu/menu.rs` to read neutral `left_panel_area`. Verify default zero still means no menu bounds.
- [ ] 9.12 Inventory other D13 Rule B readers outside the shell, including `infra/render_cadence`, `state/panel_focus`, and projection helpers. Add one move/adaptation row per responsibility and module. Verify each row chooses a shell rule, a timely push, or a neutral core rule as D13 requires; do not repair the inventory here.

## 10. Terminal input edge and test support (old 2.9)

- [ ] 10.1 Move crossterm key translation from shared input to the shell. Verify the shared router input is neutral and translation semantics are preserved.
- [ ] 10.2 Move crossterm mouse translation from shared input to the shell. Verify pointer translation semantics are preserved.
- [ ] 10.3 Split terminal/toolkit seams from `app/test_seams.rs` into shell test support. Verify neutral seams remain in shared code.
- [ ] 10.4 Split terminal helpers from `app/tests.rs` into shell test support. Verify `TestBackend`, `Terminal`, and the terminal confirm helper move while neutral helpers stay.
- [ ] 10.5 Inventory terminal-input test consumers under `app/input/` and add one neutral-event adaptation row per test module. Verify every use of the moved seam has a named row; do not rewrite all tests in this inventory task.
- [ ] 10.6 Adapt `dispatch/action/tests.rs` to build neutral keys. Verify existing expectations are preserved.
- [ ] 10.7 Adapt `dispatch/actions/tests/replacement_gate.rs` to build neutral keys. Verify existing expectations are preserved.
- [ ] 10.8 Adapt `dispatch/actions/route_tests/playback.rs` to build neutral keys. Verify existing expectations are preserved.
- [ ] 10.9 Inventory other consumers of the split test helpers and add one adaptation row per module. Verify terminal-driving tests use shell support and neutral tests do not gain terminal dependencies.

## 11. Command and playback seam (old 3.1)

- [ ] 11.1 Split the GUI-only command arms in `dispatch/action.rs` into documented exhaustive core no-ops. Verify no wildcard hides a variant and shared command behaviour stays in core.
- [ ] 11.2 Add the GUI command half in `input/gui.rs` after core dispatch. Verify the GUI-only toggle commands call core first, then the GUI method.
- [ ] 11.3 Split GUI-only playback-request arms from shared `handle_playback_request` into documented exhaustive core no-ops. Verify shared playback behaviour is retained.
- [ ] 11.4 Add the GUI playback-request half at the GUI input entry. Verify core runs first and then the GUI menu, track, chooser, or collapse action.
- [ ] 11.5 Move `activate_queue_filter` to its core owner. Verify its behaviour is preserved without GUI state parameters.
- [ ] 11.6 Move `request_close_shown_queue` and `send_named_queue_close` to the core named-queue owner. Verify close semantics and opaque queue identities are unchanged.
- [ ] 11.7 Move and rename `request_new_gui_queue` to core `request_new_queue`. Verify the request and answer correlation are preserved.
- [ ] 11.8 Move `request_rename_shown_queue` to the core named-queue owner. Verify the rename request behaviour is preserved.
- [ ] 11.9 Move `select_chooser_target` to the core playback-target owner. Verify both Clients retain the same callable behaviour.
- [ ] 11.10 Move `report_named_queue_rejection` and `take_new_queue_answer` to core. Verify request rejection and new-queue answer handling are unchanged.
- [ ] 11.11 Inventory callers and separate tests affected by 11.5–11.10 and add one adaptation row per module. Verify all old method paths are scheduled; do not fix all callers in this row.

## 12. Library effects seam (old 3.2)

- [ ] 12.1 Define core `ClientLibraryStores` and `ClientLibraryEffect` as in D4a. Verify the trait names no GUI state and accepts `App` only through its declared parameters.
- [ ] 12.2 Thread the optional store through `handle_lib_event` and `drain_lib_event`. Verify this entry layer carries the store without naming GUI types; leave lower handler changes to their rows.
- [ ] 12.3 Thread the store through `handle_browse_event`. Verify the handler signature and direct forwarding sites use the neutral trait.
- [ ] 12.4 Thread the store through `handle_music_event`. Verify the handler signature and direct forwarding sites use the neutral trait.
- [ ] 12.5 Collect and apply `RowArtSourcesChanged` at the existing library-event sites. Verify effects are applied after that event and before the next event; split by handler module before dispatch if needed.
- [ ] 12.6 Change `feed_home_video_selection_wanted` to use the optional store query. Verify the selection check remains synchronous.
- [ ] 12.7 Split `handle_feed_home_video_selection_loaded` claim retirement into `FeedTargetLanded`. Verify shared cache/fetch handling stays in core and the effect keeps its event ordering.
- [ ] 12.8 Split `handle_feed_home_video_selection_failed` claim insertion into `FeedTargetFailed`. Verify fetch-mark retirement and logging stay in core.
- [ ] 12.9 Add the transitional unit implementor in GUI code, reading the existing fields through `app`. Verify it implements only the neutral query/effects and preserves their old behaviour.
- [ ] 12.10 Wire the GUI drain to pass the transitional implementor. Verify it uses the same per-event sequence; leave TUI wiring to 12.11.
- [ ] 12.11 Make the TUI drain pass `None`. Verify no GUI effect executes from this entry.
- [ ] 12.12 Move the shared artist-name lookup state and App-side methods to core as `ArtistNameLookup`. Verify answer handling retains its client-presence guard; leave external callers to 12.13.
- [ ] 12.13 Inventory callers and separate tests affected by the library seam and artist lookup; add one adaptation row per module. Verify no caller migration is hidden in the inventory task.

## 13. GuiClientState declaration and field ownership (old 3.3)

These tasks move declarations and initializers only. References remain temporarily broken until the per-module migrations in group 14. Do not duplicate authoritative state to keep a build green.

- [ ] 13.1 Define the transitional `GuiClientState` container in GUI code and add `App::gui`. Verify it is a concrete owned struct, not type erasure or a GUI parameter threaded through core.
- [ ] 13.2 Move `gui_overlay`, `panel_collapsed`, and `queue_folded` declarations and initialization into the container. Verify defaults are preserved.
- [ ] 13.3 Move `gui_settings`, `gui_playlists_open`, and `gui_ui_reset_epoch` declarations and initialization into the container. Verify defaults are preserved.
- [ ] 13.4 Move `gui_player_auto_hide` declaration and initialization into the container, leaving prefs ownership transfer to group 16. Verify its loaded value is preserved.
- [ ] 13.5 Move Music collection fields `gui_library_groups`, `gui_music_albums`, `gui_library_picks`, and `gui_music_union_picks` into the container. Verify types and defaults are preserved.
- [ ] 13.6 Move `gui_library_row_menu_target`, `gui_rows_in_view`, and `gui_home_selection` into the container. Verify types and defaults are preserved.
- [ ] 13.7 Move `gui_shows_flat_selection`, `gui_shows_views`, `gui_shows_new`, and `gui_tv_shows` into the container. Verify types and defaults are preserved.
- [ ] 13.8 Move `gui_emby_libs`, `gui_emby_feed_sets`, and `gui_emby_roots` into the container. Verify types and defaults are preserved.
- [ ] 13.9 Move `gui_abs_libs`, `gui_podcast_libs`, and `gui_feeds_selection` into the container. Verify types and defaults are preserved.
- [ ] 13.10 Move `gui_list_sorts` and `gui_search` into the container. Verify types and defaults are preserved.
- [ ] 13.11 Move overlay accessors onto `GuiClientState`. Verify the accessors use its state directly; external callers remain separate work.

## 14. GuiClientState reference migrations (old 3.3 continued)

Each row changes field paths in ONE named source module only, from `self/app.gui_<name>` or the collapse/fold fields to the transitional `self/app.gui.<name>`. Do not convert `impl App` to `impl GuiCore` yet. Verify each row by inspecting that module's field paths; no build or test pass is required. Inline tests in the same file may change paths without changing expectations. Separate child modules and test files are not included implicitly.

- [ ] 14.1 Migrate `src/app/gui_client_state.rs`; verify its field paths use the container.
- [ ] 14.2 Migrate `gui_client_state/overlay.rs`; verify its field paths use the container.
- [ ] 14.3 Migrate `gui_client_state/popover.rs`; verify its field paths use the container.
- [ ] 14.4 Migrate `gui_client_state/player_menu.rs`; verify its field paths use the container.
- [ ] 14.5 Migrate `gui_client_state/queue_chooser.rs`; verify its field paths use the container.
- [ ] 14.6 Migrate `gui_client_state/target.rs`; verify its field paths use the container.
- [ ] 14.7 Migrate `gui_client_state/row_menu.rs`; verify its field paths use the container.
- [ ] 14.8 Migrate `gui_client_state/confirm.rs`; verify its field paths use the container.
- [ ] 14.9 Migrate `gui_client_state/daemon_lost.rs`; verify its field paths use the container.
- [ ] 14.10 Migrate `gui_client_state/queue_controls.rs`; verify its field paths use the container.
- [ ] 14.11 Migrate `gui_client_state/projection.rs`; verify its field paths use the container.
- [ ] 14.12 Migrate `src/app/gui_settings.rs`; verify its field paths use the container.
- [ ] 14.13 Migrate `gui_settings/command.rs`; verify its field paths use the container.
- [ ] 14.14 Migrate `gui_settings/feeds.rs`; verify its field paths use the container.
- [ ] 14.15 Migrate `gui_settings/keys.rs`; verify its field paths use the container.
- [ ] 14.16 Migrate `gui_settings/projection.rs`; verify its field paths use the container.
- [ ] 14.17 Migrate `gui_settings/reset.rs`; verify its field paths use the container.
- [ ] 14.18 Migrate `gui_settings/sections.rs`; verify its field paths use the container.
- [ ] 14.19 Migrate `gui_settings/services.rs`; verify its field paths use the container.
- [ ] 14.20 Migrate `gui_settings/sessions.rs`; verify its field paths use the container.
- [ ] 14.21 Migrate `src/app/gui_playlists.rs`; verify its field paths use the container.
- [ ] 14.22 Migrate `gui_playlists/commands.rs`; verify its field paths use the container.
- [ ] 14.23 Migrate `src/app/gui_library.rs`; verify its field paths use the container.
- [ ] 14.24 Migrate `gui_library/launch.rs`; verify its field paths use the container.
- [ ] 14.25 Migrate `gui_library/ensure.rs`; verify its field paths use the container.
- [ ] 14.26 Migrate `gui_library/shows_projection.rs`; verify its field paths use the container.
- [ ] 14.27 Migrate `gui_library/shows_projection/hero.rs`; verify its field paths use the container.
- [ ] 14.28 Migrate `gui_library/audiobooks_projection.rs`; verify its field paths use the container.
- [ ] 14.29 Migrate `gui_library/emby_projection.rs`; verify its field paths use the container.
- [ ] 14.30 Migrate `gui_library/item_menu.rs`; verify its field paths use the container.
- [ ] 14.31 Migrate `gui_library/rows_in_view.rs`; verify its field paths use the container.
- [ ] 14.32 Migrate `gui_library/marks.rs`; verify its field paths use the container.
- [ ] 14.33 Migrate `gui_library/podcasts_projection.rs`; verify its field paths use the container.
- [ ] 14.34 Migrate `gui_library/podcasts.rs`; verify its field paths use the container.
- [ ] 14.35 Migrate `gui_library/item_actions.rs`; verify its field paths use the container.
- [ ] 14.36 Migrate `gui_library/feeds_projection.rs`; verify its field paths use the container.
- [ ] 14.37 Migrate `gui_library/feeds.rs`; verify its field paths use the container.
- [ ] 14.38 Migrate `gui_library/entry_list.rs`; verify its field paths use the container.
- [ ] 14.39 Migrate `gui_library/audiobooks.rs`; verify its field paths use the container.
- [ ] 14.40 Migrate `gui_library/shows.rs`; verify its field paths use the container.
- [ ] 14.41 Migrate `gui_library/tv.rs`; verify its field paths use the container.
- [ ] 14.42 Migrate `gui_library/emby.rs`; verify its field paths use the container.
- [ ] 14.43 Migrate `gui_library/search.rs`; verify its field paths use the container.
- [ ] 14.44 Migrate `gui_library/projection.rs`; verify its field paths use the container.
- [ ] 14.45 Migrate `gui_library/store.rs`; verify its field paths use the container.
- [ ] 14.46 Migrate `gui_library/list_sort.rs`; verify its field paths use the container.
- [ ] 14.47 Migrate `gui_library/home.rs`; verify its field paths use the container.
- [ ] 14.48 Migrate `gui_library/artwork.rs`; verify its field paths use the container.
- [ ] 14.49 Migrate `gui_library/artwork/rows.rs`; verify its field paths use the container.
- [ ] 14.50 Migrate `gui_library/collection.rs`; verify its field paths use the container.
- [ ] 14.51 Migrate `gui_library/collection_command.rs`; verify its field paths use the container.
- [ ] 14.52 Migrate `gui_library/tabs.rs`; verify its field paths use the container.
- [ ] 14.53 Migrate `gui_library/request.rs`; verify its field paths use the container.
- [ ] 14.54 Migrate `gui_library/shows_view.rs`; verify its field paths use the container.
- [ ] 14.55 Migrate `src/app/input/gui.rs`; verify its field paths use the container.
- [ ] 14.56 Inventory remaining old field references in root test seams, separate test files, `mbv-gui`, and any other modules. Add one reference-migration row per file before dispatch. Verify inventory coverage; do not convert all remaining sites in this audit.

## 15. GuiCore wrapper and module conversions (old 3.4)

Create the wrapper first, then convert callers before callees as below. No row must build before another starts. Each conversion row changes only the named module's `impl App` blocks to `impl GuiCore`, qualifies shared reads through `self.app`, and uses transitional `self.app.gui` for GUI state. Free data types stay free data types. Files stay in place. Direct imports/exports are permitted, but widening `App` visibility, converting child modules, moving separate tests, and repairing all callers are not. Verify each row by inspecting the changed receiver and state ownership in that module. If it has no applicable block after earlier edits, record that observed result rather than inventing work.

- [ ] 15.1 Define `GuiCore { app: App }` in root `src/gui.rs` and register the module. Verify the wrapper is outside `crate::app` and has a construction path; do not move GUI state out of `App` yet.
- [ ] 15.2 Change `mbv-gui`'s `CoreThread` storage to `GuiCore`. Verify the field and construction use the wrapper; leave method-call adaptations to separately inventoried rows.
- [ ] 15.3 Convert `src/app/input/gui.rs`; verify its GUI entry methods use `GuiCore`.
- [ ] 15.4 Convert `src/app/gui_client_state.rs`; verify its applicable methods use `GuiCore`.
- [ ] 15.5 Convert `gui_client_state/overlay.rs`; verify receiver and state paths.
- [ ] 15.6 Convert `gui_client_state/popover.rs`; verify receiver and state paths.
- [ ] 15.7 Convert `gui_client_state/player_menu.rs`; verify receiver and state paths.
- [ ] 15.8 Convert `gui_client_state/queue_chooser.rs`; verify receiver and state paths.
- [ ] 15.9 Convert `gui_client_state/target.rs`; verify receiver and state paths.
- [ ] 15.10 Convert `gui_client_state/row_menu.rs`; verify receiver and state paths.
- [ ] 15.11 Convert `gui_client_state/confirm.rs`; verify receiver and state paths.
- [ ] 15.12 Convert `gui_client_state/daemon_lost.rs`; verify receiver and state paths.
- [ ] 15.13 Convert `gui_client_state/queue_controls.rs`; verify receiver and state paths.
- [ ] 15.14 Convert `gui_client_state/projection.rs`; verify receiver and state paths.
- [ ] 15.15 Convert `src/app/gui_settings.rs`; verify receiver and state paths.
- [ ] 15.16 Convert `gui_settings/command.rs`; verify receiver and state paths.
- [ ] 15.17 Convert `gui_settings/feeds.rs`; verify receiver and state paths.
- [ ] 15.18 Convert `gui_settings/keys.rs`; verify receiver and state paths.
- [ ] 15.19 Convert `gui_settings/projection.rs`; verify receiver and state paths.
- [ ] 15.20 Convert `gui_settings/reset.rs`; verify receiver and state paths.
- [ ] 15.21 Convert `gui_settings/sections.rs`; verify receiver and state paths.
- [ ] 15.22 Convert `gui_settings/services.rs`; verify receiver and state paths.
- [ ] 15.23 Convert `gui_settings/sessions.rs`; verify receiver and state paths.
- [ ] 15.24 Convert `src/app/gui_playlists.rs`; verify receiver and state paths.
- [ ] 15.25 Convert `gui_playlists/commands.rs`; verify receiver and state paths.
- [ ] 15.26 Convert `src/app/gui_library.rs`; verify receiver and state paths.
- [ ] 15.27 Convert `gui_library/launch.rs`; verify receiver and state paths.
- [ ] 15.28 Convert `gui_library/ensure.rs`; verify receiver and state paths.
- [ ] 15.29 Convert `gui_library/shows_projection.rs`; verify receiver and state paths.
- [ ] 15.30 Convert `gui_library/shows_projection/hero.rs`; verify receiver and state paths.
- [ ] 15.31 Convert `gui_library/audiobooks_projection.rs`; verify receiver and state paths.
- [ ] 15.32 Convert `gui_library/emby_projection.rs`; verify receiver and state paths.
- [ ] 15.33 Convert `gui_library/item_menu.rs`; verify receiver and state paths.
- [ ] 15.34 Convert `gui_library/rows_in_view.rs`; verify receiver and state paths.
- [ ] 15.35 Convert `gui_library/marks.rs`; verify receiver and state paths.
- [ ] 15.36 Convert `gui_library/podcasts_projection.rs`; verify receiver and state paths.
- [ ] 15.37 Convert `gui_library/podcasts.rs`; verify receiver and state paths.
- [ ] 15.38 Convert `gui_library/item_actions.rs`; verify receiver and state paths.
- [ ] 15.39 Convert `gui_library/feeds_projection.rs`; verify receiver and state paths.
- [ ] 15.40 Convert `gui_library/feeds.rs`; verify receiver and state paths.
- [ ] 15.41 Convert `gui_library/entry_list.rs`; verify receiver and state paths.
- [ ] 15.42 Convert `gui_library/audiobooks.rs`; verify receiver and state paths.
- [ ] 15.43 Convert `gui_library/shows.rs`; verify receiver and state paths.
- [ ] 15.44 Convert `gui_library/tv.rs`; verify receiver and state paths.
- [ ] 15.45 Convert `gui_library/emby.rs`; verify receiver and state paths.
- [ ] 15.46 Convert `gui_library/search.rs`; verify receiver and state paths.
- [ ] 15.47 Convert `gui_library/projection.rs`; verify receiver and state paths.
- [ ] 15.48 Convert `gui_library/store.rs`; verify receiver and state paths.
- [ ] 15.49 Convert `gui_library/list_sort.rs`; verify receiver and state paths.
- [ ] 15.50 Convert `gui_library/home.rs`; verify receiver and state paths.
- [ ] 15.51 Convert `gui_library/artwork.rs`; verify receiver and state paths.
- [ ] 15.52 Convert `gui_library/artwork/rows.rs`; verify receiver and state paths.
- [ ] 15.53 Convert `gui_library/collection.rs`; verify receiver and state paths.
- [ ] 15.54 Convert `gui_library/collection_command.rs`; verify receiver and state paths.
- [ ] 15.55 Convert `gui_library/tabs.rs`; verify receiver and state paths.
- [ ] 15.56 Convert `gui_library/request.rs`; verify receiver and state paths.
- [ ] 15.57 Convert `gui_library/shows_view.rs`; verify receiver and state paths.
- [ ] 15.58 Inventory `app_stub.rs` and `app_stub/test_reads*` seams that call converted GUI methods or read GUI state; add one seam-move row per source module beside `GuiCore`. Verify shared App-only seams remain in place; do not move all seams here.
- [ ] 15.59 Inventory separate GUI test modules and `mbv-gui` production consumers affected by the wrapper; add one adaptation row per module. Verify each GUI test setup wraps its App stub in `GuiCore` where needed and no expectation changes are planned; do not convert all consumers here.

## 16. GUI state leaves App (old 3.5)

- [ ] 16.1 Move the transitional `gui` field and its construction from `App` to `GuiCore { app, gui }`. Verify there is one GUI state owner, not a mirrored copy; leave module field-path migrations to 16.2.
- [ ] 16.2 Inventory `self.app.gui` and `app.gui` consumers after 16.1. Add one path-migration row per module to use the new GUI owner. Verify every old owner path is scheduled; do not change all modules in this inventory task.
- [ ] 16.3 Implement `ClientLibraryStores` on `GuiClientState` in place of the transitional implementor. Verify the query/effect behaviour is unchanged; leave drain wiring to 16.4.
- [ ] 16.4 Make the GUI drain pass `Some(&mut self.gui)`. Verify event effects retain the per-event ordering of D4a.
- [ ] 16.5 Load `gui_player_auto_hide` in GUI startup and save it through `save_client_pref`. Verify the persisted key and default are unchanged; leave core key removal to 16.6.
- [ ] 16.6 Remove GUI auto-hide key handling from core prefs load/save. Verify core writes retain the existing GUI key rather than deleting or replacing it.
- [ ] 16.7 Move the existing auto-hide prefs round-trip test to the GUI owner. Verify its file-format and round-trip assertions are unchanged.

## 17. Client-only event channel (old 3.6)

- [ ] 17.1 Add `drain_client_events(&mut self) -> bool` to `ClientTickHost` and call it after `drain_lib_events` in the shared tick. Verify the hook order by source inspection.
- [ ] 17.2 Implement the TUI hook as a documented no-op returning false. Verify it owns no GUI channel.
- [ ] 17.3 Add the GUI-owned event enum and channel to `GuiCore`. Verify its wake path uses the existing `ClientWaker`; leave individual event moves to 17.5–17.10.
- [ ] 17.4 Implement the GUI hook and CoreThread forwarding to drain the client channel. Verify results enter the GUI owner and preserve the existing tick wake mechanism.
- [ ] 17.5 Move the `GuiGroupAlbums` consumer from shared library dispatch to the GUI channel consumer. Verify payload handling is preserved.
- [ ] 17.6 Route the `GuiGroupAlbums` producer to the GUI channel and remove its shared enum variant. Verify the producer sends the same payload with the same waker.
- [ ] 17.7 Move the `GuiEmbyRootItems` consumer to the GUI channel consumer. Verify payload handling is preserved.
- [ ] 17.8 Route the `GuiEmbyRootItems` producer to the GUI channel and remove its shared enum variant. Verify payload and wake behaviour are preserved.
- [ ] 17.9 Move the `GuiShowsNew` consumer to the GUI channel consumer. Verify payload handling is preserved.
- [ ] 17.10 Route the `GuiShowsNew` producer to the GUI channel and remove its shared enum variant. Verify payload and wake behaviour are preserved.
- [ ] 17.11 Inventory separate tests of the three moved events and add one ownership/adaptation row per test module. Verify existing contracts and assertions are retained; do not bulk-rewrite tests here.

## 18. Neutral router entry (old 3.7)

- [ ] 18.1 Expose the router snapshot entry and `RouterOutcome` without exposing mutable router fields. Verify the public entry takes facts and returns an outcome; precedence remains in the existing router.
- [ ] 18.2 Add public prefix-state read methods. Verify callers can read the required facts without direct field access.
- [ ] 18.3 Adapt `input/gui.rs` to use the neutral router entry and prefix reads. Verify it no longer reads `self.app.router`, `key_policy`, or `prefix` directly; no new routing site is added.
- [ ] 18.4 Inventory separate router-entry test consumers and add one adaptation row per module. Verify the tests retain the existing precedence, order, and chord contracts.

## 19. Documentation (old 4.1–4.2)

Each row owns one document. Source groups must record their moved symbols and changed ownership in their handoffs so these rows do not rediscover the refactor.

- [ ] 19.1 Write ADR 0042, following `docs/adr/README.md`, for D4, D4a, D5, D6, and D6a. Verify it records composition, the no-GUI-call core boundary, client events, neutral signatures, and capabilities.
- [ ] 19.2 Add Client capability to `CONTEXT.md` with the Avoid terms `client kind` and `gui_client`. Verify the term agrees with D6a and does not rename another domain concept.
- [ ] 19.3 Update `docs/architecture/gui-client.md` for `GuiCore`, `GuiClientState`, `ClientLibraryStores`, and the client channel. Verify documented ownership matches the planned result.
- [ ] 19.4 Update `docs/architecture/tui-frontend.md` for `menu_pointer`, the single-panel push, `left_panel_area`, and tab reveal. Verify the timing rules agree with D6a, D6b, and D13.
- [ ] 19.5 Update moved-symbol site lists in `docs/recipes.md`. Verify each affected recipe names its new owner; do not rewrite unrelated recipes.

## 20. Boundary audits and integration checks (old 5.1–5.3)

These tasks assess the assembled change. Their results do not reopen completed edit rows or retroactively impose green gates on them. A failed integration check records a failure and creates bounded fix rows; it is not complete as a successful check until its stated contract passes. Never assign all failures to one repair worker.

- [ ] 20.1 Audit shared production source outside shell and GUI modules for terminal/toolkit dependencies and `shell::` calls. Inspect matches rather than accepting an empty or wrongly scoped search. Verify no forbidden production dependency remains; create separate bounded fix rows for findings.
- [ ] 20.2 Audit GUI source imports for `mbv_components` and `mbv_render`, and shared `App` for GUI state or GUI method calls. Verify the planned ownership boundary is met; create separate bounded fix rows for findings.
- [ ] 20.3 Audit shared menu signatures and state for terminal-cell anchors. Verify pointer placement is Client-owned; create separate bounded fix rows for findings.
- [ ] 20.4 Audit obsolete Client-kind names, shown-entry names, GUI `impl App` blocks, and the three GUI variants in shared events/dispatch. Verify each remaining match is either an intentional historical reference or scheduled separately for correction.
- [ ] 20.5 Run `cargo tree -p mbv-theme -e normal --prefix none`. Verify no `ratatui`, `ratatui-image`, `crossterm`, or `tuirealm` appears in its normal dependency graph.
- [ ] 20.6 Run `cargo tree -p mbv-images -e normal --prefix none`. Verify no `ratatui`, `ratatui-image`, `crossterm`, or `tuirealm` appears in its normal dependency graph.
- [ ] 20.7 Run `cargo tree -p mbv-ui-model -e normal --prefix none`. Verify no `ratatui`, `ratatui-image`, `crossterm`, or `tuirealm` appears in its normal dependency graph.
- [ ] 20.8 Run `cargo check --workspace --all-targets`. Record the result and schedule failures as bounded fix rows; this integration check succeeds when the command passes.
- [ ] 20.9 Run `cargo fmt --all -- --check`. Record the result; this integration check succeeds when the command passes.
- [ ] 20.10 Run `cargo clippy --workspace --all-targets -- -D warnings`. Record the result and schedule bounded fixes without suppressions; this integration check succeeds when the command passes.
- [ ] 20.11 Run `cargo nextest run --release --test-threads=4`. Record the result and schedule failures by owning contract; this integration check succeeds when the command passes.
- [ ] 20.12 Run `cargo nextest run --release --test-threads=4 -p mbv-gui`. Record the result and schedule failures by owning contract; this integration check succeeds when the command passes.
- [ ] 20.13 Run `openspec validate 1-decouple-client-core --strict`. Verify the change is valid; source/spec agreement is assessed separately in 20.14.
- [ ] 20.14 Compare the resulting ownership and behaviour to the delta spec. Verify neutral shared values, capability-based core rules, and Client-owned local state; record any mismatch as a bounded follow-up, not an unbounded repair assignment.

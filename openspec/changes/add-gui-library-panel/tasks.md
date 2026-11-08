# Tasks: add-gui-library-panel

Phase 1 (groups 1–5) delivers Music from end to end. Phase 2 (groups 6–12) starts only after the Phase 1 gate in `design.md` (Migration Plan). The canvas (`canvas/Library.dc.html`) is the reference for every painted value. No task changes the TUI's paint, keys, or behaviour; a task that touches TUI code proves it with the existing TUI tests, unedited.

## Phase 1 — Music

## 1. Shared Service startup

- [x] 1.1 Add `App::start_client_services` (design D11): the Emby startup worker, the Audiobookshelf startup worker, `start_feed_fetch`, and capability registration, moved from `Model::spawn_startup_services`, which calls it at the same point. Move the `App` half of `Model::fetch_home_at_startup` into `App::load_home_at_startup`; both Clients call it right after `start_client_services`, and the `Model` keeps the Home half. The GUI core thread calls both in place of `start_emby_client_fill`; delete `start_emby_client_fill`, its worker, and its tests. Add `StartupWorkerDrain::emby_started`, and start `spawn_sessions_load` and `spawn_cast_discovery` in the GUI after `start_client_services` when the attach brought a cached Emby client, and in its `drain_startup_workers` host when `emby_started` is set. Verify: the TUI startup and lifecycle tests pass unedited; a `crates/mbv-gui/tests/` nextest with a mocked Emby startup completion finds the catalog in `App::libs`; a nextest that attaches with a cached Emby client and a mocked views response finds the Music tab in `App::libs`; a regression test (guards the fill's discovery that this task deletes) asserts that the sessions load and the cast discovery start in both cases.

## 2. The GUI's App settings

- [x] 2.1 Add the GUI setting on the `App` (design D4): with it, `is_mini_view` and `queue_column_hidden_empty` return false, and the `App` starts with `LaunchRestore::Done` (design D9). `CoreThread::new` sets it before its first step and mounts `ComponentId::Library`. A public GUI entry sets Panel focus on the `App` and the core thread sets its `FocusStack` with it. Verify: an `App` test with the GUI setting and an empty queue keeps the stored Queue focus and the `Both` mode; a core-thread test sends the take-focus command and finds Library focus in the next frame; the TUI panel-focus and launch-restore tests pass unedited.

## 3. The Library core

- [x] 3.1 Add `GuiLibraryCommand`, the `library` Tauri command, `CoreInput::Library`, and the public entry in `src/app/gui_library.rs` that applies each variant (design D2): open a destination (`App::set_library_tab`), set a destination's collections, show an item's detail, open an album node, take Panel focus, play, play next, add to queue, shuffle an artist, and run an item-menu entry. Verify: one core-thread test per command family finds the `App` effect (the destination, the stored set, the mocked Owner request).
- [x] 3.2 Add the Music collection store (design D5): one album list per selected group, fetched with `get_items_sorted` and paged to completion, landing outside `nav_stack`. Verify: a hermetic `App` test selects a group and finds its albums in the store with `nav_stack` unchanged; a test re-selects a loaded group and finds no fetch; a test with one failing group keeps the other group's albums.
- [x] 3.3 Add the Music projection in `src/app/gui_library/` (design D5): the group pills, the union of artists by `ArtistKey` in sort-name order with each row's sort letter, the albums and their cached tracks, and the Hero's facts for the picked album. Verify: a projection test with two groups that share one artist yields that artist once, in order, with its letter; a test for a picked album with cached tracks yields the Hero facts and the tracklist.
- [x] 3.4 Add `GuiLibraryFrame` and the `library` field on `GuiFrame` (design D2), with the content revision and the art as `data:` URLs from the image cache (`App::fetch_card_image`, encoded as `src/artwork.rs` does). Verify: a nextest finds the content in a frame only when its revision changes and again after `report_layout`; a nextest finds the placeholder, then the art after a mocked image completion.
- [x] 3.5 Add `App::play_lib_item_next` (design D8). Verify: an `App` test with a mocked Owner admits a Library item with `before` set to the slot after the active slot, and with `before: None` when the active slot is the last one and when there is no active slot.
- [x] 3.6 Add the item menu (design D8): widen `App::build_context_menu_for`, carry its entries in the frame, and run a chosen entry through `App::execute_context_action`. Verify: a nextest opens the menu for an album, runs one entry, finds its `App` effect, and finds no `pending_overlay`.
- [x] 3.7 Add the GUI launch location (design D9): `mbv_config::load_gui_launch_state` and `save_gui_launch_state`, the save at orderly exit before `client_exit_teardown`, and the restore after the catalog lands. Verify: `crates/mbv-config` tests round-trip a location with two groups; a `crates/mbv-gui/tests/` nextest saves Music, two groups, and Library focus at exit into a temporary state path and writes nothing before exit; a nextest restores that location and drops a saved group that the library no longer has; a nextest restores a location saved on Shows and finds Music active; the TUI launch-state tests pass unedited.

## 4. The front end

- [x] 4.1 Route keys in the window content (design D3): the Library handler runs before `input.ts`'s forwarder, takes the window keys (Tab panes, digits) and the Library keys, passes every key to the core while a core overlay is open or prefix mode is armed, and lets the filter field take typed text. Update `input.ts`'s header comment. Verify: vitest that with Library focus `/` opens the Library filter and sends no `key`; that with Queue focus `/` is sent; that with prefix armed a Library key is sent; that Space on a focused pill toggles it and sends no `key`, and Space with no focused pill is sent.
- [x] 4.2 Add `LibraryPanel.svelte` and the arrangement in `App.svelte` (design D4): the Library column measurement, the 480 px and 880 px thresholds with the 600 px height guard, Mini showing the panel with Panel focus, Narrow with the Hero overlay, and Wide with the Hero pane. Use GUI-local `--canvas-*` tokens in `app.css` for the Library values; no `mbv-theme` change. Verify: vitest for the panels mounted in each arrangement.
- [x] 4.3 Paint the top bar: one tab per presented destination, the current tab marked `aria-current`, and a press sending the open command. The Home icon joins with Home (group 6), and the Search field with Search (group 12). Verify: vitest for the tabs and the press.
- [x] 4.4 Paint the pill row and its rules (design D6): `aria-pressed` and the check on selected pills, toggle, Alt-press solo, the last pill stays, *N selected · clear* keeping the first selected pill in pill order, a single pill clearing the set, Left and Right moving the focused pill, and Space and Alt+Space on it. Verify: one vitest `it.each` table of gestures and the set each one sends.
- [x] 4.5 Paint the list (design D7): the artist, album, and track rows at their canvas heights, the letter headings, the cursor, open and close, pick, the `/` filter, and only the rows in view. Verify: vitest that opening an unloaded album sends the open-album command, that a cursor move past the last visible row scrolls the row into view, and that the filter narrows the rows.
- [x] 4.6 Paint the letter rail (design D7). Verify: vitest that a letter with no rows is disabled and that a press moves the cursor to the first row of its letter.
- [x] 4.7 Paint the Hero (design D5, D8): the artist header with Shuffle artist, the album block with the square art and Play, Play next (`n`), Add to queue (`a`), and More actions, the tracklist with its hover Play next and Add to queue controls, the item menu in the row-menu look, and the Narrow overlay with the same painters. Verify: vitest that each control sends its command with the item's id, that the menu paints the frame's entries, and that the overlay mounts in Narrow.
- [ ] 4.8 Paint the Library Status bar (design D10). Verify: vitest for the hints and the location, for a toast and the armed prefix replacing the hints, and for the Queue footer keeping them while the Library panel does not show.

## 5. Phase 1 docs and checks

- [ ] 5.1 Update `docs/architecture/gui-client.md`: the GUI Library panel section (the ownership split, the frame section and its revision, the `library` command, the key routing order, the arrangement, the launch location), the input forwarding that now runs after the Library handler, and the shared Service startup in *The attach states*. Record the GUI Library panel in `docs/architecture/interactive-surface-ledger.md` as GUI-owned, with no Surface state. Verify: the guide names each new frame field and command variant.
- [ ] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `cargo fmt --all -- --check`, `make ui-check ui-lint ui-test`, and `make check-code-file-lines` before the Phase 1 push. Verify: all green.

## Phase 2 — the other destinations and Search

Each unit starts by reading its destination's data path in the `App` and recording it in design D12, then adds its projection in `src/app/gui_library/`, its commands, and its painters, and adds its tab to the top bar.

## 6. Home

- [ ] 6.1 Move Home's content from `Model::home_content` into the `App` (the TUI's Home paint and tests do not change), store the content of `App::load_home_at_startup` and of the startup drain's `home` for the GUI, and move `init_idle_feed` into `App::start_client_services`. Verify: the TUI Home and startup tests pass unedited; a `crates/mbv-gui/tests/` nextest runs a mocked asynchronous Emby bootstrap with Continue Watching items and finds them in the next Home frame.
- [ ] 6.2 Project and paint Home's sections and Hero, and add the Home icon to the top bar. Verify: a projection test; vitest for the sections.

## 7. Shows

- [ ] 7.1 Project and paint Shows: the series list with the letter rail, landscape or portrait art, the season pills and episodes as the Hero's Workspace, and no Hero for Upcoming. Verify: projection tests for the series, flat Latest, and Upcoming modes; vitest for the Workspace and each art shape.

## 8. Movies and other Emby libraries

- [ ] 8.1 Project and paint an Emby library: the Hero without a Workspace, the art shape from the artwork policy, the letter rail while sorted by name, and, in feed-group mode, the feed-group pills as collection pills with each selected group's items loaded and their union in the list's sort. Verify: a projection test; a union test for two feed groups.

## 9. Audiobooks

- [ ] 9.1 Project and paint Audiobooks: the book list, the book Hero, and the chapters as the Hero's Workspace. Verify: a projection test; vitest for the chapter Workspace.

## 10. Podcasts

- [ ] 10.1 Project and paint Podcasts: the show pills as collection pills, the union of the selected shows' episodes in date order, the episode Hero, and no letter rail. Verify: a union test for two shows in date order.

## 11. Feeds

- [ ] 11.1 Project and paint Feeds: the subscription pills as collection pills, the union of the selected subscriptions' entries in date order, and no letter rail. Verify: a union test for two subscriptions.

## 12. Search

- [ ] 12.1 Add the Search field to the top bar with `Ctrl+K`, the search command, and its results (design D12): the core runs `App::spawn_search_sidebar_query`, the GUI tick host runs `drain_search_results` in place of its no-op, and a result activates through `App::activate_search_result`. Verify: a core-thread test sends a query with a mocked result and finds it in the next frame, then activates it and finds its destination active; vitest that `Ctrl+K` focuses the field and Esc closes the results.

## 13. Phase 2 docs and checks

- [ ] 13.1 Delete the saved-destination fallback and its test now that every destination is presented (design D9), and update the guide for the Phase 2 destinations and Search. Verify: the restore nextest for a location saved on Shows finds Shows active.
- [ ] 13.2 Run the gates from 5.2. Verify: all green.

# Tasks: add-gui-library-panel

Phase 1 (groups 1–7) delivers Music from end to end. Phase 2 (groups 8–14) starts only after the Phase 1 acceptance gate in `design.md` (Migration Plan). Every port task keeps the TUI's paint, keys, and behaviour unchanged; its proof is the existing TUI tests, green and unedited. Cite canvas element names from `canvas/Library.dc.html`.

## Phase 1 — Music

## 1. Library Surface state and the Music port

- [ ] 1.1 Add `mbv_surfaces::library` with `LibrarySurface` (Panel focus, active destination, per-destination `DestinationSurface`) and `SelectorSurface` (pills, pill kinds, selected set, focused pill), per design D1 and D5. Verify: `crates/mbv-surfaces/tests/library/` covers toggle, solo, last-pill-stays, clear-keeps-first-in-pill-order, and single-pill-clears-set; `cargo nextest run -p mbv-surfaces`.
- [ ] 1.2 Move Music's browser tree state (cursor, viewport, open nodes) and Workspace state (track list, Workspace focus, filter draft) from `music_content` into `DestinationSurface`; `MusicWorkspaceComponent` becomes an adapter. Verify: `cargo nextest run -p mbv-components` and the `tick_integration` Music tests pass unedited.
- [ ] 1.3 Move Library Panel focus and the tab selection from `LibraryPanel` and `tab_panel` into `LibrarySurface`; both components become adapters. Verify: the existing `library_panel` and `tab_panel` tests pass unedited; `docs/architecture/interactive-surface-ledger.md` names the new Surface state for the Library, Tab, and Music rows.

## 2. Multi-select collection pills and persistence (Music)

- [ ] 2.1 Load each selected Music collection with its existing per-collection fetch and derive the artist list as the union de-duplicated by item id, sorted by sort name (design D5). Verify: a hermetic owner test with two mocked collections that share one artist yields that artist once, in sort order.
- [ ] 2.2 Add `extra_selectors` to `TuiLaunchState` with `#[serde(default)]`; save and restore the selected set of the active destination (design D6). Verify: `crates/mbv-config` tests round-trip a two-pill state and load a file without the field; a TUI launch test restores only the first pill.

## 3. Frame, regions, and leaf

- [ ] 3.1 Add `GuiLibraryFrame` under `crates/mbv-gui/src/bridge/frame/library.rs`, built from `LibraryPanelContent` and the adopted Surface state (design D2), with artwork carried from the image cache, and the field on `GuiFrame`. Verify: a `crates/mbv-gui/tests/` test projects a Music fixture into tabs, pills with the selected set, viewport rows, Hero facts with square art, and the TRACKLIST Workspace.
- [ ] 3.2 Add `LibraryTargetId` and the `LibraryList` and `LibraryWorkspace` `ListId`s to `src/bridge/layout.rs`, mirroring the Library hit targets (design D3). Verify: a nextest maps each reported region to its target and applies whole-row capacity to the Music lists.
- [ ] 3.3 Add the Library `GuiSurfaceLeaf` in `src/leaves.rs`: pill press and Alt-press, clear, tab, row press, letter, Hero action, Workspace row and row actions, overlay close; unported targets are documented no-ops. Verify: a nextest per gesture family asserts the emitted `Msg`.

## 4. Core wiring and the window arrangement

- [ ] 4.1 Mount the Library in the core thread's `FocusStack`, hold `LibrarySurface`, adopt the Music projection once per emitted frame, and route keys and resolved gestures through the leaf; Panel focus moves by the TUI key and by a press. Verify: a core-thread test moves Panel focus by key and by a Library row press.
- [ ] 4.2 Carry the breakpoint class in `LayoutReport` and on the frame; the front end classifies the Library column width with `mini_below = 480`, `wide_from = 880`, and the 600 px height guard (design D4). Verify: vitest for the classifier at the three classes and with the `z` rail width; a nextest that a class change keeps cursor and pill set.
- [ ] 4.3 Open and close the Library Hero overlay in Narrow by the TUI's keys, with the state in the core. Verify: a core-thread test opens the overlay in Narrow and finds no overlay state in Wide.
- [ ] 4.4 Update `docs/architecture/gui-client.md`: the Library surface, the arrangement and thresholds, and the Status bar placement. Verify: the doc names each new frame field, target id, and `ListId`.

## 5. Front end shell

- [ ] 5.1 Add `LibraryPanel.svelte` and the window arrangement in `App.svelte`: Mini shows the focused panel, Narrow shows the browser pane plus the Queue panel, Wide adds the Hero pane; regions and capacities report through `reportLayout` and retract on unmount. Verify: vitest asserts the mounted panels per class and the reported contribution.
- [ ] 5.2 Add the top bar (Home icon and destination tabs) in the canvas look, with GUI-local `--canvas-*` tokens for Library-only values (design D10). Verify: vitest asserts one tab per frame tab, the current tab marked `aria-current`, and a tab press sending its region.
- [ ] 5.3 Move the Status bar under the Library column while the Library shows (key hints, breadcrumb, prefix, toasts); the Queue footer keeps them when only the Queue shows (design D9). Verify: vitest for both placements with the prefix armed.

## 6. Music slot painters

- [ ] 6.1 Paint the selector row: pills with `aria-pressed`, the check on selected pills, the options control, and the selected-count clear control. Verify: vitest for one and two selected pills.
- [ ] 6.2 Paint the browser list: the artist, album, and track tree rows at fixed heights, letter headings, and the cursor row; the list paints only the frame's viewport rows. Verify: vitest asserts row count equals the reported capacity and the cursor row is marked.
- [ ] 6.3 Paint the letter rail from the frame's present letters; a press sends the letter region (design D7). Verify: vitest asserts absent letters are disabled and a present letter press sends its region; a core test moves the cursor to the first matching sort name.
- [ ] 6.4 Paint the Hero: artist header, square artwork slot from the frame image, album facts, and the Play, Play next, Add to queue, and more-actions controls with their key hints. Verify: vitest asserts each action control sends its region and shows its hint.
- [ ] 6.5 Paint the Workspace tracklist: header, fixed-height rows, and the hover play-next and add controls. Verify: vitest asserts hover controls send their regions and the row count equals capacity.
- [ ] 6.6 Paint the Library Hero overlay for Narrow with the same Hero and Workspace painters. Verify: vitest mounts the overlay from a Narrow frame with the overlay open.

## 7. Phase 1 checks

- [ ] 7.1 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `cargo fmt --all -- --check`, and `make ui-check ui-lint ui-test`. Verify: all green.

## Phase 2 — other destinations and Search

## 8. Shows

- [ ] 8.1 Port `tv_content` state (series tree, flat Latest carrier, season cursor, episode Workspace, Workspace focus) into `DestinationSurface`; the component becomes an adapter. Verify: the TV tests pass unedited.
- [ ] 8.2 Project Shows into `GuiLibraryFrame`: landscape or portrait art, the season-pill Workspace selector, no Workspace in flat mode, and no Hero for Upcoming. Verify: nextest projections for the three modes.
- [ ] 8.3 Paint the Workspace selector and the landscape and portrait art arms; collection pills multi-select and the letter rail on series. Verify: vitest for the Workspace selector and each art arm; a union test for two Shows collections.

## 9. Movies and other Emby libraries

- [ ] 9.1 Port `emby_library_content` state into `DestinationSurface`; adapter. Verify: its tests pass unedited.
- [ ] 9.2 Project and paint: the Hero without a Workspace, the artwork arm from the artwork policy, letter and group pills, and the letter rail. Verify: nextest projection and vitest for a Hero without Workspace.

## 10. Home

- [ ] 10.1 Port `home_content` state into `DestinationSurface`; adapter. Verify: its tests pass unedited.
- [ ] 10.2 Project and paint the Home sections and Hero, with no letter rail. Verify: nextest projection; vitest asserts no rail.

## 11. Audiobooks

- [ ] 11.1 Port `book_content` state (carrier, chapter Workspace, chapter focus) into `DestinationSurface`; adapter. Verify: its tests pass unedited.
- [ ] 11.2 Project and paint the book Hero and the chapter Workspace without a header, and the letter rail on books. Verify: nextest projection; vitest for the headerless Workspace.

## 12. Podcasts

- [ ] 12.1 Port `podcast_content` state into `DestinationSurface`; adapter. Verify: its tests pass unedited.
- [ ] 12.2 Project and paint the episode list and Hero, show pills, and no letter rail. Verify: nextest projection.

## 13. Feeds

- [ ] 13.1 Port `feeds_content` state into `DestinationSurface`; adapter. Verify: its tests pass unedited.
- [ ] 13.2 Project and paint Feeds, Feed group pills as collection pills, and no letter rail. Verify: nextest projection; a union test for two Feed groups.

## 14. Global Search and Inline Search

- [ ] 14.1 Port `search_sidebar` and `inline_search` state into `SearchSurface` and the destination filter draft; both components become adapters. Verify: their tests pass unedited.
- [ ] 14.2 Bind `Ctrl+K` to `search_open` in the GUI-only key context (design D8). Verify: a keybind test resolves `Ctrl+K` in the GUI context and leaves the TUI defaults unchanged.
- [ ] 14.3 Add the `SearchResults` `ListId` and Search targets; project the Search section; paint the top-bar field and the results in the Music row look. Verify: nextest for activation `Msg`; vitest for the field and the results capacity.
- [ ] 14.4 Update `docs/architecture/gui-client.md` and the surface ledger for the Phase 2 owners and Search. Verify: each ported owner's ledger row names its Surface state.

## 15. Phase 2 checks

- [ ] 15.1 Run the gates from 7.1 and `make check-code-file-lines`. Verify: all green.

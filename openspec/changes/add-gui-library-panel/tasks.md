# Tasks: add-gui-library-panel

Phase 1 (groups 1–7) delivers Music from end to end. Phase 2 (groups 8–14) starts only after the Phase 1 acceptance gate in `design.md` (Migration Plan). Every port task keeps the TUI's paint, keys, and behaviour unchanged; its proof is the existing TUI tests, green and unedited. Cite canvas element names from `canvas/Library.dc.html`.

## Phase 1 — Music

## 1. Library Surface state and the Music port

- [ ] 1.1 Add `mbv_surfaces::library` with `LibrarySurface` (tab selection and per-destination `DestinationSurface`; no Panel focus — the core `FocusStack` keeps it, design D1) `SelectorSurface` (pills, pill kinds, selected set, focused pill, pill focus), and `LibraryHitTarget`, the Library hit-target enum that `LibraryTargetId` mirrors (design D3), per design D1 and D5. Verify: `crates/mbv-surfaces/tests/library/` covers toggle, solo, last-pill-stays, clear-keeps-first-in-pill-order, and single-pill-clears-set; `cargo nextest run -p mbv-surfaces`.
- [ ] 1.2 Move Music's browser tree state (cursor, viewport, open nodes) and Workspace state (track list, Workspace focus, filter draft) from `music_content` into `DestinationSurface`; move Music's key interpretation (`[`/`]`, `/`, Enter, `n`, `a`, `s`, list movement) into `DestinationSurface::key`, as `QueueSurface::key` does (design D1); `MusicWorkspaceComponent` becomes an adapter whose `on_key` delegates. Verify: `cargo nextest run -p mbv-components` and the `tick_integration` Music tests, key tests included, pass unedited.
- [ ] 1.4 Add `LibrarySlotContent` in `crates/mbv-ui-model/src/library_slots.rs` and a public Music projection in `src/app/state/projection/` that builds it from `App::libs` and the Music `DestinationSurface`; move `panel_content`'s pill, tree, Hero (`resolved_hero_data`), and Workspace derivations into it, and build `MusicWorkspaceComponent`'s `LibraryPanelContent` from it (design D2). Move the component tests that assert those derivations (`music_content/tests.rs:328-349` pills, `tests.rs:352` Hero) to the projection's tests with the same fixtures and expected results, and delete them from the component. Verify: the moved tests pass in their new place; the Music TUI paint, tick, and key tests pass unedited; a `tests/` projection test builds an album-selected Music fixture with square art and the TRACKLIST Workspace.
- [ ] 1.5 Port `inline_search` state and key rules into the destination's filter draft in `DestinationSurface` (Music's `/` filter, the letter rail's keyboard path); `InlineSearch` becomes an adapter, and the Music projection carries the `Search` list slot. Carry the filter state (open, query) on `GuiLibraryFrame` and paint the filter bar under the selector row (design D2). Verify: the Inline Search and Music tests pass unedited; vitest paints the bar with its query; a core test opens the filter with `/`, filters on typed text, and closes it with Esc.
- [ ] 1.3 Move the tab selection from `tab_panel` into `LibrarySurface`; `tab_panel` becomes an adapter, and `LibraryPanel` keeps reading Panel focus from the `FocusStack` as its `panel_focused` paint input. Verify: the existing `library_panel` and `tab_panel` tests pass unedited; `docs/architecture/interactive-surface-ledger.md` names the new Surface state for the Library, Tab, and Music rows.

## 2. Multi-select collection pills and persistence (Music)

- [ ] 2.0 Bind the selector chords in the GUI-only key context (design D8). `{` and `}` are gated on Library Panel focus only; they move the focused pill and give the selector pill focus. Space, Alt+Space, and Backspace are gated on Library Panel focus plus pill focus; they toggle, solo, and clear, and resolve ahead of `toggle_play_pause` and the Visual-selection candidate. `[` and `]` keep their TUI effect. Verify: `src/app/input/gui/tests.rs` resolves `}` with Library focus and no pill focus and asserts that pill focus is then held; resolves Space, Alt+Space, and Backspace with pill focus; resolves Space to play/pause without pill focus, with Queue focus, and after `]`, and leaves the TUI resolution of all six unchanged.
- [ ] 2.1 Load each selected Music collection with its existing per-collection fetch and derive the artist list as the union de-duplicated by item id, sorted by sort name (design D5). Verify: a hermetic owner test with two mocked collections that share one artist yields that artist once, in sort order.
- [ ] 2.2 Add `extra_music_groups` to `TuiLaunchState` (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`) and write it from the Music `SelectorSurface` set in the shared `launch_state_snapshot`, only when Music is the selected tab; the builder keeps the Audiobookshelf bucket scope (design D6). Verify: a shared snapshot test writes no extra groups when another tab is selected; `crates/mbv-config` tests round-trip a two-group state and load a file without the field; a shared snapshot test writes no extra groups for a one-group set.
- [ ] 2.3 Add the shared launch-snapshot builder over `LibrarySurface` and the `FocusStack`; it writes `item: None` (design D6); `Model::launch_state_snapshot` uses it, and the GUI core thread calls it and saves through `mbv_config::save_tui_launch_state` at its orderly exit, before `client_exit_teardown` (design D6). Verify: the existing TUI launch-state tests pass unedited; a `crates/mbv-gui/tests/` nextest saves Music, Library focus, and two groups at GUI exit into a temporary state path and writes nothing before exit.
- [ ] 2.4 Update `CONTEXT.md`: the *TUI launch state* entry (the GUI Client also writes the snapshot at orderly exit; the snapshot carries the GUI's extra selected Music groups), and new entries for *Collection pill*, *Pill focus* (the selector mode, distinct from the focused pill), *Letter rail*, and *Library Status bar*. Verify: each entry exists and the launch-state entry names both Clients and the extra groups.
- [ ] 2.5 Apply `extra_music_groups` in the GUI core thread's startup adoption, after the shared `reanchor_launch_state` restores the first group; the shared restore path stays unchanged (design D6). Verify: a `crates/mbv-gui/tests/` nextest restores Hip-Hop and Soul from a two-group snapshot and drops a missing extra group without a substitute; a TUI launch test restores Hip-Hop only from the same snapshot.

## 3. Frame, regions, and leaf

- [ ] 3.1 Add `GuiLibraryFrame` under `crates/mbv-gui/src/bridge/frame/library.rs`, built from the Music `LibrarySlotContent` projection and the adopted Surface state (design D2), with artwork carried from the image cache, and the field on `GuiFrame`. Verify: a `crates/mbv-gui/tests/` test projects a Music fixture into tabs, pills with the selected set, viewport rows, Hero facts with square art, and the TRACKLIST Workspace.
- [ ] 3.2 Add `LibraryTargetId` and the `LibraryList` and `LibraryWorkspace` `ListId`s to `src/bridge/layout.rs`, mirroring the Library hit targets, the Hero's play, play-next, add, shuffle-artist, and more-actions targets included (design D3). Verify: a nextest maps each reported region to its target and applies whole-row capacity to the Music lists.
- [ ] 3.3 Add the Library `GuiSurfaceLeaf` in `src/leaves.rs`: pill press and Alt-press, clear, tab, row press, letter, Hero action, Workspace row and row actions, overlay close; unported targets are documented no-ops. Verify: a nextest per gesture family asserts the emitted `Msg`.

## 4. Core wiring and the window arrangement

- [ ] 4.1 Mount the Library in the core thread's `FocusStack`, hold `LibrarySurface`, adopt the Music projection once per emitted frame, and route keys and resolved gestures through the leaf; Panel focus moves by the TUI key and by a press. Verify: a core-thread test moves Panel focus by key and by a Library row press.
- [ ] 4.2 Report the Library column width and height in `LayoutReport`; the core classifies them with `mbv_surfaces::breakpoint::classify` (`mini_below = 480`, `wide_from = 880`) and `hero_side_by_side` (600 px guard), feeds the class to the `App`'s Mini check, and carries it on the frame as a bridge enum (design D4). Verify: a nextest classifies the three classes and the `z` rail width, keeps cursor and pill set across a class change, and keeps Mini focus ephemeral after widening; vitest that the arrangement follows the frame's class.
- [ ] 4.3 Open and close the Library Hero overlay in Narrow by the TUI's keys, with the state in the core. Verify: a core-thread test opens the overlay in Narrow and finds no overlay state in Wide.
- [ ] 4.4 Port the Library item context menu as a GUI overlay: the menu state in the core, a Library item menu section on `GuiOverlayFrame`, `ContextMenuTargets::Emby(_)` routed in place of its documented no-op in `src/app/input/gui.rs`, opened by the Hero's more-actions target and by the TUI's context-menu chord (design D11). Verify: a nextest opens the menu by the target and by the chord and runs one entry's TUI action.
- [ ] 4.5 Update `docs/architecture/gui-client.md`: the Library surface, the arrangement and thresholds, and the Status bar placement. Verify: the doc names each new frame field, target id, and `ListId`.

## 5. Front end shell

- [ ] 5.1 Add `LibraryPanel.svelte` and the window arrangement in `App.svelte`: Mini shows the focused panel, Narrow shows the browser pane plus the Queue panel, Wide adds the Hero pane; regions and capacities report through `reportLayout` and retract on unmount. Verify: vitest asserts the mounted panels per class and the reported contribution.
- [ ] 5.2 Add the top bar (Home icon and destination tabs) in the canvas look, with GUI-local `--canvas-*` tokens for Library-only values (design D10). Verify: vitest asserts one tab per frame tab, the current tab marked `aria-current`, and a tab press sending its region.
- [ ] 5.3 Paint the Library Status bar under the Library column while the Library shows: the shared prefix and toast, the GUI-only key hints (action id plus the configured chord, carried on the frame), and the location (destination, selected pills, selected item); prefix or toast replaces the hints; the Queue footer keeps prefix and toast when only the Queue shows (design D9). Verify: a nextest that a rebound chord reaches the frame hint; vitest for both placements with the prefix armed and for the toast replacing the hints.

## 6. Music slot painters

- [ ] 6.1 Paint the selector row: pills with `aria-pressed`, the check on selected pills, and the selected-count clear control; no options control (design D3). Verify: vitest for one and two selected pills.
- [ ] 6.2 Paint the browser list: the artist, album, and track tree rows at fixed heights, letter headings, and the cursor row; letter headings are non-selectable rows at the fixed row height (design D12); the list paints only the frame's viewport rows. Verify: vitest asserts headings plus rows equal the reported capacity and the cursor row is marked; a core test that the cursor skips a heading.
- [ ] 6.3 Paint the letter rail from the frame's present letters; a press sends the letter region (design D7). Verify: vitest asserts absent letters are disabled and a present letter press sends its region; a core test moves the cursor to the first matching sort name.
- [ ] 6.4 Paint the Hero: artist header, square artwork slot from the frame image, album facts, the Shuffle artist control, and the Play, Play next, Add to queue, and more-actions controls with their key hints; the more-actions control opens the item menu in the row-menu look (design D11). Verify: vitest asserts each action control sends its region and shows its hint, and the item menu paints from its overlay section.
- [ ] 6.5 Paint the Workspace tracklist: header, fixed-height rows, and the hover play-next and add controls. Verify: vitest asserts hover controls send their regions and the row count equals capacity.
- [ ] 6.6 Paint the Library Hero overlay for Narrow with the same Hero and Workspace painters. Verify: vitest mounts the overlay from a Narrow frame with the overlay open.

## 7. Phase 1 checks

- [ ] 7.1 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `cargo fmt --all -- --check`, `make ui-check ui-lint ui-test`, and `make check-code-file-lines` before the Phase 1 push. Verify: all green.

## Phase 2 — other destinations and Search

## 8. Shows

- [ ] 8.1 Port `tv_content` state (series tree, flat Latest carrier, season cursor, episode Workspace, Workspace focus) into `DestinationSurface` and its key rules into `DestinationSurface::key` (design D1), and its slot-content derivation into the destination's `LibrarySlotContent` projection in `src/app/state/projection/` (design D2); the component becomes an adapter whose `on_key` delegates and whose `LibraryPanelContent` is built from the projection. Move the component tests that assert a moved derivation to the projection's tests with the same fixtures and expected results (design D2). Verify: the moved tests pass in their new place; its paint, tick, and key tests pass unedited.
- [ ] 8.2 Project Shows from its `LibrarySlotContent` into `GuiLibraryFrame`: landscape or portrait art, the season-pill Workspace selector, no Workspace in flat mode, and no Hero for Upcoming. Verify: nextest projections for the three modes.
- [ ] 8.3 Paint the Workspace selector and the landscape and portrait art arms; Shows pills stay single-select (design D5) and series get the letter rail. Verify: vitest for the Workspace selector and each art arm; a core test that a Shows pill press replaces the selection.

## 9. Movies and other Emby libraries

- [ ] 9.1 Port `emby_library_content` state into `DestinationSurface` and its key rules into `DestinationSurface::key` (design D1), and its slot-content derivation into the destination's `LibrarySlotContent` projection in `src/app/state/projection/` (design D2); the component becomes an adapter whose `on_key` delegates and whose `LibraryPanelContent` is built from the projection. Move the component tests that assert a moved derivation to the projection's tests with the same fixtures and expected results (design D2). Verify: the moved tests pass in their new place; its paint, tick, and key tests pass unedited.
- [ ] 9.2 Project and paint: the Hero without a Workspace, the artwork arm from the artwork policy, letter pills single-select, feed-group pills as collection pills in feed-group mode (design D5), and the letter rail. Verify: nextest projection; vitest for a Hero without Workspace; a union test for two feed groups.

## 10. Home

- [ ] 10.1 Port `home_content` state into `DestinationSurface` and its key rules into `DestinationSurface::key` (design D1), and its slot-content derivation into the destination's `LibrarySlotContent` projection in `src/app/state/projection/` (design D2); the component becomes an adapter whose `on_key` delegates and whose `LibraryPanelContent` is built from the projection. Move the component tests that assert a moved derivation to the projection's tests with the same fixtures and expected results (design D2). Verify: the moved tests pass in their new place; its paint, tick, and key tests pass unedited.
- [ ] 10.2 Project and paint the Home sections and Hero, with no letter rail. Verify: nextest projection; vitest asserts no rail.

## 11. Audiobooks

- [ ] 11.1 Port `book_content` state (carrier, chapter Workspace, chapter focus) into `DestinationSurface` and its key rules into `DestinationSurface::key` (design D1), and its slot-content derivation into the destination's `LibrarySlotContent` projection in `src/app/state/projection/` (design D2); the component becomes an adapter whose `on_key` delegates and whose `LibraryPanelContent` is built from the projection. Move the component tests that assert a moved derivation to the projection's tests with the same fixtures and expected results (design D2). Verify: the moved tests pass in their new place; its paint, tick, and key tests pass unedited.
- [ ] 11.2 Project and paint the book Hero and the chapter Workspace without a header, and no letter rail (no inline filter, design D7). Verify: nextest projection; vitest for the headerless Workspace.

## 12. Podcasts

- [ ] 12.1 Port `podcast_content` state into `DestinationSurface` and its key rules into `DestinationSurface::key` (design D1), and its slot-content derivation into the destination's `LibrarySlotContent` projection in `src/app/state/projection/` (design D2); the component becomes an adapter whose `on_key` delegates and whose `LibraryPanelContent` is built from the projection. Move the component tests that assert a moved derivation to the projection's tests with the same fixtures and expected results (design D2). Verify: the moved tests pass in their new place; its paint, tick, and key tests pass unedited.
- [ ] 12.2 Project and paint the episode list and Hero, show pills as collection pills (design D5), and no letter rail. Verify: nextest projection; a union test for two shows in episode date order.

## 13. Feeds

- [ ] 13.1 Port `feeds_content` state into `DestinationSurface` and its key rules into `DestinationSurface::key` (design D1), and its slot-content derivation into the destination's `LibrarySlotContent` projection in `src/app/state/projection/` (design D2); the component becomes an adapter whose `on_key` delegates and whose `LibraryPanelContent` is built from the projection. Move the component tests that assert a moved derivation to the projection's tests with the same fixtures and expected results (design D2). Verify: the moved tests pass in their new place; its paint, tick, and key tests pass unedited.
- [ ] 13.2 Project and paint Feeds, subscription pills as collection pills (design D5), and no letter rail. Verify: nextest projection; a union test for two subscriptions.

## 14. Global Search and Inline Search

- [ ] 14.1 Port `search_sidebar` state and its key rules (query editing, cursor, type filter) into `SearchSurface` with a `key` method; the component becomes an adapter whose `on_key` delegates. Verify: its tests, key tests included, pass unedited.
- [ ] 14.2 Bind `Ctrl+K` to `search_open` in the GUI-only key context (design D8). Verify: a keybind test resolves `Ctrl+K` in the GUI context and leaves the TUI defaults unchanged.
- [ ] 14.3 Add the `SearchResults` `ListId` and Search targets; project the Search section; paint the top-bar field and the results in the Music row look. Verify: nextest for activation `Msg`; vitest for the field and the results capacity.
- [ ] 14.4 Update `docs/architecture/gui-client.md` and the surface ledger for the Phase 2 owners and Search. Verify: each ported owner's ledger row names its Surface state.

## 15. Phase 2 checks

- [ ] 15.1 Run the gates from 7.1 and `make check-code-file-lines`. Verify: all green.

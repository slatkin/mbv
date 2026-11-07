# Design: add-gui-library-panel

## Context

See `proposal.md` for the why and `specs/gui-client/spec.md` for the requirements. The canvas reference is in `canvas/Library.dc.html` and `canvas/canvas.json` (a verbatim copy of the Library artboard source and the canvas index, with the SPEC note on multi-select pills). The copy is a source to read, not a page to open: its `./support.js` runtime belongs to the canvas host. To see it rendered, open the live canvas, `https://claude.ai/artifact/D8uLGS6NyCeHd6ed7PHgsr` (the *Library — Music › Hip-Hop* artboard).

What the code holds today:

- Every TUI destination owner gives the Library panel one typed paint input, `LibraryPanelContent` (`crates/mbv-components/src/library_panel/content.rs`). It has a `SelectorRow`, a `ListSlot` (`Media`, `Empty`, `Search`), and a `HeroSlot` (`Absent` or `Pane`). A `HeroContent` has `HeroFacts` with an artwork shape arm, and an optional `Workspace` (header, selector, list).
- The interaction state of the Library is in the TUI components: `LibraryPanel`, the seven owners (`music_content`, `tv_content`, `emby_library_content`, `home_content`, `book_content`, `podcast_content`, `feeds_content`), `tab_panel`, `inline_search`, and `search_sidebar`. `mbv-surfaces` holds only queue, queue playback, status bar, list, media list, seekbar, gesture, hit map, focus stack, capacity, and breakpoint.
- `mbv_surfaces::breakpoint` defines `Mini | Narrow | Wide`, `classify`, and `hero_side_by_side`. No Client calls them yet.
- The GUI core thread owns an `App`, emits `GuiFrame`, and applies `LayoutReport` (`docs/architecture/gui-client.md`). The guide's surface order (*Adding a surface to the GUI*) applies to every surface in this change.
- The launch state is `TuiLaunchState` (`crates/mbv-config/src/launch_state.rs`). The `App` loads it at construction for both Clients, but only the TUI saves it: `Model::teardown` builds the snapshot from the mounted TuiRealm `LibraryPanel` (`Model::launch_state_snapshot`), and the GUI's `App::client_exit_teardown` saves nothing by design. The selector is one `SelectorIdentity` for the active destination.

## Goals / Non-Goals

**Goals:**

- One Library painter for each slot in the GUI, used by every destination.
- The TUI's interaction rules run once, in the Client core, for both Clients.
- Music is complete and accepted before the other destinations start.

**Non-Goals:**

- No change to the TUI's paint, key output, or behaviour.
- No Owner-side, ctrl, or Player change. No new dependency.
- No canvas artboards for the other destinations: their look derives from the Music slots (proposal).
- No restyle of the Queue panel.

## Decisions

### D1. Port each owner's state into one Library Surface state in `mbv-surfaces`

A new `mbv_surfaces::library` module holds `LibrarySurface`: the tab selection (active destination) and one `DestinationSurface` per mounted destination. Panel focus between Library and Queue stays in the core `FocusStack`, its one authority (`client-interaction-core` "The core focus stack is the one focus authority"); `LibrarySurface` holds no copy of it, and `LibraryPanel`'s `panel_focused` stays a paint input read from the focus stack. The focus between the browser pane and the Workspace inside a destination is that destination's Workspace focus bit in `DestinationSurface`. A `DestinationSurface` holds a `SelectorSurface` (pills, selected set, focused pill), the browser list state (the existing `list`/`media_list` Surface types, and the tree state for Music and Shows), the Workspace list and selector state, the Workspace focus bit, and the filter draft. `SearchSurface` holds the Search sidebar's query, type filter, and result list.

The TUI components become adapters over this state (the ledger rule in `docs/architecture/interactive-surface-ledger.md`). They keep geometry, paint policy, and raw-input translation. Each port is behaviour-preserving: the existing TUI component and tick tests are the proof and stay green unchanged.

**The key rules move with the state.** Library navigation keys are component-local today: each owner's `on_key` matches `[`/`]`, `/`, Enter, `n`, `a`, `s`, and the list movement keys as fixed `Key::Char` values (for example `music_content/owner.rs:413`, `podcast_content/panel.rs:253`); the registry declares none of them. Each port moves the owner's key interpretation into a `key(&mut self, KeyEvent) -> LeafKeyResult` on its `DestinationSurface`, the precedent being `QueueSurface::key` (`crates/mbv-surfaces/src/queue.rs:415`). The TUI owner's `on_key` and the GUI Library leaf both delegate to it, so the GUI gets every TUI Library key with no second copy. The existing TUI key tests are the proof. "Raw-input translation" in the adapter means only crossterm-to-`KeyEvent` and mouse-to-gesture translation.

*Alternative:* the GUI core thread drives the TUI components directly. Rejected: the components are TuiRealm types and own their state, which the guide forbids a second Client to reimplement or reach into.

### D2. One GUI-neutral slot-content projection, used by both Clients

`LibraryPanelContent` cannot feed the GUI: it is an `mbv-components` type that borrows `&mut dyn PanelList` from the TUI's mounted owners, and `mbv-gui` cannot depend on `mbv-components` (TuiRealm and ratatui) and has no owner map. Every GUI frame section is built in `crates/mbv-gui/src/bridge/frame.rs` from public `mbv::App` projections plus GUI state.

So the slot content gets a plain model, `LibrarySlotContent` in `crates/mbv-ui-model/src/library_slots.rs`: the selector row (pills with stable identities, kinds, selected set, focused pill), the list rows (tree nodes or media rows, with non-selectable letter headings), the Hero (facts, artwork shape, artwork image key, actions with their action ids, overview, credits), and the Workspace (header, selector, rows). It holds owned data, no borrow and no `PanelList`. For each destination, a public `App` projection in `src/app/state/projection/` builds it from the `App`'s Service data (`App::libs` and its `nav_stack`, as `music_wide.rs` already does for Music) plus that destination's `DestinationSurface`.

The derivations that build slot content today in the owners (`MusicWorkspaceComponent::panel_content` and `resolved_hero_data`, `TvContent`'s selector and Hero, and the five siblings) move into these projections. Each TUI owner then builds its `LibraryPanelContent` from the same `LibrarySlotContent`, so each derivation exists once and the TUI paints exactly what it paints today; the existing TUI tests are the proof.

The core thread builds `GuiLibraryFrame` (`crates/mbv-gui/src/bridge/frame/library.rs`) from `LibrarySlotContent` plus the adopted Surface state, once per emitted frame. It carries: the breakpoint class, the tabs, the selector row (pills, selected set, kinds), the list rows of the viewport only, the letter-rail letters present, the Hero (facts, artwork shape, artwork image reference from the image cache, actions with their key hints, overview, credits), the Workspace (header, selector, viewport rows), whether the Hero overlay is open, and the Search section. Text stays glyph-free. The destination never reaches the frame as a key the front end branches on: the front end paints only the slots the frame carries (slot content with no Hero gives no Hero).

### D3. Regions, list ids, and the leaf

`LibraryTargetId` mirrors the Library hit-target enum variant for variant: tab, pill, pill clear, list row, letter, Hero action (play, play next, add to queue, shuffle artist, more actions), Workspace pill, Workspace row, Workspace row action, overlay close, Search field, Search result. `ListId` gains `LibraryList`, `LibraryWorkspace`, and `SearchResults`; each reports whole rows as capacity. A `GuiSurfaceLeaf` for the Library resolves gestures against the reported regions and emits the same semantic `Msg`s as the TUI Library pointer path. A target the GUI has not ported is a documented no-op.

### D4. The breakpoint is classified on the Library column width

The front end measures the Library column (the window width minus the Queue panel's current width, 400 px expanded or 72 px rail) and reports its width and height in `LayoutReport` beside `container_size`. The core classifies it with the shared `mbv_surfaces::breakpoint::classify` and `hero_side_by_side`, their first non-test caller, and the frame carries the class as a GUI bridge enum (`crates/mbv-gui/src/bridge/frame/library.rs`), so `mbv-surfaces` needs no serde. The front end arranges the panels from the frame's class. Thresholds, from the canvas sizes: `mini_below = 480` (the 320 px list plus the pill row's minimum) and `wide_from = 880` (the 320 px list plus a 560 px Hero: 300 px art, 64 px padding, and the text column). The height guard for `hero_side_by_side` is 600 px. In Mini, the panel with Panel focus takes the whole window. Mini focus is ephemeral, as in the TUI: the reported class feeds the `App`'s Mini check in place of the terminal-width test (`src/app/state/panel_focus.rs`), so a focus move in Mini writes `mini_view_focus`, not the stored Panel focus, and does not survive a widening. The core uses the class for the frame, for Mini focus, and for the Hero overlay rule. The trade-off: after a resize, the arrangement follows on the next frame, not in the same paint; the queue panel's own four states keep their front-end measurement.

*Alternative:* classify on the window width. Rejected: the `z` rail frees 328 px, and the Library should use it. *Alternative:* classify in the front end. Rejected: the core needs the class for Mini focus and the Hero overlay, and the shared classifier would stay dead.

### D5. Multi-select is a selector kind rule in the core

`SelectorSurface` marks each pill as a collection pill or a single pill. The owner sets the kind when it builds the pill row:

| Destination | Collection pills | Single pills |
| --- | --- | --- |
| Music (grouped) | every group pill | none |
| Podcasts | every show pill | New, the played-state filters |
| Feeds | every subscription pill (`FeedGroupKey::Feed`) | Latest, the filters, All |
| Emby library in feed-group mode | every feed-group pill | New, All |
| Shows, Movies and other Emby libraries, Home, Audiobooks | none | every pill |

A pill names a collection when it narrows the list to one content set the Service holds (a group folder, a show, a subscription); Latest, filters, and letter or surname buckets are views, not sets. A collection pill toggles; a single pill (Latest, filters, letter buckets) clears the set and selects alone. The core keeps at least one pill selected and gives the clear control the first selected pill in pill order. The owner loads each selected collection with its existing per-collection fetch, keyed by the collection's stable id, and the core derives the list as the union de-duplicated by item id, in the destination's existing sort. The TUI adapter never toggles; it selects one pill as today, so its set always has one member.

### D6. The extra pills persist in an additive launch-state field

The `tui-launch-state` main spec persists one pill scope and bans multi-selection, so this change amends it (delta `specs/tui-launch-state/spec.md`). Only a Grouped Music library persists a live collection choice today (its selected group); every other destination with a Latest pill restores Latest. So only Music's extra groups persist: `TuiLaunchState` gains `#[serde(default, skip_serializing_if = "Vec::is_empty")] extra_music_groups: Vec<String>` (the group content ids after the first, in pill order). `selector` keeps the first selected group.

The builder writes `item: None`, as every owner's `launch_snapshot` does today (`music_content/owner.rs:148` and siblings), so the TUI snapshot stays byte-identical and the field stays as it is. The user chose one shared file: the GUI Client writes the same snapshot at its orderly exit, and the last completed exit of either Client sets the next launch of both (the existing two-TUI rule; delta `tui-launch-state` "Launch state is written only at orderly TUI exit"). After the port, one snapshot builder reads `LibrarySurface` (tab, the selector set with its stable `SelectorIdentity` values) and the `FocusStack` (Panel focus); `Model::launch_state_snapshot` calls it in place of reading the component, so the TUI's snapshot stays byte-identical, and the GUI core thread calls it at its exit boundary and saves through `mbv_config::save_tui_launch_state` before `client_exit_teardown`. The GUI saves only at orderly exit, never while open. The builder writes the extra groups from the Music `SelectorSurface` set, which the TUI never grows past one, so a TUI snapshot carries none. The restore stays split: the shared `reanchor_launch_state` path keeps restoring `selector` only, unchanged, for both Clients; the GUI core thread applies `extra_music_groups` to the Music `SelectorSurface` in its own startup adoption, after the shared restore resolved the first group. The TUI has no apply site, so it restores the first group only. A saved extra group that the library no longer has is dropped, not replaced: the first-guaranteed-pill fallback applies only to the first group. In the session, each destination keeps its set because owners stay mounted.

The main spec's "SHALL NOT record the selected library item" already disagrees with the struct's `item` field; that drift predates this change and the delta does not touch it.

### D7. The letter rail reads the list's sort names

The frame carries the letters that have rows. A press emits a jump `Msg`; the core moves the cursor to the first row whose sort name starts with the letter (the sort-name rule the destination already sorts with) and re-anchors the viewport. The rail follows the list's current sort, not the destination: it shows only while the list is sorted by name (Music artists, Shows series, Audiobook books, and an Emby library while its sort is by name) and never on a Latest view or a date-sorted list (Home, Feeds, Podcast episodes, an Emby library sorted by date).

### D8. `Ctrl+K` binds Search in the GUI-only key context

`Ctrl+K` binds `search_open` in the GUI-only key context that already holds `z`, `q`, and `c`. The TUI's defaults do not change. The Search results paint in the Music list-row look.

The same GUI-only context binds `{` and `}` to move the focused pill back and forward without changing the selection, and Space and Alt+Space to toggle and solo the focused pill (design D5). `[` and `]` keep the TUI effect in both Clients: they move the focused pill and select it alone. The focused pill is `SelectorSurface` state; the TUI adapter keeps it equal to the selected pill.

**Gate and precedence.** Space is `toggle_play_pause`'s default chord, and the media list's Visual selection also claims it; `KeyPolicyGate::GuiClient` (`src/app/input/key_policy.rs:305`) tests only the Client kind, no overlay, and no text entry. So the selector chords get their own gate: the Library panel has Panel focus **and** the selector holds *pill focus*. `{` or `}` gives the selector pill focus (the focused pill paints its focus ring); any other Library navigation key, a press outside the pill row, or Esc returns pill focus to the list. While the selector holds pill focus, Space and Alt+Space resolve on the selector first, ahead of `toggle_play_pause` and the Visual-selection candidate; at every other time Space keeps its existing meaning in both panels. Pill focus is `SelectorSurface` state. The four chords stay canvas-fixed literals in the GUI-only key context, like the existing `z`, `q`, and `c` entries (`key_policy.rs:415-455`), so the keybind recipe in `docs/recipes.md` does not apply.

The Phase 1 gate's live test checks Alt+Space under GNOME on Wayland (the compositor can take Alt+Space for its window menu) and the `{`/`}` spelling with Shift on the user's layout.

### D9. The Status bar moves to the Library column

The shared Status bar projection carries the prefix state and the toast (`mbv_surfaces::status_bar`, `GuiStatusBar`). When the Library panel shows, those two paint in a Library Status bar under the Library column; when only the Queue panel shows, the Queue footer paints them as today.

The Library Status bar adds two GUI-only items that the canvas Library artboard shows (`canvas/Library.dc.html`, the bottom bar) and no Status bar carries today:

- **Key hints** (left): the main actions of the focused Library surface (filter, play, play next, add to queue, Panel focus, tab jump). The frame carries each hint as an action id plus the chord that runs it in the GUI: the configured chord for a registry action that `[keys]` binds (Panel focus, tab jump, search), and the component's fixed key for the rest (`/`, Enter, `n`, `a` are fixed `Key::Char` matches in the owners, not registry actions). The Hero action hints follow the same rule. Prefix or toast replaces the hints while active.
- **Location** (right): the destination name, the selected pill names, and the selected item title, each a fact the Library frame already carries. The front end joins them.

The TUI Status bar's other items (volume and mute pills, the Visual-mode indicator, the queue-source link, username, and Service glyphs) do not paint in the GUI: the canvas shows none of them, and the Queue footer already paints the queue source and the owner marker.

### D10. The canvas look comes through GUI-local tokens

The Library paints the canvas values through the GUI-local `--canvas-*` tokens in `crates/mbv-gui/ui/src/app.css`, with new tokens for the Library-only values. No `mbv-theme` slot changes.

### D11. The Hero's more-actions control opens the Library item context menu

The canvas Hero has *More album actions* and *Shuffle artist* (`canvas/Library.dc.html:99,114`). Shuffle artist runs the TUI owner's existing shuffle action (`s`, `MusicTreeAction::Shuffle`). More actions opens the TUI's Library item context menu for the Hero's item: the GUI ports it as an overlay, with the menu state in the core (the same overlay path as the queue row menu: `GuiOverlayFrame` gains a Library item menu section, and the GUI's documented no-op for `ContextMenuTargets::Emby(_)` in `src/app/input/gui.rs:334` becomes a routed target). It paints in the row-menu look and opens from the keyboard by the TUI's context-menu chord on the selected item.

### D12. Letter headings are non-selectable rows at the fixed row height

The canvas list interleaves 24 px letter headings with 30 px rows. A fixed row height is required so that capacity is a whole number of rows, so a heading is a non-selectable row of the list flow at the same row height. `ListCapacity` counts every row, headings included; the cursor and the paging stride skip headings, as they skip any non-selectable row. The heading paints at the canvas heading's type and colour inside the taller row.

## Risks / Trade-offs

- [The seven owner ports are large and touch TUI code] → Each port is behaviour-preserving and its proof is the existing TUI test suite, unchanged. Music lands first, so the port pattern is reviewed before it repeats.
- [A union of collections makes one fetch per selected collection] → The owner reuses the existing fetch and cache per collection; a selected collection that fails to load keeps the others and shows the TUI's failure text for that collection.
- [An older `mbv` binary may read a launch state with `extra_music_groups`] → `TuiLaunchState` has no `deny_unknown_fields`, so serde ignores the field and the older binary restores the first pill.
- [The breakpoint thresholds may feel wrong live] → They are constants beside the front end's panel-state constants; the Music gate's live test checks them.

## Migration Plan

One change, two phases with an acceptance gate:

1. **Phase 1 — Music from end to end.** The Library Surface state and the Music port, the frame section, regions, leaf, and core wiring, the window arrangement (breakpoints, Panel focus, Hero overlay), the top bar tabs (only the destinations the frame can project; the other tabs are absent until their unit lands, under the no-stub rule), the Status bar move, the Music slot painters, Music's Inline Search filter, the Hero's shuffle and more-actions menu, multi-select pills with persistence, and the letter rail.
2. **Gate.** A `/code-review` round over Phase 1 and a user live-test round. Phase 2 starts only after the user accepts Phase 1. The gate is not a task.
3. **Phase 2 — the other destinations and Search.** Shows, Movies and other Emby libraries, Home, Audiobooks, Podcasts, Feeds, then global Search. Each destination is one port plus its slot content; the painters from Phase 1 do not change except for slot variants that Music does not use (landscape and portrait art, the Workspace selector, no Hero).

Rollback is a per-unit revert. No Owner-side or stored-state change exists apart from the additive launch-state field.

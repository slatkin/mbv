# Design: add-gui-library-panel

## Context

See `proposal.md` for the why and `specs/gui-client/spec.md` for the requirements. The canvas reference is in `canvas/Library.dc.html` and `canvas/canvas.json` (a verbatim copy of the Library artboard and the canvas index, with the SPEC note on multi-select pills).

What the code holds today:

- Every TUI destination owner gives the Library panel one typed paint input, `LibraryPanelContent` (`crates/mbv-components/src/library_panel/content.rs`). It has a `SelectorRow`, a `ListSlot` (`Media`, `Empty`, `Search`), and a `HeroSlot` (`Absent` or `Pane`). A `HeroContent` has `HeroFacts` with an artwork shape arm, and an optional `Workspace` (header, selector, list).
- The interaction state of the Library is in the TUI components: `LibraryPanel`, the seven owners (`music_content`, `tv_content`, `emby_library_content`, `home_content`, `book_content`, `podcast_content`, `feeds_content`), `tab_panel`, `inline_search`, and `search_sidebar`. `mbv-surfaces` holds only queue, queue playback, status bar, list, media list, seekbar, gesture, hit map, focus stack, capacity, and breakpoint.
- `mbv_surfaces::breakpoint` defines `Mini | Narrow | Wide`, `classify`, and `hero_side_by_side`. No Client calls them yet.
- The GUI core thread owns an `App`, emits `GuiFrame`, and applies `LayoutReport` (`docs/architecture/gui-client.md`). The guide's surface order (*Adding a surface to the GUI*) applies to every surface in this change.
- The launch state is `TuiLaunchState` (`crates/mbv-config/src/launch_state.rs`). The `App` loads it at construction and saves it at exit, so the GUI and the TUI already share one file. The selector is one `SelectorIdentity` for the active destination.

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

A new `mbv_surfaces::library` module holds `LibrarySurface`: Panel focus between Library and Queue, the active destination, and one `DestinationSurface` per mounted destination. A `DestinationSurface` holds a `SelectorSurface` (pills, selected set, focused pill), the browser list state (the existing `list`/`media_list` Surface types, and the tree state for Music and Shows), the Workspace list and selector state, the Workspace focus bit, and the filter draft. `SearchSurface` holds the Search sidebar's query, type filter, and result list.

The TUI components become adapters over this state (the ledger rule in `docs/architecture/interactive-surface-ledger.md`). They keep geometry, paint policy, and raw-input translation. Each port is behaviour-preserving: the existing TUI component and tick tests are the proof and stay green unchanged.

*Alternative:* the GUI core thread drives the TUI components directly. Rejected: the components are TuiRealm types and own their state, which the guide forbids a second Client to reimplement or reach into.

### D2. One frame section built from `LibraryPanelContent`

The core thread builds `GuiLibraryFrame` (`crates/mbv-gui/src/bridge/frame/library.rs`) from the active owner's `LibraryPanelContent` plus the adopted Surface state, once per emitted frame. It carries: the breakpoint class, the tabs, the selector row (pills, selected set, kinds), the list rows of the viewport only, the letter-rail letters present, the Hero (facts, artwork shape, artwork image reference from the image cache, actions with their key hints, overview, credits), the Workspace (header, selector, viewport rows), whether the Hero overlay is open, and the Search section. Text stays glyph-free. The destination never reaches the frame as a key the front end branches on: the front end paints only the slots the frame carries (`HeroSlot::Absent` gives no Hero).

### D3. Regions, list ids, and the leaf

`LibraryTargetId` mirrors the Library hit-target enum variant for variant: tab, pill, pill clear, list row, letter, Hero action, Workspace pill, Workspace row, Workspace row action, overlay close, Search field, Search result. `ListId` gains `LibraryList`, `LibraryWorkspace`, and `SearchResults`; each reports whole rows as capacity. A `GuiSurfaceLeaf` for the Library resolves gestures against the reported regions and emits the same semantic `Msg`s as the TUI Library pointer path. A target the GUI has not ported is a documented no-op.

### D4. The breakpoint is classified on the Library column width

The front end measures the window and the Queue panel's current width (400 px expanded, 72 px rail). The Library column width is the window width minus the Queue panel width. Thresholds, from the canvas sizes: `mini_below = 480` (the 320 px list plus the pill row's minimum) and `wide_from = 880` (the 320 px list plus a 560 px Hero: 300 px art, 64 px padding, and the text column). The height guard for `hero_side_by_side` is 600 px. In Mini, the panel with Panel focus takes the whole window. The front end reports the class with the container size in `LayoutReport`; the core stores it for the frame and for the Hero overlay rule, and does not classify.

*Alternative:* classify on the window width. Rejected: the `z` rail frees 328 px, and the Library should use it.

### D5. Multi-select is a selector kind rule in the core

`SelectorSurface` marks each pill as a collection pill or a single pill. A collection pill toggles; a single pill (Latest, filters, letter buckets) clears the set and selects alone. The core keeps at least one pill selected and gives the clear control the first selected pill in pill order. The owner loads each selected collection with its existing per-collection fetch, keyed by the collection's stable id, and the core derives the list as the union de-duplicated by item id, in the destination's existing sort. The TUI adapter never toggles; it selects one pill as today, so its set always has one member.

### D6. The extra pills persist in an additive launch-state field

`TuiLaunchState` gains `#[serde(default)] extra_selectors: Vec<SelectorIdentity>`: the selected pills after the first, for the active destination. `selector` keeps the first selected pill, so the TUI restores it unchanged and ignores the new field. In the session, each destination keeps its set because owners stay mounted.

### D7. The letter rail reads the list's sort names

The frame carries the letters that have rows. A press emits a jump `Msg`; the core moves the cursor to the first row whose sort name starts with the letter (the sort-name rule the destination already sorts with) and re-anchors the viewport. Rails show on Music artists, Shows series, Emby library lists, and Audiobook books; not on Home, Feeds, or Podcast episodes.

### D8. `Ctrl+K` binds Search in the GUI-only key context

`Ctrl+K` binds `search_open` in the GUI-only key context that already holds `z`, `q`, and `c`. The TUI's defaults do not change. The Search results paint in the Music list-row look.

### D9. The Status bar moves to the Library column

When the Library panel shows, the Status bar (key hints, breadcrumb, prefix, toasts) paints under the Library column. The breadcrumb joins the destination, the selected pill names, and the selected item title from frame facts. When only the Queue panel shows, the Queue footer paints the Status bar states as today.

### D10. The canvas look comes through GUI-local tokens

The Library paints the canvas values through the GUI-local `--canvas-*` tokens in `crates/mbv-gui/ui/src/app.css`, with new tokens for the Library-only values. No `mbv-theme` slot changes.

## Risks / Trade-offs

- [The seven owner ports are large and touch TUI code] → Each port is behaviour-preserving and its proof is the existing TUI test suite, unchanged. Music lands first, so the port pattern is reviewed before it repeats.
- [A union of collections makes one fetch per selected collection] → The owner reuses the existing fetch and cache per collection; a selected collection that fails to load keeps the others and shows the TUI's failure text for that collection.
- [An older `mbv` binary may read a launch state with `extra_selectors`] → `TuiLaunchState` has no `deny_unknown_fields`, so serde ignores the field and the older binary restores the first pill.
- [The breakpoint thresholds may feel wrong live] → They are constants beside the front end's panel-state constants; the Music gate's live test checks them.

## Migration Plan

One change, two phases with an acceptance gate:

1. **Phase 1 — Music from end to end.** The Library Surface state and the Music port, the frame section, regions, leaf, and core wiring, the window arrangement (breakpoints, Panel focus, Hero overlay), the top bar tabs, the Status bar move, the Music slot painters, multi-select pills with persistence, and the letter rail.
2. **Gate.** A `/code-review` round over Phase 1 and a user live-test round. Phase 2 starts only after the user accepts Phase 1. The gate is not a task.
3. **Phase 2 — the other destinations and Search.** Shows, Movies and other Emby libraries, Home, Audiobooks, Podcasts, Feeds, then global Search. Each destination is one port plus its slot content; the painters from Phase 1 do not change except for slot variants that Music does not use (landscape and portrait art, the Workspace selector, no Hero).

Rollback is a per-unit revert. No Owner-side or stored-state change exists apart from the additive launch-state field.

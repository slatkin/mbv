# Proposal

## Why

The GUI is not built from components. Each destination and each surface paints its own copy of
the same shapes, so a list fix or a look change must go into each copy, and the copies drift.
Issue #1074 records the latest proof: `gui-unified-list-rows` (archived 2026-11-24) shared the
row parts only. Every list still composes those parts in its own painter, and `gui-tree-row-roles`
cannot land as one edit. Invariant 38 did not stop this. Its rule 2 lets a consumer compose shared
parts, its rule 3 lets each shape claim an exception, and `Element` props let a consumer hand its
own rsx to a shared part.

The repetition is in every layer of the GUI, not only in the row paint:

- **Lists**: six list painters (`LibraryList`, `LibraryShowsList`, `LibraryHomeList`,
  `LibraryLetteredList`, `PlaylistsPanel`, `QueuePanel`), plus the Workspace table.
- **Heroes**: seven Hero components, each composing `LibraryHeroFrame` with its own action row and
  list band through `Element` slots.
- **Controls**: 37 component files build their own `button`, and five build their own `input`.
- **Bars**: six separate headers, status bars, and footers.
- **Input**: two input systems. The Library uses Dioxus handlers, Blitz hover, and pointer
  markers. The Queue, the menus, the dialogs, and the player panel use region sets that the core
  hit-tests.
- **Destinations**: each destination has its own bridge frame types, its own window state and key
  file (about 3,000 lines in `window/library/`), and its own commands (53 `GuiLibraryCommand`
  variants).

## What Changes

This change is a structure-only refactor of the whole GUI Client. Every current look stays the
same.

- **One component per shape, in levels.** The logic of a shape lives at the highest level that all
  of its children share. A child adds to its parent and never re-implements it. The internals of
  a level are private to that level.
  - `LIST BASE` → `LIBRARY LIST` (→ `FLAT`, `TREE`) | `QUEUE` | `WORKSPACE`. `LIST BASE` is generic
    over a source that gives a key type and an extra type, so a Library row carries no Queue data.
  - `HERO BASE`, whose body is `None` or a `WORKSPACE` list. The Music artist page is a
    `WORKSPACE` list with `Heading` rows.
  - `CONTROL BASE`, `FIELD BASE`, `POPUP BASE` (→ `MENU`, `DIALOG`), `BAR BASE`, `SETTINGS ROW`,
    and `PANEL BASE`.
  - Menus and Settings rows are not lists of data. Each has its own single owner.
  - `LIBRARY` is a composite: `BAR` + `LIST` + `HERO` + `BAR`.
- **A base takes data, never an `Element`.** Only a composite puts bases together.
  `LibraryHeroFrame`'s `actions` and `children` slots, and every other `Element` prop on a base,
  go.
- **Destinations exist only in the core projection.** The core projects every destination into
  neutral frames: a list frame, a Hero frame, and a Playlists tree. The window sends neutral
  intents keyed by row identity (press, activate, toggle, menu, play, enqueue, shuffle, reorder,
  rename), and the core resolves their meaning for the destination. The per-destination bridge
  frames, window state, key files, list painters, Heroes, and `GuiLibraryCommand` variants go.
- **One input system: Dioxus Native.** The region sets, the core `HitMap`, and the core pointer
  path go from the GUI. Every component takes pointer input through Dioxus handlers, Blitz hover,
  and pointer markers. One drag gesture serves the Queue reorder and the scrubber seek.
- **One viewport.** Every list uses native scroll with a virtual window that `LIST BASE` cuts. The
  Queue and the Playlists tree become virtual lists. **BREAKING (behaviour):** the Queue scrolls by
  pixel like the Library lists, so a row can show in part at the viewport's edge. The Queue's
  cursor row still stays fully visible after each keyboard movement.
- **Interaction state.** The Queue's viewport, hover, and drag gesture move from the core to the
  window. Its active row, marks, prefix mode, folds, and open menu stay in the core.
- **Look.** Each mechanic has one implementation and one token name. A variant sets only the
  token's value at its root. No look changes.
- **Invariants.** Invariant 38 is rewritten: it states the whole-GUI scope, removes the rule 3
  exception and the consumer-composition loophole, forbids `Element` props on a base, and lists the
  real failures until this change closes them. Invariant 30 names the bases.

Sequencing: PR 1075 (`emby-favourites`) merges before this work starts, and this change converts
Favourites with the other destinations. `add-gui-pin-window` and `gui-tree-row-roles` stay on hold
until this change lands. After that, the pin window is a `PANEL` composite over the `QUEUE` list,
and the tree row redesign is token and `TREE` edits in one place.

Out of scope: the TUI, `mbv-client-core`, `mbv-surfaces`, the Owner, the ctrl protocol, and every
Service client. The TUI keeps `mbv-surfaces`, including the Queue viewport and the hit map.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`:
  - "Library lists and Workspace tables paint through shared parts" becomes the whole-GUI
    component requirement: one owner per shape, the levels, data-only bases, and destination-free
    window and bridge.
  - "Scrolling follows the cursor" drops whole-row capacity and keeps the cursor-reveal rule.
  - "The Client core holds the player panel's interaction state" moves the Queue viewport, hover,
    and drag gesture out of the core, and replaces raw pointer forwarding with intents.
  - "The window content holds the Library panel's interaction state" covers the Queue's viewport,
    hover, and drag gesture too.
  - "GUI pointer input resolves through semantic targets" states that every GUI surface resolves
    pointer input through its painted components.

## Impact

- `crates/mbv-gui/src/window/components/`: nearly every file. New level modules `list/`, `hero/`,
  `control/`, `field/`, `popup/`, `bar/`, `settings_row/`, `panel/`, and `library/`. The
  per-destination list painters and Heroes go.
- `crates/mbv-gui/src/window/library/`: the per-destination state and key files collapse into
  generic list and Hero state keyed by list identity.
- `crates/mbv-gui/src/window/layout/region_set.rs` and every region-set report go.
- `crates/mbv-gui/src/bridge/frame/library/` and `bridge/frame/playlists.rs`: per-destination
  frames become neutral frames. `bridge/frame/queue.rs` loses `hover`, `drag`, and `scroll`.
  `bridge/library.rs` and `bridge/playlists.rs`: commands become neutral intents.
- `crates/mbv-gui/src/gui_core/gui_library/*_projection.rs` and `gui_core/gui_playlists/`: project
  neutral frames and resolve intents.
- `crates/mbv-gui/src/core_thread.rs` and `core_thread/pointer.rs`: the GUI stops using
  `HitMap`, `PointerGestureState`, and the `QueueSurface` viewport.
- Tests: per-destination painter tests become base-level contract tests in `mbv-gui`, and
  projection tests in `gui_core`.
- Docs: invariants 30 and 38, `docs/architecture/gui-client.md`, `crates/mbv-gui/AGENTS.md`, and
  `CONTEXT.md` (the level names).
- No dependency change. The window stack stays on `dioxus-native =0.8.0-alpha.3` and
  `blitz-dom =0.3.0-beta.4`.

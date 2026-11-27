# Design

## Context

See `proposal.md` (Why) for the motivation. This section holds only the facts that shape the
approach.

- **Window stack.** `dioxus-native =0.8.0-alpha.3` over `blitz-dom =0.3.0-beta.4`
  (`crates/mbv-gui/Cargo.toml`, pinned by `replace-tauri-with-blitz` D2). Blitz has no pointer
  capture. It dispatches pointer down, move, and up to the hit node and bubbles them through the
  ancestors (`blitz-dom/src/events/driver.rs:315-410`). It handles wheel by scrolling the chain
  from the hovered node (`events/pointer.rs:828-850`). `handle_pointer_leave` exists
  (`events/driver.rs:67`).
- **Two input systems.** Path A: Library and Hero components take Dioxus handlers, Blitz resolves
  hover, CSS paints the hover veil, and `window/pointer_target.rs` markers name the key target.
  Path B: the Queue, the player panel, the Status bar, the menus, the dialogs, the attach screen,
  the folded queue, the compact strip, and the collapsed rail report region sets
  (`window/layout/region_set.rs`, `RegionTarget` in `bridge/layout.rs:75`). The core thread
  hit-tests raw pointer events against them through `mbv_surfaces::HitMap` and
  `PointerGestureState` (`core_thread.rs:22-26`, `core_thread/pointer.rs`), and sends `hover`,
  `drag`, and `scroll` back in `GuiQueue` (`bridge/frame/queue.rs:28-37`).
- **Three viewports.** Library lists: native scroll and a virtual window over `GeometryRow`
  (`library_virtual_list.rs`, `window/library/list/geometry.rs`). Queue: the core owns the scroll
  through `QueueSurface` and sends only the rows in view. Playlists and the Workspace table: every
  row, no virtual window.
- **Destinations in three layers.** Core projections per destination
  (`gui_core/gui_library/*_projection.rs`, about 3,100 lines), bridge frames per destination
  (`bridge/frame/library/*.rs`, 12 files), and window state and keys per destination
  (`window/library/`, about 3,000 lines). 53 `GuiLibraryCommand` variants and 9
  `GuiPlaylistsCommand` variants name destination acts.
- **The one pattern that works today.** `LibraryLetteredList<D: LetteredDestination>` paints four
  destinations through one component. This design extends that idea to every shape and moves the
  per-destination part of `D` into the core.
- **Spec constraints.** The Client core keeps the Queue's active row, marks, prefix mode, folds,
  and open menu (`gui-client`, "The Client core holds the player panel's interaction state").
  The Library panel holds no cursor (`gui-client`, "No Library list ... SHALL have a keyboard
  cursor").

## Goals / Non-Goals

**Goals:**

- Each GUI shape has one component, the components form levels, and the compiler stops a
  consumer from composing a shape's parts.
- The window and the bridge contain no destination code.
- One input system, one viewport, and one drag gesture.
- The window looks and behaves as it does today. The one stated exception is the Queue's
  pixel scroll (spec delta, "Scrolling follows the cursor").

**Non-Goals:**

- Any look change. `gui-tree-row-roles` lands after this change.
- Any change to the TUI, `mbv-client-core`, `mbv-surfaces`, the Owner, ctrl, or a Service crate.
- Any change to the Settings data model or to the Library data fetches. The projections change
  their output shape, not what they load.

## Decisions

### D1. The level tree and its module layout

Each level is a module under `crates/mbv-gui/src/window/components/`. A level's parts are
`pub(in <level path>)`. Only the level's entry component and its data types are `pub`. A child
level is a submodule of its parent, so it can use the parent's parts, and nothing outside the
parent can.

```
components/
  control.rs   control/     CONTROL BASE: button, icon button, text button, tab, pill,
                            switch, checkbox, pick button, hover action. One press,
                            hover, focus, and disabled look.
  field.rs     field/       FIELD BASE: text input, draft, clear control, commit and
                            cancel keys, focus-while-open.
  popup.rs     popup/       POPUP BASE: anchored card, open and close, outside press,
    menu.rs    popup/menu/    MENU: one menu entry (label, mark, shortcut, submenu)
    dialog.rs  popup/dialog/  DIALOG: frame, title, body text, button row
  bar.rs       bar/         BAR BASE: lead, title, trail slots as data, height, tokens
  settings_row.rs           SETTINGS ROW: label, detail, one control or field
  panel.rs     panel/       PANEL BASE: frame, divider, fold, collapse
  list.rs      list/        LIST BASE
    library.rs list/library/  LIBRARY LIST
      flat.rs  list/library/flat/
      tree.rs  list/library/tree/
    queue.rs   list/queue/
    workspace.rs list/workspace/
  hero.rs      hero/        HERO BASE
  library.rs   library/     LIBRARY composite (BAR + LIST + HERO + BAR)
  toast.rs                  TOAST (one owner today, `toast_stack.rs`; moves as is)
```

Shared leaf parts that already have one owner (`ArtImage`, `TypedText`, `Truncate`, icons,
`PlayMark`/`RowPlay`) stay where they are and become `pub(in crate::window::components)`.

*Why not keep flat files with `pub` parts?* That is the present layout, and it let six list
painters compose the row parts. Visibility makes the rule a compile error.

### D2. Bases take data; levels extend through a sealed trait

A base's props hold data only: no `Element`, no `children`, no closure that returns an
`Element`. A consumer (a composite or the app root) passes frames and intent sinks.

A child level extends its parent through a **sealed** trait that only modules inside the parent
can implement (the sealed-supertrait pattern, `mod private { pub trait Sealed {} }`). For the
list:

```rust
pub trait ListSource: private::Sealed + 'static {
    type Key: Clone + Eq + Hash + Display;   // Display gives the pointer-row marker
    type Extra: RowExtra;                    // () for a Library list
    fn rows(&self) -> &[ListRow<Self::Key, Self::Extra>];
    fn caps(&self) -> ListCaps;              // reorder, active_row, letters, header
    fn sink(&self) -> ListSink<Self::Key>;   // where intents go
}
pub trait RowExtra: private::Sealed { /* paints the level's own cells */ }
```

The implementors are the levels: `LibraryListSource` (`list/library/`), `QueueSource`
(`list/queue/`), and `WorkspaceSource` (`list/workspace/`). A level's own cells (the Queue's
ordinal and ring, the Workspace's column grid and playing bars) paint inside that level's
module. This is allowed because a level is part of the component, not a consumer. The Hero uses
the same pattern for its body.

*Why not one open `Element` slot for the extra cells?* Every earlier partial migration passed
through an `Element` slot.

### D3. LIST BASE owns every list mechanic

LIST BASE owns these mechanics, one implementation each:

- **Viewport.** Native scroll, the virtual window (the `library_virtual_list` algorithm, made the
  base's), the spacers, the measured viewport report, the scroll-offset report, and the
  rows-in-view report that row art and the Queue use. `GeometryRow` becomes the base's row
  geometry, keyed by role, with each level's heights as level constants.
- **Rows.** The row host, its roles (`Root`, `Branch`, `Leaf`, `Heading`, `Field`), the cell
  layout (thumb, number, title, subtitle, facts, trailing text), the play marks through
  `RowPlay`, the selection and pick fill, the marked fill, the active-row fill, and the divider.
- **Hover and menu.** The CSS hover veil, the hover-actions slot (`RowHoverActions` moves in), and
  the context-menu open intent.
- **Input.** Press, double-click (activate), context press, and the keyboard reveal of a cursor
  row (`cursor_reveal.rs` moves in).
- **Notices.** The loading, empty, and error notices (`list_frame_notices.rs`,
  `library_list_notice.rs` move in).
- **Capabilities, off by default.** `reorder` (D9), `active_row` (the Queue's cursor and the
  now-playing row), `letters` (headings and rail, LIBRARY LIST only).

Level look differences are token values at the level root (D10).

### D4. The levels below LIST BASE

| Level | Adds | Consumers today |
| --- | --- | --- |
| LIBRARY LIST | `GuiRowKey` keys, row art scope, filter and sort hookup, letter headings and A-Z rail (`letters` cap), list header BAR, the Library key map | every Library destination, Search |
| FLAT | top-level rows only | Home, Emby (incl. Favourites), Audiobooks, Podcasts, Feeds, Shows New and Upcoming |
| TREE | expand and collapse, Expand all and Collapse all, guides and rails, `Field` row (the rename draft through FIELD BASE), child loads on open | Music, Shows All, Playlists |
| QUEUE | `QueueSlotId` keys, ordinal, progress ring, now-playing and marked fills, `reorder`, `active_row` | Queue panel, folded queue |
| WORKSPACE | column grid, header row, playing bars in the number cell, `Heading` rows for grouped bodies | every Hero list band, the Music artist page |

### D5. HERO BASE

HERO BASE takes one `GuiHeroFrame`: header, detail block, art shape and art, overview, cast,
actions as `Vec<GuiHeroActionFrame>` (painted by CONTROL BASE), the favourite heart as
`favourite: Option<GuiHeroFavouriteFrame>` (a header field: the item key and its state; the
toggle is `GuiHeroAct::SetFavourite(bool)`, which the core resolves to today's `SetFavourite`), backdrop, and a body
`GuiHeroBodyFrame::{None, Workspace(GuiListFrame)}`. The seven Hero components and the
`actions` and `children` props of `LibraryHeroFrame` go. The current `LibraryHeroFrame`
mechanics become HERO BASE's internals. The Shows Hero's extra fact layout and the Music artist
page are data in the frame (fact rows, a grouped Workspace body), not code.

### D6. Neutral frames, minted by the core

The bridge carries these frames in place of the per-destination frames:

- `GuiListId`: an opaque id that the core mints per list instance (destination, view, and
  collection). The window keys its list state by it and never reads its parts.
- `GuiRowKey`: the row's stable id as the core mints it today (the `GeometryRow::id` strings).
- `GuiListFrame { id, shape: Flat | Tree, header: GuiListHeaderFrame, rows: Arc<[GuiListRowFrame]>,
  letters: Option<GuiLettersFrame>, notice: Option<GuiListNoticeFrame> }`.
- `GuiListRowFrame { key, role, depth, cells: GuiRowCellsFrame, play: Option<GuiHomeRowStateFrame>,
  art: Option<GuiArtRef>, expandable: bool, hover_actions: GuiRowActionsFrame }`.
- `GuiListHeaderFrame`: title, Service mark, sort choices, refresh, Expand all and Collapse all,
  and the dropdowns as `GuiChoiceFrame` (the collection dropdown and the Shows view dropdown
  become MENU data).
- `GuiHeroFrame` (D5).

Rows stay behind `Arc` so a repaint copies no section content (`gui-client`, "A repaint copies
no section content"). Section revisions stay as they are.

### D7. Neutral intents, resolved by the core

The window sends acts keyed by identity. The core maps each to today's per-destination
behaviour, so no behaviour moves into the window:

```rust
GuiListIntent { list: GuiListId, act: GuiListAct }
GuiListAct = Press(GuiRowKey) | Activate(GuiRowKey) | Toggle(GuiRowKey, bool)
           | Menu(GuiRowKey) | RowAction(GuiRowKey, GuiRowActionId)     // Play, Enqueue, Shuffle
           | Rename(GuiRowKey, String) | Header(GuiListHeaderAct)         // sort, refresh, expand all
           | Choose(GuiChoiceId, GuiChoiceKey) | RowsInView(Vec<GuiRowKey>)
GuiHeroIntent { list: GuiListId, act: GuiHeroAct }  // action id, body row act
```

`GuiLibraryCommand` and `GuiPlaylistsCommand` reduce to these intents plus the acts that are not
row acts (tab change, Search text, Home). Each projection module gains a `resolve` function that
maps an intent for its lists to the command it runs today. The per-destination `press_*`,
`play_*`, `toggle_*`, and `*_hover_presses` functions in `window/library/` go.

The Library key map (`window/library/keys.rs`, `keys_shared.rs`, and the five `*_keys.rs` files)
becomes one LIBRARY LIST key map. It turns a key over a pointer row into a `GuiListAct`, so no
key file names a destination. The pointer marker's list token is the `GuiListId` (D8), not a
destination enum.

### D8. Dioxus Native is the only input system

Every component takes pointer input through Dioxus handlers on its own nodes. The core's hit
test goes:

- `window/layout/region_set.rs`, every region-set report, `core_thread/pointer.rs`'s hit-map
  resolution, and the GUI's use of `HitMap` and `PointerGestureState` go. `mbv-surfaces` keeps
  them for the TUI.
- `RegionTarget` and its target ids (`bridge/layout.rs`) stay as the vocabulary of a new
  `GuiPointerAct { target: RegionTarget, gesture: Press | DoubleClick | Context | Wheel(f32) }`
  intent. A component's handler sends the target it painted. The core runs the same per-target
  handling that the hit map reaches today. So the player panel's, Status bar's, and dialogs'
  behaviour does not move.
- Popups stack by DOM order. A press on a menu entry reaches the entry, because Blitz hit-tests
  the topmost node. The overlay region ordering goes.
- `pointer_target.rs` stays as the mechanism that names the key target for the keyboard router,
  but `PointerList` loses its destination variants (`Music`, `Home`, `Shows`, `Emby`,
  `Audiobooks`, `Podcasts`, `Feeds`, `Tracklist`, `Workspace`). It becomes one `PointerList` that
  carries a `GuiListId` token (the marker value is the id's string form). `PointerZone` stays.
  The core resolves the id to the destination's meaning, as it does for every intent (D7).
- The core-side pointer code goes with the hit map, in three places the first draft did not name:
  - `leaves.rs` (`QueueLeaf`, `PlaybackLeaf`, `StatusBarLeaf`) keeps the per-target handling and
    the `Msg`s it emits. It stops resolving a point. Each leaf takes the `RegionTarget` that a
    `GuiPointerAct` carries. `row_slot_at`, `row_hover_at`, and `seekbar_region` go. A queue row
    target already carries its `QueueSlotId`. The scrubber reports its seek fraction in the act
    (`Seek(f32)` gesture), measured by the scrubber through the same measured-size report that
    LIST BASE uses, so no core code reads a rectangle.
  - `attach.rs` (the pre-attach campaign's mismatch prompt) drops its `HitMap` and
    `PointerGestureState`. The DIALOG buttons of `attach_screen` send a `GuiPointerAct` whose
    target is `RegionTarget::Attach(Restart | Quit)`, and `mismatch_pointer` maps that target to
    the same `PromptOutcome` it maps today. A press on no target sends nothing.
  - `bridge/layout.rs` loses `hit_map` and `LayoutRegion` reports. `RegionTarget` and the target
    id enums stay.

### D9. One drag gesture

`components/gesture.rs` holds one drag state machine, armed by a press and started by motion
past the existing threshold. Because Blitz has no pointer capture, the app root installs one
`onpointermove`, one `onpointerup`, and one `onpointerleave`. They forward to the armed gesture
through a context signal, so moves reach it anywhere in the window by bubbling. A new press while
armed, or a pointer leave, cancels the gesture. LIST BASE's `reorder` capability paints the ghost
and the insertion line from it and sends `Reorder(key, anchor)` on drop. The QUEUE level turns
that into the move request it sends today. The scrubber (PANEL, player panel) uses the same
gesture for seek. `GuiQueue.drag` and `GuiQueue.hover` leave the frame.

*Why not a capture shim in Blitz?* The rules forbid patching the pinned stack, and bubbling to
the root is enough.

### D10. Look tokens

Each mechanic has one token name, for example `--list-row-fill-selected`,
`--list-row-fill-active`, `--list-row-divider`. A level sets only that token's value at its root.
The Queue's `--ef-*` values and the Library's values become those values. No component names a
colour, size, or radius except through a token. Where two variants paint the same mechanic
differently today, the difference becomes two values of one token, and the look stays the same.

### D11. Interaction state ownership

- Window, generic over `GuiListId`: scroll, hover, open tree nodes, filter text, the rename
  draft. `window/library/state.rs` and its children become one `ListStates` map. The
  per-destination state files go.
- Window, Queue: viewport and drag gesture (spec delta). The window reports the Queue's rows in
  view, and the core uses that report for page keys and the wheel-follow rule.
- Core: the data selection, the Queue's active row, marks, prefix mode, folds, and the open menu,
  as today.

### D12. Inventory: where each current file goes

| Current file(s) in `window/components/` | Target |
| --- | --- |
| `library_virtual_list`, `library_list_column`, `list_row` (+ `flat`, `tree`, `thumb`), `tree_row`, `tree_guide`, `cursor_reveal`, `library_letter_heading`, `library_letter_rail`, `library_list_notice`, `list_frame_notices`, `row_hover_actions` | LIST BASE or LIBRARY LIST internals |
| `library_lettered_list`, `library_home_list`, `library_favourites_list` | FLAT |
| `library_list`, `library_shows_list`, `playlists_panel` | TREE (painters go; Playlists screen becomes a composite) |
| `queue_panel` (+ `row`, `row_cells`, `geometry`), `folded_queue` | QUEUE |
| `library_workspace_table`, `library_tracklist`, `library_artist_tracklist` | WORKSPACE |
| `library_hero_frame`, `library_hero_art`, `library_hero_cast`, `library_hero_detail`, `library_hero_fact_rows`, `library_hero_section`, `library_hero_pills` | HERO BASE internals |
| `library_hero`, `library_home_hero`, `library_shows_hero`, `library_emby_hero`, `library_favourites_hero`, `library_audiobooks_hero`, `library_podcasts_hero`, `library_feeds_hero` | deleted (data in `GuiHeroFrame`) |
| `header_icon_button`, `settings_button`, `settings_switch`, `checkbox_box`, `clear_field_button`, `library_hero_actions`, `prefix_badge`, `settings_state_pill`, `library_sort_control`, `queue_target_control`, `library_top_bar/tabs`, `settings_pick_button`, `track_chips` (chips), `library_hero_heart` (the favourite heart) | CONTROL BASE |
| `library_filter_field`, `library_search_control`, `settings_field`, the Playlists rename input | FIELD BASE |
| `queue_row_menu`, `chooser_menu`, `queue_chooser_menu`, `library_dropdown`, `library_collection_dropdown`, `library_view_dropdown`, `menu_card`, `track_chips` (popover) | POPUP / MENU |
| `dialog_frame`, `confirm_overlay`, `daemon_lost_overlay`, `attach_screen` | POPUP / DIALOG |
| `library_top_bar`, `library_list_header`, `queue_header`, `library_status_bar`, `settings_status_bar`, `queue_footer` | BAR BASE |
| `settings_row`, `settings_rows`, `settings_entry_row`, `settings_card`, `settings_checkbox_grid` | SETTINGS ROW |
| `settings_feeds`, `settings_services`, `settings_sessions`, `settings_keys`, `settings_library`, `settings_sidebar`, `settings_screen` | composites over SETTINGS ROW, CONTROL, FIELD, BAR |
| `panel`, `panel_divider`, `collapsed_rail`, `compact_strip`, `queue_playback_panel` (+ `art_stage`, `scrubber`) | PANEL BASE and the player panel composite |
| `library_panel` | LIBRARY composite |
| `art_image`, `typed_text`, `play_mark`, `library_attr`, `use_focus_while`, `toast_stack` | shared leaf parts, unchanged in role |

`window/library/`: `favourites*`, `audiobooks*`, `emby*`, `feeds*`, `podcasts*`, `shows*`, `home*`, `episode`,
`hero`, `lettered*`, `parts`, `row_memo`, and the per-destination parts of `state*` and `keys*`
go. `list*`, `scroll`, `search`, `status`, and `reset` become generic over `GuiListId`.
`window/sections.rs` replaces its `playlists` section with the neutral Library section of task 1.5
(the Playlists tree is a `GuiListFrame`); the Playlists screen's open flag stays a plain section.

### D13. Tests

Tests follow the contracts, not the files. Each base level owns its contract tests in
`crates/mbv-gui/tests/` (or `src/` tests for private items): LIST BASE (viewport window, roles,
hover, menu intent, notices), each level's additions, HERO BASE, CONTROL, FIELD, POPUP, BAR,
SETTINGS ROW, and the drag gesture. Per-destination painter tests are removed, not ported. The
behaviour they guarded moves to projection tests in `gui_core` (rows and Hero frame per
destination) and intent-resolution tests (one intent maps to today's command). No test claims a
pixel position (scratchpad item, 2026-11-19).

## Risks / Trade-offs

- [The refactor is large and touches almost every GUI file.] → The tasks build the new bases
  beside the old code, move consumers one destination at a time, and delete the old code last.
  A unit can leave the build red when its task records what the next unit expects.
- [A look drifts while code moves.] → Every look value goes through one token (D10), and each
  task that moves a surface compares the painted structure with the old painter's tests before
  it deletes them.
- [The Queue loses whole-row scroll.] → Accepted by the user and stated in the spec delta. The
  cursor row stays fully visible after each keyboard movement.
- [Native wheel steps differ from the Queue's uniform step.] → Blitz scrolls lines at 20 px. The
  Library lists already use it, so the Queue and the Library now agree.
- [A drag misses its release outside the window.] → The root `onpointerleave` cancels the armed
  gesture, as the core's cancel rule does today.
- [Virtualising Playlists and Workspace tables changes their measure path.] → They use the base
  viewport and its measured-height report, which the Library lists already use.
- [The neutral frames lose a destination fact.] → Each projection task lists the facts its old
  frame carried and maps each to a neutral field before the old frame goes.

## Migration Plan

1. PR 1075 (`emby-favourites`) merges first.
2. Build order: neutral frames and intents → projections → bases → levels → composites →
   input-system retirement → deletions → docs.
3. One branch and one PR. `add-gui-pin-window` and `gui-tree-row-roles` stay on hold until it
   merges, and then rebase onto the bases.
4. Rollback is a revert of the PR. No persisted data changes shape.

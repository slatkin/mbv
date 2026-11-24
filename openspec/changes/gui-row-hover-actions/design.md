# Design

## Context

See `proposal.md` for the motivation. The specs are in `specs/gui-client/spec.md`.

Today the GUI has three hover mechanisms, and this change unifies two of them:

- **Library tree parents** use CSS `:hover`. `TreeHint` (`crates/mbv-gui/src/window/components/tree_hint.rs`) is a span under the row, and `.tree-parent:hover .tree-hint` in `assets/app.css` shows it. Blitz sets and clears the hover itself.
- **Workspace tables** (`library_workspace_table.rs`) keep a window `Signal<Option<String>>` that `onpointerenter` and `onpointerleave` set. The signal swaps the number cell for a play control and shows the `WorkspaceAction` buttons (Play next, Add). `pointer_target.rs` records why this mechanism is weak: under a still pointer, a list that scrolls leaves a pointer-enter signal stale, but Blitz re-resolves `:hover` after every layout and scroll.
- **Queue rows** get their hover from the Owner frame (`queue.hover`). The hover Play control is a sibling of the row button at the row's trailing end and reports a hit region (`QueueTargetId::RowPlay`) after the row's `Slot` region; the old grip and `RowActions` button are retired (user request 2026-11-23). `crates/mbv-gui/src/leaves.rs` resolves presses through the hit map, not through Dioxus handlers.

A top-level Library row (`LibraryItemRow`) is one `<button>` with `onclick` (pick), `ondoubleclick` (play or toggle), and `oncontextmenu`. The Music album and track rows (`library_list.rs`) and the Shows season and episode rows (`library_shows_list.rs`) have their own painters with the same three handlers. The Playlists screen rows (`playlists_panel.rs`) have their own painters and their own `Control` set.

The frame already tells the window which rows are collections. `GuiEmbyRowKindFrame` and `GuiHomeRowKindFrame` carry `Collection` or `Media`. The row model also names each kind: `LibraryRow::Artist`/`Album`/`Track`, the Shows tree's series, season, and episode rows, the Shows New and Upcoming flat rows, books, podcast episodes, feed entries, and `Pick::Playlist`/`Pick::Item`.

The right-click path already exists. `GuiLibraryCommand::RowMenu { row }` names a `GuiLibraryMenuRow`. `App::gui_library_menu_entries` builds that row's menu through the shared builders, and `run_gui_library_menu_entry(row, index)` runs one entry. The Emby builder gives a folder Play All, Shuffle, and Add to Queue, and a leaf Play and Add to Queue (`src/app/dispatch/context_menu/menu/emby.rs`).

All Play next admission is GUI-only. `play_lib_item_next` (`src/app/dispatch/queue/playlist_mutation.rs`) has three callers, and all are GUI commands. The queue row menu's Play next move (`ContextAction::PlayNext`, `play_queue_item_next`) comes only from the GUI row menu builder (`src/app/gui_client_state/row_menu.rs`).

## Goals / Non-Goals

**Goals:**

- One hover-controls painter for every Library row surface, with one control order.
- Every control reuses a path that a gesture already runs. Only Shuffle gets a new command.
- Remove all Play next code, not only its UI.

**Non-Goals:**

- The Library Search results dropdown, the Hero's own action buttons (other than the Play next removal), and the Settings screen rows get no hover controls.
- No keyboard access to the hover controls. The keys stay Enter and `a`.
- The TUI does not change.
- ADR 0036 mentions play next as history. It stays as written, because an accepted ADR records the decision as it was made.

## Decisions

### D1. One `RowHoverActions` part, revealed by CSS `:hover`

A new component `crates/mbv-gui/src/window/components/row_hover_actions.rs` paints the controls in this fixed order: Play, Add to queue, Shuffle, Expand/collapse. Each control is an `Option<Callback<()>>` prop, and only the controls with a callback paint. Expand/collapse also takes the node's open state, which picks its glyph. The icons are the existing `Play`, `Plus`, and `Shuffle` icons in `window/icons.rs`, plus the existing `ChevronDown` (node closed, a press expands) and `ChevronUp` (node open, a press collapses). Each button calls `event.stop_propagation()` in its `onclick` and `ondoubleclick`, so a press never reaches the row's pick or double-click.

The library surfaces show the part through CSS, with no window hover state: a `.row-hover-host` element carries the row, `.row-hover-actions` is `display: none` at rest, and `.row-hover-host:hover .row-hover-actions` is `display: flex`. `.row-hover-host:hover .row-hover-trail` hides the trailing text. The reveal has no rest delay. The tree hint's 500 ms delay was for a tooltip. A control must answer at once.

*Alternatives:* the Workspace table's `onpointerenter` signal goes stale in a list that scrolls under a still pointer (see Context). An Owner-held hover, as the queue has, would send a Client core round trip on every pointer move over the Library. The ADR 0036 rule keeps Library interaction state in the window, so the window owns this hover.

### D2. Controls are siblings of the row button, overlaid on the trailing end

A control must not nest inside the row's `<button>`. That is the queue's rule, and it keeps the button semantics valid. Each row painter wraps its row in a `div.row-hover-host` with `position: relative`. The div holds the unchanged row button and, as a sibling, the `RowHoverActions` part, positioned absolute at the trailing end and vertically centered. The part paints the row's own fill behind it, so it covers the trailing text and the end of a long title cleanly. `Truncate` measures in Rust, so a CSS reveal cannot shorten the title, and the cover is the accepted look. The row button's own fill moves to a `--row-fill` custom property on the host, and both the button and the part read it.

A tree parent row's `tree-parent` class and `tree-trailing` marker go with `TreeHint`. The trailing text carries `row-hover-trail` on every row that paints the part.

*Alternative:* swapping the trailing cell in flow would change the title width on hover and make the row jump.

### D3. Which controls a row gets is the window's choice, from facts it already holds

The row painter picks the callbacks from the row's kind (see Context). No new frame field is needed:

| Row | Play | Add | Shuffle | Expand |
| --- | --- | --- | --- | --- |
| Music artist (with an item id), album | yes | yes | yes | yes |
| Music artist with no item id | | | | yes |
| Music track | yes | yes | | |
| Shows series, season (All view) | yes | yes | yes | yes |
| Shows tree episode, Shows New row | yes | yes | | |
| Shows Upcoming row | | | | |
| Emby / Home row, `Collection` | yes | yes | yes | |
| Emby / Home row, `Media` | yes | yes | | |
| Audiobooks book, Podcasts episode, Feeds entry | yes | yes | | |
| Playlists screen playlist | yes | yes | yes | yes |
| Playlists screen item | yes | yes | | |
| Hero tracklist, Shows episodes, Feeds entries tables | yes | yes | | |
| Audiobooks chapters table | yes | | | |

### D4. Each control runs an existing path, and Shuffle runs the right-click entry

- **Play** on a Library list row calls the same function that the destination's key adapter's `PanelKeys::play_item` calls. This is Enter's play, which the right-click Play already matches. On a Shows series or season it plays the item, even though Enter toggles there. On a Workspace or tracklist row it calls the table's existing `on_play`, which the double-click and the old number-cell control already call.
- **Add to queue** calls the same function as `PanelKeys::queue_item` (the `a` key), or the table row's existing Add action.
- **Expand/collapse** calls the row's existing toggle (`on_toggle`, `toggle_library_node`, and the Shows and Playlists toggles).
- **Shuffle** on a Library row sends a new `GuiLibraryCommand::Shuffle { row: GuiLibraryMenuRow }`, mirrored in `crates/mbv-gui/src/bridge/library.rs`. `App::apply_gui_library_command` resolves the row with `gui_library_menu_target`, builds its entries with `gui_library_menu_entries`, finds the first entry whose action is a shuffle (a new `ContextAction::is_shuffle` in `crates/mbv-ui-model/src/context_menu.rs`, an exhaustive `match` with no wildcard), and runs it through `run_gui_library_menu_entry(row, index)`. A row whose menu has no Shuffle entry logs through `log_ignored_library_command` and does nothing.
- **Playlists screen** controls call the screen's existing control dispatch for `Control::Play`, `Control::Add`, and `Control::Shuffle`, the same dispatch the screen's hero and menu use.

*Alternative:* resolving Play and Add through the menu entries as well would give the same effect with more indirection. The key paths are already per-destination and already tested.

### D5. Workspace table: the hover signal and the number swap go

`LibraryWorkspaceTable` loses its `hovered` signal, `hover_styles`, the play control in `paint_number_cell`, and the `WorkspaceAction`/`WorkspaceActionIcon` types. The number cell always paints the number (or the playing glyph, as today). The actions cell and the duration cell merge into one trailing cell. It holds the duration (`row-hover-trail`) and the `RowHoverActions` part. The `li` becomes the `row-hover-host`. The table takes `on_play` (it has it already), an optional `on_queue: Option<Callback<String>>`, and paints Play, plus Add when `on_queue` is set. The tracklist, the Shows episodes, and the Feeds entries tables pass `on_queue`. The Audiobooks chapters table does not. The hover-swap regression test in `library_workspace_table/tests.rs` goes with the swap.

### D6. Queue: a `RowPlay` hit target beside `RowActions`

`QueueTargetId` gains `RowPlay(u64)` (`crates/mbv-gui/src/bridge/layout.rs`). The queue panel makes a third hover mark for it, pushed after the row's `Slot` region with the grip and actions marks. `queue_row_card` paints a play button as a sibling of the row button, just left of the actions button, and `HOVERED_TRAILING_PAD_PX` grows to fit both buttons. `RowCells` paints no duration under `RowChrome::Hover`. In `leaves.rs`, a click on `RowPlay(raw)` selects the slot and returns `ShellRequest::QueueRowActivate`, which is the double-click's request. `double_click` already plays only on a `Slot` region, so a double-click on the control does not play twice. Every `match` on `QueueTargetId` gets an explicit `RowPlay` arm.

The queue cannot use D1's CSS reveal: its hover and its hit regions are the Owner's frame and the core's hit map, and the grip and drag depend on them. It paints the same icons and button look as `RowHoverActions`, so the controls look the same.

### D7. Play next removal

These go, with their tests:

- **Window:** the Play next button in `LibraryHeroActions` (`on_play_next`) and every caller that passes it (`library_hero.rs`, `library_hero_summary.rs`, `library_emby_hero.rs`, `library_shows_hero.rs`, and others found by the compiler). The `n` handling in `keys.rs`, `keys_shared.rs` (`play_next_item`, the `n` branches of `route_remaining_key`, `route_list_key`, `route_hero_key`, `route_item_key`), `emby_keys.rs`, `shows_keys.rs`, `feeds_keys.rs`, and `audiobooks_keys.rs`. `play_next_shows_episode` in `shows_workspace.rs`, the `play_next` hooks in `lettered.rs` and `lettered/adapters.rs` (with `EmbyList::play_next`), and the `HomePlayNext` sender in `home.rs`.
- **Bridge and core:** `GuiLibraryCommand::{PlayNext, HomePlayNext, ShowsPlayNext, EmbyPlayNext}` in both enums and their conversion arms, `play_next_gui_emby_item`, `play_next_gui_shows_item`, `play_lib_item_next`, and `play_next_anchor`. `do_enqueue_folder_before` stays, because `src/app/dispatch/actions.rs` still calls it with `None`.
- **Queue row menu:** the Play next entry in `open_gui_row_menu`, `ContextAction::PlayNext(usize)` and its executor arm, `play_queue_item_next`, `play_next_position`, and `position_after` (`src/app/dispatch/queue.rs`; its only caller is `play_next_position`).

With `n` gone from the Library keys, `n` over the Library is a key that the window content does not take, so it goes to the keyboard router, as the existing rule says. It is not consumed.

### D8. Docs

`docs/architecture/gui-client.md` updates its row menu entry list (line ~828), the `LibraryHeroActions` description (~2031), and the `TreeHint` paragraph (~2076). The paragraph is replaced with a short `RowHoverActions` paragraph that records D1 and D2. The Direct remote control entry in `CONTEXT.md` drops "play next" from its list of queue operations. The `TreeHint` comment block in `assets/app.css` goes with the rule. `assets/app.generated.css` is regenerated by the existing stylesheet build (the `dioxus-native-blitz` skill's regenerate step).

## Risks / Trade-offs

- [Blitz hit-tests a `display: none` → `flex` sibling that overlaps the row button] → The overlay is a later sibling with `z-index`, the same arrangement the Library dropdowns and popovers use over list content. They resolve to the top element today.
- [The overlay covers the end of a long title on hover] → Accepted (D2). The full title is visible at rest and in the Hero.
- [A collection row whose right-click menu has no Shuffle entry shows a Shuffle control that does nothing] → The Emby folder rule decides both, so they agree for every Emby collection. A mismatch logs through `log_ignored_library_command` and changes nothing.
- [Users of `n` lose the key] → The user asked for this. `n` falls through to the router.

## Migration Plan

This is a GUI-only change with no persisted state. It ships with the GUI build. Rollback is a revert.

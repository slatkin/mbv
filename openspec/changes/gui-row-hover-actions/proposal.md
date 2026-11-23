# Proposal

## Why

The GUI Library rows can play and queue an item today only by double-click, a key, or the right-click menu. The hover controls that do exist differ by surface: the Workspace tables put play in the number cell and add a Play next button, the tree rows show a tooltip, and the queue row shows only a grip and a menu button. The user wants one hover affordance on every row that runs the actions the row's right-click menu already has.

Play next was added to the GUI without a request from the user, and the user does not use it. The queue order is what drag and drop sets. This change removes all Play next behavior from the GUI.

## What Changes

- Every GUI Library row (every top-level list row, every tree child row, the Hero tracklist rows, the Workspace table rows, and the Playlists screen rows) shows hover controls at its trailing end. The controls take the place of the row's trailing text (date, year, duration, count) while they show.
- A collection row (Emby folder or boxset, Music artist or album, Shows series or season, Home collection row, saved playlist) gets Play, Add to queue, and Shuffle. A media row (track, episode, movie, book, podcast episode, feed entry, playlist item) gets Play and Add to queue. An Audiobooks chapter row gets Play only. A Shows Upcoming row gets none.
- A GUI tree parent row also gets an Expand/collapse control. It replaces the double-click hint tooltip, which is removed. The double-click still opens and closes the node.
- Each control does what the row's existing gesture does: Play does what Enter does on the row, Add to queue does what `a` does, Shuffle runs the Shuffle entry of the row's right-click menu, and Expand/collapse does what the double-click does.
- The Hero tracklist and Workspace table rows keep their number on hover. Their play control moves from the number cell to the trailing controls.
- A queue row shows a Play control on hover in its right column, in place of the item's duration. The control plays that slot, as a double-click on the row does.
- **BREAKING (GUI only):** All Play next behavior is removed: the Hero's Play next control, the `n` Library key, the hover Play next buttons, the GUI Play next commands and their admission path, and the queue row menu's Play next move. The TUI has none of these and does not change. The "Always play next" setting is a different feature (series autoplay) and stays.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: adds the row hover controls requirement; removes the tree-parent double-click hint requirement; modifies the queue row requirement (hover Play in the right column, no Play next menu entry), the Library keys requirement (no `n`), the Hero actions requirement (no Play next, tracklist hover controls), and the Hero frame requirement (no Play next action).

## Impact

- `crates/mbv-gui`: a new shared hover-controls part; `LibraryItemRow`, the Music album and track rows, the Shows season and episode rows, the Workspace table, the Hero tracklist, the Playlists screen rows, and the queue row card paint it. `TreeHint` and its CSS rule go. The Workspace table's hover signal goes. A new `QueueTargetId::RowPlay` hit target. The Library key adapters lose `play_next_item` and `n`. The Hero action buttons lose Play next.
- `src/app`: a new `GuiLibraryCommand::Shuffle { row }`; the four GUI Play next commands, `play_lib_item_next`, `play_next_anchor`, the queue row menu's Play next entry, `ContextAction::PlayNext`, and `play_queue_item_next` go.
- `crates/mbv-ui-model`: `ContextAction::PlayNext` goes, and the context actions gain a shuffle-family predicate.
- No dependency changes. No TUI behavior changes.

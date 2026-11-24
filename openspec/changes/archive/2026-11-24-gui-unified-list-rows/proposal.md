# Proposal

## Why

The GUI has two list types, flat and tree, but no shared row look. Each list paints its own rows. The flat lists share `LibraryItemRow`, but each caller passes its own title colour, fill, progress, and played mark. Every nested tree row (Music album and track, Shows season and episode, Playlists playlist and item) builds its own `button` with its own style string, height, inset, fill, radius, weight, and tone. So two rows at the same level look different from list to list, and a look fix reaches only the list that someone edits by hand.

This change centralises row painting. Two row parts own every Library and Playlists row: a flat row and a tree row. A list gives a row its data, its state, and its handlers. It gives no colour, weight, fill, radius, height, inset, or style string, and it builds no row `button`. Every row at one level looks the same in every list. The Music list is the model for that look.

This is not a feature change. It adds no play state, no mark, and no Service data. Where a list already carries a play state, the row uses the rule the Shows list uses today: a played title paints in the muted tone.

## What Changes

- Two row parts replace every bespoke Library and Playlists row painter: `FlatRow` and `TreeRow`. Only these parts build a list row's `button`, style, height, inset, fill, tones, weights, guides, hint, and `Truncate` measure. Neither part takes a look value.
- Three row levels, each with one look taken from the Music list:
  - **Top-level row** (the Music artist row): every flat-list row, and the top-level row of every tree list (Music artist, Shows series, Playlists playlist). It paints through `FlatRow`.
  - **Nested parent row** (the Music album row): Music album and Shows season. It paints through `TreeRow`.
  - **Leaf row** (the Music track row): Music track, Shows episode, and Playlists item. It paints through `TreeRow`.
- One play-state rule (the Shows rule today): a row whose item carries a play state paints its title in the muted tone when the item is played. A row with a percent keeps the thumb's progress run. No other play mark paints.
- One hook scheme: every row part paints `data-library-<attr>-<part>` through `library_attr`.
- The bespoke paths go: `LibraryItemRow` and its look props (`title_color`, `background`, `progress`, `played_mark`), `RowPlayedMark` and the Recently added mark of commit `ad064a00e`, `paint_item_row`, `ItemRowPaint`, `name_color`, `row_progress`, `recent_played_mark`, `row_name_color`, `row_background`, `row_inset`, `row_style`, `item_row_style`, `row_fill`, `PLAYLIST_ROW_HEIGHT_PX`, `PLAYLIST_LABEL_X`, `PLAYLIST_GUIDE_X`, and the hand-built row `button`s in `library_list.rs`, `library_shows_list.rs`, and `playlists_panel.rs`.

Visible results: rows that differ from the Music look today take the Music look. Shows seasons lose the bold selected title and keep a radius only while selected. Shows episodes get the Music track number cell. Playlists rows lose their `canvas-selected` fill, 6 px radius, 36 px height, and own indent: a playlist paints as a top-level row without a thumb, and an item paints as a leaf row. Recently added rows lose the dot, check, dimmed thumb, and their own tones and weights; a played Recently added title paints muted.

Out of scope: any new play state or mark; projection and bridge frame changes; the Queue rows, the Workspace tables, and the Hero tracklist; the TUI.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: "Library lists and Workspace tables paint through shared parts" names the two row parts and the three row levels, takes the Music list as the look of each level, and removes the carve-out that lets nested rows keep their own painter.

## Impact

- `crates/mbv-gui/src/window/components/`: new `list_row.rs` with `list_row/flat.rs`, `list_row/tree.rs`, `list_row/thumb.rs`, and `list_row/tests.rs`. `library_item_row.rs` is deleted. Callers: `library_home_list.rs`, `library_list.rs`, `library_shows_list.rs`, `playlists_panel.rs`, `window/library/lettered/adapters.rs`. `tree_guide.rs` takes `ROW_DIVIDER_PX`, `ROW_PADDING_PX`, and `THUMB_WIDTH_PX` from `list_row` and loses the Playlists guide constants.
- The row heights (`LIBRARY_ROW_HEIGHT`, `NODE_ROW_HEIGHT`, `TRACK_ROW_HEIGHT`) stay in `window/library/list.rs`, where the list geometry reads them. The parts read the same constants.
- The Playlists hooks move from `data-playlists-row` to `data-library-playlists-row` (and the other part hooks of the scheme).
- No Client core, projection, bridge frame, dependency, or TUI change.
- Commit `ad064a00e` is undone in effect: its mark and treatment go.
- The open change `gui-row-hover-actions` (group 5) edits the same painters. This change lands first, and that group then paints its hover controls into the two parts. The open change `gui-playlists-in-queue-column` edits `playlists_panel.rs` and its hooks. The change that lands second moves its row edits onto `TreeRow` and the new hook names.

# Proposal

## Why

Every GUI Library list paints its own rows. The flat lists share one row part, but each caller passes its own colour, weight, and progress. Every tree child row (Music album and track, Shows season and episode, Playlists item) builds its own `button` with its own style string. So a row look reaches only the lists that someone wires by hand. The played treatment shows this: Home's Recently added got it, and Podcasts, Feeds, Emby, Audiobooks, Shows, Music, and Playlists did not. Home's Recently added rows also hold a snapshot of the played state from fetch time, so a row played after the fetch still paints as unplayed.

The GUI has two list row types, flat and tree. Each type must paint the same in every list, and no list may restyle it.

## What Changes

- Two row primitives replace every bespoke Library row painter: a **flat row** (the top-level row of every list, flat or tree, with its art thumb) and a **tree row** (a nested child row of a tree list). Both take data and handlers only. Neither takes a colour, weight, opacity, fill, or style value, so a list cannot restate the look.
- One **row state** feeds both primitives: the item's play state (not tracked, unplayed, partly played with its percent, played) and whether the row is selected. The primitives derive all of the look from it: the title and subtitle tones, the title weight, the played mark before the trailing text, the dimmed thumb of a played row, the progress run of a partly played row, and the selection fill.
- Every list moves onto the primitives: Home, Podcasts, Feeds, Emby, Audiobooks, and Shows New and Upcoming (flat); Music artist, album, and track rows, Shows series, season, and episode rows, and the Playlists screen's playlist and item rows (tree).
- The Client core projections carry the play state for every list row that names a playable item. The Emby, Audiobooks, Shows flat, and Playlists item rows gain it; they paint without it today.
- Home's Recently added rows follow the live played state: a played or progress change to an item updates its stored Recently added copy.
- The bespoke paths go: `title_color`, `background`, `progress`, and `played_mark` on `LibraryItemRow`, `name_color`, `row_name_color`, `episode_played`, and the hand-written row `button`s in the Music, Shows, and Playlists painters.
- One component test paints every list from rows in the same states and asserts that each list paints the same marks.

Out of scope: the Queue rows, the Workspace tables, and the Hero tracklist (a column table, a third shape); the TUI.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the shared-parts requirement now names two row types, flat and tree, that every Library list and the Playlists screen paint through, with no carve-out for nested rows; a new requirement says every list row shows its item's play state in the same way.

## Impact

- `crates/mbv-gui/src/window/components/`: a new `list_row.rs` with the two primitives and the row state look; `library_item_row.rs` is replaced by it; `library_home_list.rs`, `library_list.rs`, `library_shows_list.rs`, `playlists_panel.rs`, and `window/library/lettered/adapters.rs` paint through it.
- `crates/mbv-gui/src/bridge/frame/`: `GuiEmbyRowFrame`, `GuiAudiobooksRowFrame`, `GuiShowsRowFrame`, `GuiMusicTrackFrame`, and `GuiPlaylistItem` gain a state field.
- `src/app/gui_library/` and `src/app/gui_playlists.rs`: the projections fill the new state; `src/app/state/app_struct/home_recently_added.rs` gains the update of stored Recently added items.
- Commit `ad064a00e` (the Recently added-only played mark) is superseded: its `played_mark` prop and `recent_played_mark` go.
- The open change `gui-row-hover-actions` edits the same row painters (its group 5). This change lands first; that change's group 5 then paints its hover controls into the two primitives.
- No dependency changes. No TUI change.

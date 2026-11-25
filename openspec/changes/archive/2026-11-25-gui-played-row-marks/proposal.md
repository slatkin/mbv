# Proposal

## Why

In the GUI Client, a played row and an unplayed row look almost the same in every list. The title colour and weight are the same, the thumb is the same, and no mark shows the state. Only the queue is different: it strikes through a played title, so the lists also disagree with each other. Users cannot scan a list for what they have not watched or heard yet.

## What Changes

- Every GUI list row that carries a played state paints one shared played/unplayed treatment:
  - An unplayed row (which includes a partly played row) paints a strong title, its normal secondary text, its thumb at full strength, and an unplayed mark (a small filled dot).
  - A played row paints a dim title and dim secondary and trailing text, a dimmed thumb, and a played mark (a check) in the same place.
  - A partly played row keeps its present progress mark (the thumb's resume run, the Workspace row's progress run, or the queue's progress ring) unchanged.
- The mark sits before the row's trailing text (date, duration, or time left). A row with no trailing text shows the mark at its right edge. Every row of a list that carries a play state keeps the same width for the mark, so the trailing text stays aligned and no row moves when its state changes.
- The mark carries an accessible name: "Unplayed" or "Played".
- **BREAKING (look)**: the queue no longer strikes through a played title. It uses the shared treatment.
- The now-playing queue row and the Workspace row that plays at the playhead keep their own look. Their state takes priority, and they show no played or unplayed mark.
- The change is styling only. It uses the play state that each row frame carries now. It adds no frame field, no Service call, no setting, and no new row component.

### Lists in scope (the rows carry a play state)

- Home: Continue watching and Recently added (flat row, `GuiHomeItemRowFrame::state`).
- Podcasts episodes and Feeds entries (flat row, `GuiEntryRowFrame::state`).
- Shows All view: episode leaf rows (tree row, the carried `GuiShowsEpisodeRowFrame::state`).
- Hero Workspace tables: Shows episodes and Feeds entries (`WorkspaceRow::state`).
- The Queue panel (`GuiQueueRow::Item::state`).

### Lists out of scope (no play state on the row frame, left as they are)

- Emby library rows (movies, series, collections): `GuiEmbyRowFrame` has no state.
- Shows Latest and Upcoming flat rows (`GuiShowsRowFrame`), Shows series and season rows.
- Music artist, album, and track rows, and the Music tracklist Workspace.
- Audiobooks book rows and the chapter Workspace.
- Playlists playlist and item rows.
- Library search results (`GuiSearchRowFrame`).
- The queue's drag ghost: it is a drag preview, not a list row.

### Decisions made while planning

- The change covers the GUI Client only. The TUI cannot paint a sized dot, opacity, or a font weight scale, and the request is stated in CSS terms.
- The thumb dims by opacity only. Blitz parses `filter: grayscale()`, but the `vello-hybrid` renderer the GUI uses drops colour-matrix filters, so a grayscale filter paints nothing.
- The mark hides with the trailing text while the row's hover controls show, and the hovered queue row drops it with its duration.
- The queue's right column (96 px) and the Workspace table's trailing column (88 px) keep their widths: the mark's slot and its gap fit beside the longest duration and the progress ring.
- The queue's now-playing row has no play icon today; it keeps its fill and its title look, as now.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the requirement "The queue shows a row menu and drag reorder" changes: a played row is no longer struck through, and the right column carries the row's played or unplayed mark. The requirement "Library lists and Workspace tables paint through shared parts" changes: a row that carries a play state shows the played or unplayed mark and the played row's dim look, in place of "no other played or unplayed mark".

## Impact

- `crates/mbv-gui/src/window/components/list_row.rs` and a new child module for the shared play look and mark.
- `crates/mbv-gui/src/window/components/list_row/flat.rs`, `tree.rs`, `thumb.rs`.
- `crates/mbv-gui/src/window/components/library_workspace_table.rs`.
- `crates/mbv-gui/src/window/components/queue_panel/row_cells.rs`.
- The window tests beside those modules. No core, bridge, ctrl, or Service change. No new dependency.

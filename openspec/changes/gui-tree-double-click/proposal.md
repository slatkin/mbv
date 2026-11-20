# Proposal

## Why

The GUI tree lists copy the TUI: a chevron on each parent row and a single
press that opens or closes the row. A GUI can do better. The chevron takes
space and adds noise, and a single press does two things at once (pick and
open or close). Today the gestures are also different between screens: a
double-click on a playlist row plays it, but a double-click on an artist,
album, series, or season row does nothing. One gesture must do the same thing
on every screen.

## What Changes

- The GUI tree lists are the Music list (artist, album, track), the Shows list
  in its All view (series, season, episode), and the Playlists screen
  (playlist, item). A **parent row** is a row that opens to child rows in the
  same list: an artist, an album, a series, a season that has episodes, and a
  playlist.
- **No chevrons.** No tree row paints a chevron or another open-or-closed
  glyph. Nested rows keep their indent.
- **A single press on a parent row picks it and does not open or close it.**
  The Hero (on the Playlists screen, the control set) shows the row, as now.
  **BREAKING** (spec): "The Music list is the canvas tree" said that a press
  opens or closes an artist or album row.
- **A double-click on a parent row opens or closes it**, on every screen. The
  first press of the double-click picks the row, so a double-click picks and
  opens or closes.
- **A double-click on a playlist row no longer plays the playlist.** The
  playlist's Play control and the Play entry of its right-click menu still play
  it. **BREAKING** (behavior): a user who double-clicked to play a playlist
  must now use Play.
- A double-click on a leaf row (track, episode, playlist item) plays it, as
  now.
- **Hover hint.** While the pointer is on a parent row, the row shows a hint at
  its right end: "Double-click to expand" when the row is closed, "Double-click
  to collapse" when it is open. The hint has no other text. While it shows, it
  takes the place of the row's trailing text (for example the album year).
- Under the Library filter, a node that the filter opens still closes on a
  double-click, as it closes on a press today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: "The Music list is the canvas tree" — a press on an artist or
  album row picks it, and a double-click opens or closes it. A new requirement,
  "GUI tree rows open and close on double-click", sets the one rule for every
  GUI tree list (Music, Shows, Playlists): no open-or-closed glyph, press picks,
  double-click opens or closes, and the hover hint.

## Impact

- `crates/mbv-gui/src/window/components/library_list.rs`: artist and album
  rows lose the chevron, take a double-click, and show the hint.
- `crates/mbv-gui/src/window/components/library_shows_list.rs`: series and
  season rows lose `paint_chevron`, take a double-click, and show the hint.
- `crates/mbv-gui/src/window/components/playlists_panel.rs`: playlist rows lose
  `ChevronDown` and `ChevronRight`; the double-click opens or closes in place of
  Play; the rows show the hint.
- `crates/mbv-gui/src/window/library/state/rows.rs` and
  `crates/mbv-gui/src/window/library/shows.rs`: the press splits into a pick
  (single press) and a toggle (double-click).
- `crates/mbv-gui/assets/app.css`: the hover rule that shows the hint and hides
  the trailing text.
- The chevron icons stay in `icons.rs`: the top bar, the folded queue, the
  dropdowns, and the Settings controls still use them.
- Tests in `library_list/tests.rs`, `library_shows_list/tests.rs`, and
  `playlists_panel/tests.rs`.
- The `gui-delineated-rows` change edits the same artist and series rows. The
  change that merges second rebases on the first. Neither change depends on the
  other.
- The TUI does not change. The TUI tree already opens and closes on
  double-click (`grouped-music-tree-browser`). No new dependency.

## Purpose

Renders the selected album's detail (art, metadata, track list) in the hero panel at the top of the music library's content area, and packs the album list into two columns with artist-group headers.

## ADDED Requirements

### Requirement: Album detail renders in the hero panel

When a music library with levels is at its album-browsing level, the selected album's expanded detail (album art, metadata, track list) SHALL render in the hero panel pinned to the top of the content area — the same hero position used by movies and series in other libraries. The album list below the hero SHALL NOT contain inline expansion for the selected album.

#### Scenario: Selected album detail appears in the hero

- **WHEN** the user selects an album in a music library with levels
- **THEN** the hero panel shows that album's art, metadata, and track list, and the album's row in the list below is compact (no inline expansion)

#### Scenario: Cursor moves to a different album

- **WHEN** the user moves the cursor to a different album
- **THEN** the hero updates to show the newly selected album's detail, and the previously selected album's row returns to compact display

#### Scenario: No album selected (loading or empty)

- **WHEN** the album list is loading or empty
- **THEN** the hero panel shows a placeholder or is suppressed, matching the existing hero behavior for other library types

### Requirement: Track browsing in the hero

Track-level navigation (entering track selection, moving between tracks, playing a track) SHALL operate within the hero panel's track list. The track list in the hero SHALL support the same interactions as the current inline track expansion: cursor movement, playback initiation, and exit back to album-level browsing.

#### Scenario: Enter track selection

- **WHEN** the user presses Enter on the selected album (which is already showing in the hero)
- **THEN** the hero's track list becomes navigable and the track cursor activates

#### Scenario: Play a track from the hero

- **WHEN** the user presses Enter on a track in the hero's track list
- **THEN** playback starts from that track, same as the current inline track interaction

#### Scenario: Exit track selection

- **WHEN** the user exits track selection (e.g. Escape or navigating away)
- **THEN** the hero returns to album-level display and the album list regains cursor focus

### Requirement: Two-column album list with artist-group headers

The album list below the hero SHALL pack album rows into two columns when the list area is wide enough, using the same column-count threshold as other libraries. Artist-group headers SHALL span the full width of the list, and each artist group SHALL pack its albums independently (a row never mixes albums from two groups).

#### Scenario: Wide terminal shows two-column album list

- **WHEN** the list area width meets the two-column threshold
- **THEN** albums pack two per row within each artist group, with artist headers spanning full width

#### Scenario: Narrow terminal shows single-column album list

- **WHEN** the list area width is below the two-column threshold
- **THEN** albums render one per row, matching the current single-column behavior

#### Scenario: Artist header spans both columns

- **WHEN** an artist group boundary occurs in the album list
- **THEN** the artist header row spans the full list width and the next group's albums start on a fresh row

### Requirement: Music-group pills remain above the hero

The genre/mood pill selector (music-group pills) SHALL continue to render above the library content area, in its current position. The hero panel SHALL render below the pills, followed by the album list.

#### Scenario: Pills, hero, and list stack correctly

- **WHEN** a music library with levels and group pills is displayed
- **THEN** the vertical order is: group pills → hero → album list

## MODIFIED Requirements

### Requirement: Wide Workspaces are one header plus one Selector row plus one list box

A destination whose item has constituent items (TV seasons and episodes, Music tracks, Audiobookshelf
chapters) SHALL present them in a Workspace below the overview: an
optional one-row header, an optional Selector row (season pills)
followed by one Main content box
holding the canonical list. The Workspace box SHALL render the accent-soft surface while its list holds
focus and the backdrop surface otherwise. Its selected row SHALL paint the selected-row bar like every
other canonical list row. A Hero pane with a Workspace is focusable; a Hero pane without one is read-only and always
renders the resting surface. No destination SHALL render a second Workspace box.

Grouped Music's Workspace SHALL carry the header `Tracks` (user direction 2026-09-15): the title in the
foam metadata role, then the same `▁` separator line and one blank row the Movie hero's overview box
puts under its overview text, then the track rows — all inside the Workspace box, whose surface and
bottom padding are unchanged. The header SHALL be omitted when the box has no room to keep a list row
under it.

Audiobookshelf podcast episodes are not constituent items: the podcast tab lists them as the panel's own
rows, and the selected episode's hero has no Workspace.

#### Scenario: Track list takes focus
- **WHEN** track selection becomes active in Wide grouped Music
- **THEN** the track Workspace box renders the accent-soft surface

#### Scenario: Grouped Music's track Workspace header
- **WHEN** a Music album's tracks render in the Wide Workspace
- **THEN** a `Tracks` header row paints above them in the foam metadata role
- **AND** the `▁` separator line and one blank row paint between the header and the first track row

#### Scenario: Episode list takes focus
- **WHEN** episode selection becomes active in Wide TV shows
- **THEN** the episode Workspace box renders the accent-soft surface, identically to Music

#### Scenario: Audiobookshelf chapters render in the Workspace
- **WHEN** an Audiobookshelf book is selected at Wide geometry
- **THEN** its chapters render in the one Workspace box with the selected-row bar
  used by TV and Music

#### Scenario: Read-only hero
- **WHEN** Home, Movies, home videos, Feeds or Audiobookshelf podcasts render at Wide geometry
- **THEN** the Hero pane has no Workspace and never renders the focused surface

### Requirement: Narrow workspace lists open only through the Library Hero overlay

The Narrow constituent-list selection modal is removed. In every non-Wide Library panel, constituent items SHALL be reachable through the selected parent's Library Hero overlay, whose Workspace uses the same canonical list and provider-owned content as Wide Hero. Audiobookshelf podcast episodes are not constituent items: the podcast tab lists them directly as its own rows, and Enter plays the selected episode.

#### Scenario: Narrow Audiobookshelf book

- **WHEN** an Audiobookshelf book is selected where the Wide Hero arrangement does not fit
- **THEN** no chapter rows render inside the browser list
- **AND** Enter opens the Library Hero overlay with its chapter Workspace focused

#### Scenario: Narrow Audiobookshelf podcast

- **WHEN** an Audiobookshelf podcast episode is selected where the Wide Hero arrangement does not fit
- **THEN** the episode remains an ordinary fixed-height media row in the browser list
- **AND** no filter pills or episode rows render inside a detail block or overlay
- **AND** Enter plays the episode

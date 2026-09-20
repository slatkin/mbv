## Purpose

Covers the library-panel surface behaviour that landed with the 2026-09-20 tweak batch (user-approved as necessary): the library column body — and the Selector row's spacer row inside it — follows the library panel's own focus bit in every geometry, instead of resting at a fixed backdrop in every focus state.

## MODIFIED Requirements

### Requirement: The non-Wide library panel is the Wide browser pane without a Hero

In non-Wide geometry the Library panel SHALL compose the Wide browser pane's own rows — the Selector
row, the optional List controls row, and the list box — through the same browser-pane composition and
the same surface identities the Wide list pane uses, with the Hero pane absent. The non-Wide list box
SHALL be filled with the `LibraryPanel` pair and SHALL stripe with the `MainContentBox` pair, exactly
as the Wide Browser-pane list does; it SHALL NOT carry a body fill or a scrollbar-column fill of its
own. The non-Wide library column body — the panel placement, the Selector row's spacer row, the status
band's padding rows and the list's scrollbar column — SHALL resolve the `LibraryColumn` surface
against the library panel's own focus bit in every geometry: while the panel rests, the column body
paints the app backdrop (`#2d353b`); while the library panel holds focus, it paints the column
level's focused fill. The panel's focus bit SHALL be the only focus input: a Workspace focused in
Wide geometry SHALL NOT subtract or add the column body's focus after a transition to non-Wide
geometry, so a panel-focused rail paints one consistent focus response in both geometries. The status
row inside the status band SHALL remain the status bar's own surface. No non-Wide-specific library
surface identity SHALL exist. The non-Wide browser rail's focus input SHALL be the library panel's
own focus bit alone: a Workspace focused in Wide geometry SHALL NOT subtract the rail's focused
surface after a transition to non-Wide geometry, so a panel-focused non-Wide rail always paints one
visible focus indicator. A non-Wide list slot with no rows to spare SHALL keep its single row rather
than collapsing to an empty rect, so a very short panel still paints one row. The Wide library column
paints the same `LibraryColumn` pair against the same panel focus bit — no geometry keeps a separate
fixed-column arm.

#### Scenario: A focused non-Wide library keeps the column backdrop

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused
- **THEN** the panel placement, the Selector row's spacer row, the status band's padding rows and the
  list's scrollbar column all carry the column level's focused fill
- **AND** the status row keeps the status bar's own surface

#### Scenario: A resting non-Wide library keeps the backdrop

- **WHEN** the library panel renders in non-Wide geometry without the library panel focused
- **THEN** the panel placement, the Selector row's spacer row, the status band's padding rows and the
  list's scrollbar column all carry `#2d353b`

#### Scenario: The non-Wide column body does not follow focus

- **WHEN** the library panel renders in non-Wide geometry with the library panel focused and without
  it, in either Hero Workspace focus state
- **THEN** the column body's painted fill differs only where the library panel's own focus bit
  differs (the body follows the panel's focus bit, never the Hero Workspace's)
- **AND** the Workspace's focus bit never changes the column body on its own

#### Scenario: The non-Wide list box matches the Wide Browser-pane list

- **WHEN** a non-Wide library list renders in either focus state
- **THEN** its list box carries the `LibraryPanel` fill for that state
- **AND** its alternating rows carry the `MainContentBox` fill for that state
- **AND** its scrollbar column carries the same fill the Wide Browser-pane list's column carries

#### Scenario: A very short non-Wide panel keeps one row

- **WHEN** a non-Wide library panel is too short to afford the row-flow inset
- **THEN** its list slot keeps a single row instead of collapsing to an empty rect

#### Scenario: A Workspace focused in Wide does not darken the non-Wide rail

- **WHEN** the Hero Workspace holds focus in Wide geometry and the panel transitions to non-Wide
  geometry with the Library Hero overlay closed
- **THEN** a panel-focused non-Wide browser rail still paints its focused list-box fill
- **AND** the rail's focus input does not depend on the Workspace's focus bit

#### Scenario: Wide keeps its own column fill

- **WHEN** the library panel renders in Wide geometry, focused or not
- **THEN** the library column resolves the same `LibraryColumn` pair against the panel's focus bit
  that non-Wide geometry resolves
- **AND** no geometry resolves a separate fixed-backdrop column arm

#### Scenario: Mini follows the non-Wide presentation

- **WHEN** the terminal is narrow enough for the mini view
- **THEN** the library pane resolves the same `LibraryColumn` pair against the panel's focus bit as
  Narrow, and the same `LibraryPanel`/`MainContentBox` list-box pair
- **AND** there is no Mini-specific paint bit or resting-only override

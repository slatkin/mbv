## MODIFIED Requirements

### Requirement: Modal styling matches the application palette

The modal body background SHALL depend on the active search mode: global mode SHALL use `LIBRARY_SIDE_BG` (#2d353b), and fuzzy mode SHALL use `BG_GREEN` (#3c4841). Both colours already exist in the application palette.

The search input row SHALL use the playback-panel background colour, the search input border and the hero block's rules SHALL use the unplayed seek-track colour, and text SHALL use the soft-white foreground colour. These SHALL remain the same regardless of mode.

Result rows, empty-state and loading messages, type-filter chip gaps, and the modal-frame fill SHALL use the same mode-dependent body background as the modal body itself.

#### Scenario: Modal drawn in global mode

- **WHEN** the search modal is rendered in global mode
- **THEN** the modal body, result rows, state messages, type-filter gaps, and modal-frame fill SHALL all use `LIBRARY_SIDE_BG`

#### Scenario: Modal drawn in fuzzy mode

- **WHEN** the search modal is rendered in fuzzy mode
- **THEN** the modal body, result rows, state messages, and modal-frame fill SHALL all use `BG_GREEN`

#### Scenario: Mode promoted while modal is open

- **WHEN** the modal is promoted from fuzzy to global while it is open
- **THEN** the background SHALL change from `BG_GREEN` to `LIBRARY_SIDE_BG` on the next frame
- **AND** no other styling (input row, borders, text, hero rules) SHALL change

#### Scenario: Modal drawn

- **WHEN** the search modal is rendered
- **THEN** its body, input row, borders, and text SHALL use the applicable palette colours
- **AND** the backdrop behind it SHALL be dimmed

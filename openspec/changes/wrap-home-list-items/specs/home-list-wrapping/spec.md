## MODIFIED Requirements

### Requirement: Home list labels remain fully readable

The Home tab list SHALL preserve the existing content and meaning of each rendered item while wrapping labels onto continuation rows instead of truncating them with an ellipsis. Non-episode items SHALL remain one row when their existing label and duration fit the available width.

#### Scenario: Fitting non-episode item

- **WHEN** a non-episode Home item label and its existing duration fit the list width
- **THEN** the item SHALL render as one row with the same content, styling, and duration placement as before

#### Scenario: Wrapping non-episode item

- **WHEN** a non-episode Home item label does not fit the available list width
- **THEN** the complete existing label SHALL render across one or more rows, with continuation rows indented beneath the content column and no ellipsis truncation

#### Scenario: Music item

- **WHEN** a Music item is rendered in the Home list
- **THEN** the Home list SHALL display the same fields and representation it displayed before this change, wrapping that existing content only when necessary

### Requirement: Episode layout remains inline when possible

The Home tab SHALL render an episode using the existing inline series/title representation when that representation fits with the existing duration. When it does not fit, the episode SHALL use a stacked series/title layout containing the complete existing series and episode text without truncation. The existing duration SHALL remain right-aligned on the first line of the episode-title block and SHALL not be repeated on continuation lines.

#### Scenario: Fitting episode

- **WHEN** the existing inline episode series/title representation fits the available width
- **THEN** the episode SHALL remain a one-row inline entry with its existing series and title styling

#### Scenario: Stacked episode

- **WHEN** the existing inline episode representation does not fit
- **THEN** the series and episode title SHALL render as coherent stacked content, with continuation rows indented and no ellipsis truncation

### Requirement: Variable-height rows preserve interaction

The Home list SHALL calculate physical content height from the rendered height of each item and SHALL preserve cursor visibility, scrolling, scrollbar geometry, selection decoration, and mouse hit testing for multi-row items.

#### Scenario: Selected wrapped item

- **WHEN** a wrapped item is selected and focused
- **THEN** its selection background and marker behavior SHALL cover the logical item without causing continuation rows to be treated as separate items

#### Scenario: Click continuation row

- **WHEN** the user clicks any visible continuation row of a Home item
- **THEN** the logical item owning that row SHALL be selected

#### Scenario: Oversized selected item

- **WHEN** the selected item's physical height exceeds the list viewport
- **THEN** the item's marker/first physical row SHALL remain reachable and visible while its continuation rows may extend beyond the viewport and be revealed through physical scrolling

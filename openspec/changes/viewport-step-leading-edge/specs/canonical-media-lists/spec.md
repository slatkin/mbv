## MODIFIED Requirements

### Requirement: The media-list viewport is scrollable independently of the selection

A canonical media list SHALL expose a viewport step that moves its visible window one display row in the
step direction without moving the selection, clamped so the window never passes the first or last display
row, and a page step that moves the window by its painted row-flow height.

A step SHALL drag the selection only when it would otherwise put the selection outside the window, and it
SHALL drag it to the leading edge of the step direction: the first selectable row of the new window when the
step moves toward the preceding display row, and the last selectable row of the new window when it moves
toward the following display row. The selection SHALL NOT be left on the window edge it was dragged off, so
that holding a step moves the selection with the gesture instead of pinning it to the row it is leaving.
Every selectable row SHALL stay reachable by stepping: no window position SHALL make the first display row,
the last display row, or the `Heading` that labels a group unreachable. A window MAY show the labelling
`Heading` directly above the selection, and stepping to the top of a grouped list SHALL show that list's
first `Heading` above its first item.

Every destination that composes a canonical list SHALL bind the viewport step to the pointed wheel
gesture and to a list-local chord, and the page step to `PgUp`/`PgDn`. No other chord SHALL move the
window, and the wheel and page chords SHALL move the selection only through the drag rule.

#### Scenario: A step inside the window moves only the viewport

- **WHEN** the user steps the viewport down while the selection is not on the window's first visible row
- **THEN** the window moves one display row
- **AND** the selection keeps its stable target and its painted screen row

#### Scenario: A step at the window edge drags the selection

- **WHEN** the user steps the viewport down while the selection is on the window's first visible row
- **THEN** the window moves down one display row
- **AND** the selection moves to the last selectable row the new window shows
- **AND** the selection is not left on the row it was dragged from

#### Scenario: A step up carries the selection to the leading edge

- **WHEN** the user steps the viewport up while the selection is on the window's last visible row
- **THEN** the window moves up one display row
- **AND** the selection moves to the first selectable row the new window shows
- **AND** stepping up again moves the window up one row with the selection riding that leading row

#### Scenario: Stepping back to the top restores the leading heading

- **WHEN** a grouped list has been scrolled away from the top and the user steps the viewport up until it stops
- **THEN** the window's first row is display row 0
- **AND** the first group's `Heading` is painted above its first item
- **AND** the selection is still painted

#### Scenario: The page step pages the viewport

- **WHEN** the user presses `PgDn` on a canonical list whose content extends beyond its painted height
- **THEN** the window moves by the painted row-flow height
- **AND** the selection is dragged into the window when the step would leave it outside
- **AND** the dragged selection lands on the leading-edge selectable row of the new window (`PgUp` lands it
  on the new window's first selectable row)

#### Scenario: A step at a content end changes nothing

- **WHEN** the user steps the viewport toward the first or last display row when the window is already at that end
- **THEN** the window and the selection are unchanged

#### Scenario: A cursor move still brings its row into view

- **WHEN** the user moves the selection with a cursor chord past the window's last visible row
- **THEN** the window moves the minimum distance that shows the selection — except that when that would
  place the selection on the window's first row with its labelling `Heading` directly above, the window
  moves one row further so the label stays visible
- **AND** the selection keeps its stable target

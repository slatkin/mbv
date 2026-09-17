## ADDED Requirements

### Requirement: The media-list viewport is scrollable independently of the selection

A canonical media list SHALL expose a viewport step that moves its visible window one display row in the
step direction without moving the selection, clamped so the window never passes the first or last display
row, and a page step that moves the window by its painted row-flow height.

A step SHALL drag the selection only when it would otherwise put the selection outside the window, and it
SHALL drag it to the nearest selectable row the window shows. Every selectable row SHALL stay reachable by
stepping: no window position SHALL make the first display row, the last display row, or the `Heading` that
labels a group unreachable. A window MAY show the labelling `Heading` directly above the selection, and
stepping to the top of a grouped list SHALL show that list's first `Heading` above its first item.

Every destination that composes a canonical list SHALL bind the viewport step to the pointed wheel
gesture and to a list-local chord, and the page step to `PgUp`/`PgDn`. No other chord SHALL move the
window, and the wheel and page chords SHALL move the selection only through the drag rule.

#### Scenario: A step inside the window moves only the viewport

- **WHEN** the user steps the viewport down while the selection is not on the window's last visible row
- **THEN** the window moves one display row
- **AND** the selection keeps its stable target and its painted screen row

#### Scenario: A step at the window edge drags the selection

- **WHEN** the user steps the viewport up while the selection is on the window's first visible row
- **THEN** the window moves up one display row
- **AND** the selection moves with it so it stays inside the window

#### Scenario: Stepping back to the top restores the leading heading

- **WHEN** a grouped list has been scrolled away from the top and the user steps the viewport up until it stops
- **THEN** the window's first row is display row 0
- **AND** the first group's `Heading` is painted above its first item
- **AND** the selection is still painted

#### Scenario: The page step pages the viewport

- **WHEN** the user presses `PgDn` on a canonical list whose content extends beyond its painted height
- **THEN** the window moves by the painted row-flow height
- **AND** the selection is dragged into the window when the step would leave it outside

#### Scenario: A step at a content end changes nothing

- **WHEN** the user steps the viewport toward the first or last display row when the window is already at that end
- **THEN** the window and the selection are unchanged

#### Scenario: A cursor move still brings its row into view

- **WHEN** the user moves the selection with a cursor chord past the window's last visible row
- **THEN** the window moves the minimum distance that shows the selection
- **AND** the selection keeps its stable target

### Requirement: The media-list viewport has one writer

The shared canonical media-list owner SHALL hold the visible window as its own row-local state. Input SHALL
be the only writer: a viewport step, a page step, a cursor move that would leave the selection outside the
window, and an explicit restore or hand-off boundary are the only operations that MAY change it.

Painting SHALL NOT write the window. The fixed-row presentation SHALL read the owner's window, clamp it to
the height it is painting for display only, and SHALL NOT store a resolved offset back into the owner. No
parent, projection, or restore path SHALL retain a second copy of the window, and no path SHALL recompute
it from the selection outside those input operations.

#### Scenario: A repaint with no input leaves the window unchanged

- **WHEN** a list is painted twice with no input between the paints and an unchanged row flow and height
- **THEN** the window is unchanged by the second paint
- **AND** the painted offset equals the window

#### Scenario: A shorter paint does not raise the window onto the selection

- **WHEN** a list's painted height shrinks while its selection lies inside the window
- **THEN** the painted offset is the window clamped to the new height
- **AND** the owner's window is not raised to the selection's display row

#### Scenario: A restore seeds the window once

- **WHEN** a saved position or a navigation boundary re-anchors a list
- **THEN** the window is seeded once from that explicit value
- **AND** no later paint changes it while the selection stays inside the window

## MODIFIED Requirements

### Requirement: Responsive handoff preserves an explicit anchor

A geometry change for one logical list SHALL reuse its shared canonical owner and fixed-row presentation. The presentation SHALL preserve the selected ordinary row's viewport offset when possible and clamp it to its new viewport otherwise. It SHALL NOT copy cursor, selected target, scroll, or other row-local state into a presentation-specific control. Ordinary refresh SHALL preserve the stable target and locally clamp. Only a discrete navigation or restoration boundary MAY explicitly re-anchor the shared owner from a shell-owned stable target and row offset.

A replacement of the row flow itself — letter regrouping, a provider reorder, a page append, or a refresh
— SHALL NOT carry the previous flow's display-row index into the new one. The owner SHALL re-anchor the
window from the first selectable row the previous flow showed at the window's top when that target is
still present, and otherwise SHALL keep the selection inside the window and clamp, so a replaced flow
never leaves the window pointing at unrelated rows. The stable target under the cursor SHALL survive the
replacement as it does for an ordinary refresh.

#### Scenario: TV re-anchors across breakpoints

- **WHEN** TV changes between Wide and non-Wide geometry
- **THEN** the same logical series owner and fixed-row presentation preserve the selected stable target and row-local state
- **AND** the viewport preserves or clamps the selected row offset
- **AND** no shell cursor or scroll mirror is adopted

#### Scenario: A reordered row flow keeps the window's place

- **WHEN** a list's rows are replaced in a different order while the user is not stepping
- **THEN** the window shows the previous top visible target at its painted offset where the new flow allows
- **AND** the selection keeps its stable target and stays inside the window
- **AND** no display-row index from the previous flow is carried over

#### Scenario: A page append keeps the window still

- **WHEN** more rows are appended to a list whose window is far from the appended end
- **THEN** the window and the selection are unchanged
- **AND** the appended rows extend the flow without moving what the user is reading

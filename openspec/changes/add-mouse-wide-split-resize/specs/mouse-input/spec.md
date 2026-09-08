## ADDED Requirements

### Requirement: The Wide hero split supports precise pane resizing

When a Wide hero two-pane surface is active, the existing gap columns between the browser (list) pane and the hero pane SHALL be a mouse resize target. Pressing the left button on those gap columns and dragging horizontally SHALL resize the list pane live at one-column precision, with the hero pane taking the remainder. The resulting list-pane width SHALL place the grabbed edge at the pointer column, subject to the shared arrangement's minimum pane widths: neither pane may fall below the minimum, so the split is clamped to the bounds implied by the active surface's content area.

The resize target SHALL use the existing gap without adding a divider, gutter, hover treatment, or wider invisible hit region. A left press and release without drag motion SHALL leave the split unchanged. The split SHALL be mouse-only: no keyboard binding SHALL move it.

A dedicated Interactive Component SHALL be the sole painter and gesture owner of the gap columns. The panes' hit geometry SHALL exclude the gap. The boundary owner SHALL receive mouse events through normal component subscriptions, follow the same overlay arbitration as other panel surfaces, recognize the gesture locally, and emit resolved widths. The shell SHALL NOT re-resolve raw pointer coordinates. Pane row dragging and other pane mouse gestures SHALL remain independently owned and SHALL NOT activate from the gap columns.

The resulting split SHALL be a single in-memory session width shared by every Wide hero surface: switching between wide surfaces SHALL apply the same split, clamped to each surface's valid range. The split SHALL NOT be persisted: it SHALL NOT be written to preferences or config, and SHALL return to the default split after the application restarts. Refreshing the current view SHALL revert the split to the default ratio. When the terminal is resized, the split SHALL be clamped to the valid range at the new size, not reverted.

#### Scenario: Drag resizes by one column

- **WHEN** a Wide hero surface is active and the user presses the gap columns between the list and hero panes and drags by one terminal column within the allowed range
- **THEN** the list pane becomes exactly one column wider or narrower during the drag and the hero pane takes the remainder
- **AND** the existing visual gap follows the pointer without adding new chrome

#### Scenario: Drag is clamped to the shared pane bounds

- **WHEN** an active split drag moves beyond the minimum or maximum list-pane width implied by the shared minimum pane widths
- **THEN** the live width remains at the bound nearest the pointer
- **AND** further motion beyond that bound does not exceed it

#### Scenario: Click without motion does not resize

- **WHEN** the user presses and releases the gap columns without drag motion
- **THEN** the split width remains unchanged

#### Scenario: No keyboard binding moves the split

- **WHEN** the user presses any keyboard chord while a Wide hero surface is active
- **THEN** the split stays at its current width
- **AND** keyboard resizing remains defined only for the queue column

#### Scenario: The split is shared across wide surfaces

- **WHEN** the user drags the split on one wide surface and switches to another wide surface
- **THEN** the other surface shows the same split
- **AND** if that surface's valid range is narrower, the split is clamped to it for that surface without changing the session width

#### Scenario: Refresh reverts to the default split

- **WHEN** the user refreshes the current view
- **THEN** the split returns to the default arrangement ratio on that surface
- **AND** the session width override is cleared

#### Scenario: The split is never persisted

- **WHEN** a split drag ends
- **THEN** no preference or config value is written
- **AND** after the application restarts the split is at the default ratio

#### Scenario: Terminal resize clamps and preserves

- **WHEN** the terminal is resized while a split override is active
- **THEN** the override is clamped to the valid range at the new size
- **AND** the override is not reverted to the default

#### Scenario: Press outside the gap does not arm resize

- **WHEN** the user presses inside either pane and then drags
- **THEN** split resizing is not armed
- **AND** the component that owns that pane remains free to interpret the gesture normally

#### Scenario: Overlay arbitration suppresses the boundary

- **WHEN** an overlay that arbitrates panel surfaces is mounted while the pointer drags across the gap columns
- **THEN** the boundary owner receives no gesture and the split does not change

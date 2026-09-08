## ADDED Requirements

### Requirement: The Queue panel boundary supports precise column resizing

When both panels are visible, the single full-height terminal column at the outer right edge of the Queue-side column SHALL be a mouse resize target. This is the root Queue column's trailing edge next to the Library panel, not the inset queue frame or list edge. Pressing the left button on that column and dragging horizontally SHALL resize the queue column live at one-column precision. The resulting width SHALL place the grabbed edge at the pointer column, subject to the same minimum and maximum bounds as keyboard resizing.

The resize target SHALL use the existing visual edge without adding a divider, gutter, hover treatment, or wider invisible hit region. A left press and release without drag motion SHALL leave the width unchanged. The final width SHALL be persisted when the drag ends, not once per drag event.

A dedicated Interactive Component SHALL be the sole painter and gesture owner of the boundary column. Root chrome and Queue destination hit geometry SHALL exclude that column. The boundary owner SHALL receive mouse events through normal component subscriptions, recognize the gesture locally, and emit resolved widths. The shell SHALL NOT re-resolve raw pointer coordinates. Queue row dragging and other panel mouse gestures SHALL remain independently owned and SHALL NOT activate from the boundary column.

#### Scenario: Drag resizes by one column

- **WHEN** both panels are visible and the user presses the Queue panel's right-edge column and drags it by one terminal column within the allowed range
- **THEN** the queue column becomes exactly one column wider or narrower during the drag
- **AND** the existing visual boundary follows the pointer without adding new chrome

#### Scenario: Drag is clamped to the existing bounds

- **WHEN** an active boundary drag moves beyond the minimum or maximum queue-column width
- **THEN** the live width remains at the existing bound nearest the pointer
- **AND** further motion beyond that bound does not exceed it

#### Scenario: Click without motion does not resize

- **WHEN** the user presses and releases the Queue panel boundary without drag motion
- **THEN** the queue-column width and persisted preference remain unchanged

#### Scenario: Final width is persisted once

- **WHEN** a boundary drag produces one or more live width changes and then ends
- **THEN** the final clamped width is persisted
- **AND** intermediate drag positions are not persisted individually

#### Scenario: Press outside the exact boundary does not arm resize

- **WHEN** the user presses in either panel outside the Queue panel's single-column right edge and then drags
- **THEN** queue-column resizing is not armed
- **AND** the component that owns that panel remains free to interpret the gesture normally

#### Scenario: Overlay suppresses boundary resizing

- **WHEN** an overlay or popup has exclusive mouse eligibility over the panels
- **THEN** the boundary owner does not receive the press or drag
- **AND** the queue-column width is unchanged

#### Scenario: Live tick delivers the complete resize gesture

- **WHEN** boundary press, drag, and release events are injected through the event listener and processed by the application's real tick and synchronization sequence
- **THEN** the boundary owner applies the resolved live width and persists the final width on release
- **AND** no Queue or Library destination handles the same gesture

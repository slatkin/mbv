# Spec Delta

## MODIFIED Requirements

### Requirement: Wheel scrolling moves the viewport by a uniform step

Every scrollable interactive surface SHALL interpret an accepted wheel gesture as a scroll of its viewport by one fixed step of three rows or text lines, toward the preceding rows for ScrollUp and the following rows for ScrollDown, clamped at the flow bounds. A wheel gesture SHALL NOT move, select or extend the selection or cursor. No surface SHALL use its own wheel step or a page-sized wheel movement.

#### Scenario: Wheel scrolls a canonical list by one step

- **WHEN** the user turns the wheel down once over a painted canonical media list with at least three rows below the viewport
- **THEN** the list's owning component moves its viewport down by three rows
- **AND** its selection, multi-selection and live range are unchanged

#### Scenario: Wheel scrolls the selection out of view

- **WHEN** the user turns the wheel until the selected row is outside the viewport
- **THEN** the selected row stays selected and is not painted
- **AND** the detail, hero and resting-position effects that follow the selection do not run

#### Scenario: Wheel moves a text viewport by one step

- **WHEN** the user turns the wheel up once over a painted text viewport with at least three preceding lines
- **THEN** the viewport's owning component moves its local offset back by three lines
- **AND** its cursor or selection remains unchanged

#### Scenario: A boundary clamps wheel movement

- **WHEN** the user turns the wheel toward the start or end of a scrollable surface whose viewport is already at, or fewer than three rows from, that boundary
- **THEN** the viewport stops at the boundary
- **AND** at the end boundary of a list that has been painted, the last flow row is the last row the viewport shows, never the first
- **AND** every list surface stops at the same position, whether or not the list repainted between wheel notches

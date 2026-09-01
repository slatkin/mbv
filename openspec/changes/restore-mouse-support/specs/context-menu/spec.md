## MODIFIED Requirements

### Requirement: Mouse-triggered menu retains pointer anchoring
When a context menu is opened by mouse, its anchor SHALL be the click position
and SHALL NOT depend on selected-item geometry. Placement SHALL be recalculated
while rendering and clamped to remain visible using the same bounds policy as a
keyboard-triggered menu. Right-click SHALL open the context menu on every
migrated interactive surface that paints a selectable row and has a
keyboard-triggered context menu, not only on a fixed subset of surfaces.

#### Scenario: Mouse-triggered menu opens
- **WHEN** a selectable row on any surface with a keyboard context menu is
  right-clicked
- **THEN** the row is focused and the menu opens from the click position rather
  than the selected item's keyboard anchor

#### Scenario: Right-click parity across migrated surfaces
- **WHEN** a surface supports opening its context menu by keyboard
- **THEN** right-clicking a selectable row on that surface opens the same menu
  with the same entries
- **AND** a surface with no keyboard context menu opens none on right-click

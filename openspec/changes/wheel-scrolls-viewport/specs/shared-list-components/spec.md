# Spec Delta

## MODIFIED Requirements

### Requirement: The viewport keeps the selection visible and clamps at bounds

A list SHALL keep its selection visible in its viewport while the viewport follows the selection. A viewport the user scrolled freely SHALL NOT be pulled back to the selection until it follows the selection again. When the selection survives a content or geometry change, the list SHALL preserve its prior viewport offset where bounds permit, and otherwise apply only the minimum scroll needed to bring it into view, clamping at flow bounds.

A geometry change SHALL clamp the existing viewport in place and SHALL NOT transfer viewport or cursor state into a second control. Paging distance SHALL be an explicitly named policy a list shape selects, because shapes differ in what a page means over their flow.

#### Scenario: A surviving selection keeps its viewport row
- **WHEN** content is replaced and the selected target is still present
- **THEN** its prior viewport offset is preserved where bounds permit
- **AND** otherwise the viewport scrolls the minimum needed to keep it visible

#### Scenario: Geometry change clamps in place
- **WHEN** a list's available area changes
- **THEN** the same list retains its cursor, viewport, and selection
- **AND** the viewport is clamped to the new bounds without copying state elsewhere

#### Scenario: Paging is a named shape policy
- **WHEN** two list shapes page their viewports
- **THEN** each applies its own explicitly named paging policy
- **AND** neither inherits the other's paging behavior implicitly

#### Scenario: A freely scrolled viewport stays where the user left it
- **WHEN** the user has scrolled the viewport so that the selection is out of view, and the list repaints, is resized, or receives refreshed content that keeps the selected target
- **THEN** the viewport offset stays where the user left it, clamped to the flow bounds
- **AND** the selection is not brought back into view

## ADDED Requirements

### Requirement: A selection change makes the viewport follow the selection again

A freely scrolled viewport SHALL return to following the selection on the next keyboard operation on the list, or the next operation that moves or sets the selection: a click, a select, a restore, or a shell re-anchor. Once following, the list SHALL apply the minimum scroll that brings the selection into view. The operation SHALL then act on the selection as it would without the earlier scroll.

#### Scenario: A key after a scroll brings the selection back
- **WHEN** the selection is out of view after a wheel scroll and the user presses the move-down key
- **THEN** the selection moves to the next row
- **AND** the viewport applies the minimum scroll that shows the new selection

#### Scenario: Activate after a scroll acts on the selection
- **WHEN** the selection is out of view after a wheel scroll and the user presses the activate key
- **THEN** the selected item is activated
- **AND** the viewport shows the selected row

#### Scenario: A click after a scroll selects the pointed row
- **WHEN** the user wheel-scrolls and then clicks a painted row
- **THEN** that row becomes the selection
- **AND** the viewport does not move

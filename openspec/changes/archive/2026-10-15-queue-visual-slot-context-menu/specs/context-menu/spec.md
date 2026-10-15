# Spec Delta

## ADDED Requirements

### Requirement: Right-click on the visual slot opens its menu

A right-click inside the queue visual slot's image rectangle SHALL open the
context menu at the click point, clamped inside the queue column, without
moving panel focus. It SHALL NOT open while the slot is idle or hidden. The
slot paints no selectable row and has no keyboard-triggered menu, so it is a
deliberate pointer-only exception to the row right-click parity rule.

#### Scenario: Right-click the visual slot image

- **WHEN** the queue visual slot paints its image rectangle and the user
  right-clicks inside it
- **THEN** the menu SHALL open at the click point, clamped inside the queue
  column

#### Scenario: Right-click outside the image rectangle

- **WHEN** the user right-clicks in the visual slot band outside the image
  rectangle, such as the blank space beside a centered narrow image
- **THEN** no menu SHALL open

#### Scenario: Focus does not move

- **WHEN** the Library panel holds focus and the user right-clicks the visual
  slot image
- **THEN** the menu SHALL open inside the queue column and the Library panel
  SHALL keep focus

#### Scenario: Slot hidden or idle

- **WHEN** the visual slot is hidden or playback is idle
- **THEN** a right-click on the slot region SHALL NOT open the menu

### Requirement: The visual slot menu offers the other content and hide

While the visual slot shows, its menu SHALL offer one entry selecting the
content the image is not showing and one entry hiding the slot. Each entry
SHALL perform the same transition as its keyboard action.

#### Scenario: Artwork is showing

- **WHEN** the visual slot shows artwork and the menu opens
- **THEN** it SHALL offer the visualizer entry and the hide entry

#### Scenario: Visualizer is showing

- **WHEN** the visual slot shows the visualizer and the menu opens
- **THEN** it SHALL offer the artwork entry and the hide entry

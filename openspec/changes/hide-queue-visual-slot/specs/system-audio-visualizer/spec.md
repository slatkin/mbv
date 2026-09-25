## ADDED Requirements

### Requirement: A hidden visual slot suspends the artwork and visualizer selection

While the user has hidden the queue's visual slot, pressing `v` SHALL have no effect: the
artwork/visualizer selection SHALL NOT change. While hidden, mbv SHALL NOT start or keep
system-audio capture and SHALL NOT fetch or prefetch card artwork. When the slot is shown again
during active playback, the selection in effect before hiding SHALL render, and capture or
artwork fetching SHALL resume as that selection requires.

#### Scenario: `v` while hidden does nothing

- **WHEN** the visual slot is hidden with artwork selected and the user presses `v`
- **THEN** the selection SHALL remain artwork, and showing the slot again SHALL display artwork

#### Scenario: Hiding the visualizer stops capture

- **WHEN** the visualizer is displayed during supported local playback and the user hides the slot
- **THEN** mbv SHALL stop system-audio capture and SHALL NOT repaint on the visualizer's frame cadence

#### Scenario: Showing the visualizer resumes capture

- **WHEN** the visualizer is selected, the slot is hidden, supported local playback is active, and
  the user shows the slot
- **THEN** mbv SHALL resume system-audio capture and display the visualizer

#### Scenario: Hidden slot fetches no artwork

- **WHEN** the visual slot is hidden with artwork selected and the now-playing item changes
- **THEN** mbv SHALL NOT fetch card artwork for it until the slot is shown again

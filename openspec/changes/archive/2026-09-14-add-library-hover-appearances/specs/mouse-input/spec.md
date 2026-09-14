## ADDED Requirements

### Requirement: Library navigation targets show presentation-only pointer hover

When the terminal delivers pointer movement, each visible tab label and each pill in the Library panel's main Selector row SHALL show a hover appearance while the pointer is inside that target's geometry from the most recently painted frame. The hover appearance SHALL be visually stronger than the target's resting appearance and SHALL NOT replace or weaken its selected appearance.

Hover SHALL be transient presentation state only. Moving the pointer SHALL NOT select, focus, activate, scroll, persist state, or dispatch external work. Workspace-selector pills, selection-modal pills, list controls, Queue and playback controls, and media rows SHALL retain their existing appearance and behavior.

#### Scenario: Pointer enters an unselected tab label

- **WHEN** the terminal delivers a pointer-movement event inside the latest painted hit region of a visible unselected tab label
- **THEN** that tab label is repainted with the hover appearance
- **AND** tab selection and keyboard focus remain unchanged

#### Scenario: Pointer enters an unselected main Selector-row pill

- **WHEN** the terminal delivers a pointer-movement event inside the latest painted hit region of an unselected pill in the Library panel's main Selector row
- **THEN** that pill is repainted with the hover appearance in both Narrow and Wide Panel modes
- **AND** the pill's selected value and Library content remain unchanged

#### Scenario: Selected appearance remains dominant

- **WHEN** the pointer moves over an already-selected tab label or main Selector-row pill
- **THEN** the target retains its selected appearance rather than being replaced by the unselected hover appearance
- **AND** no selection action is emitted

#### Scenario: Pointer leaves a target

- **WHEN** the pointer moves from a hovered target to a gap, another target, or another visible surface
- **THEN** the previous target returns to its non-hovered appearance
- **AND** a newly pointed in-scope target, if any, receives the hover appearance

#### Scenario: Pointer moves over an excluded target

- **WHEN** the pointer moves over a Workspace-selector pill, selection-modal pill, list control, Queue or playback control, or media row
- **THEN** that target's appearance and behavior remain unchanged by this capability

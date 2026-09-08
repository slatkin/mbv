## MODIFIED Requirements

### Requirement: Column resize deactivated outside both

Queue-column resizing by keyboard or mouse and the Alt+Left return-to-queue key SHALL be inactive whenever the Panel mode is not `both`. Mouse resizing SHALL also be inactive while an overlay or popup exclusively owns mouse delivery. Leaving `both` or losing panel mouse eligibility during an armed boundary gesture SHALL cancel that gesture without changing or persisting the width again.

#### Scenario: Resize disabled in queue-only

- **WHEN** the Panel mode is queue-only
- **THEN** the Shift+Left/Shift+Right column-width resize keys SHALL do nothing
- **AND** no queue-column mouse resize target SHALL be active

#### Scenario: Resize disabled in library-only

- **WHEN** the Panel mode is library-only
- **THEN** the Shift+Left/Shift+Right column-width resize keys SHALL do nothing
- **AND** no queue-column mouse resize target SHALL be active

#### Scenario: Return-to-queue disabled outside queue

- **WHEN** the Panel mode is not `both`
- **THEN** the Alt+Left return-to-queue key SHALL do nothing

#### Scenario: Interrupted boundary drag is cancelled

- **WHEN** an armed queue-column boundary drag loses eligibility because the Panel mode changes or an overlay or popup takes exclusive mouse delivery
- **THEN** the resize gesture is cancelled
- **AND** a later drag event cannot continue from the stale anchor

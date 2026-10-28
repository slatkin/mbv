# Spec Delta

## MODIFIED Requirements

### Requirement: Immediate direct-daemon intent presentation
For direct `mbvd` playback, the client SHALL visibly present a guarded playback intent on its first input before waiting for a daemon response. The guarded path SHALL be shared by keyboard and mouse dispatch and SHALL NOT require double-Space or double-stop-chord confirmation.

#### Scenario: First playback input is immediately visible
- **WHEN** the user invokes a guarded playback action for direct daemon playback
- **THEN** the client presents the pending action or target immediately and sends one correlated intent

#### Scenario: Legacy double-key confirmation is absent
- **WHEN** the user presses Space or the Stop chord once for an available direct-daemon playback action
- **THEN** the client dispatches that action without waiting for a second press

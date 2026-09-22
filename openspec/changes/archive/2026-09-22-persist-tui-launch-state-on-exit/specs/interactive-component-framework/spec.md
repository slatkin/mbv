# Spec Delta

## ADDED Requirements

### Requirement: Exit snapshots do not create live component-state mirrors
A TUI launch snapshot SHALL be assembled only at orderly exit from the selected destination's current component-owned main Selector pill and library-item identities, together with shell-owned tab selection and Panel focus. Snapshot extraction SHALL be a discrete persistence boundary and SHALL NOT copy component-owned pill, cursor, selection, or scroll state into continuously synchronized shell fields while the TUI is running.

Each destination component SHALL expose only the bounded stable identities needed for the launch snapshot. The shell SHALL remain the persistence authority and SHALL perform the disk write; an Interactive Component SHALL NOT receive a state path, persistence service, `Config`, or filesystem authority.

#### Scenario: Component-local launch state changes while running
- **WHEN** the selected destination changes its main Selector pill or selected library item
- **THEN** the component SHALL keep the live value under its existing presentation authority
- **THEN** the shell SHALL NOT mirror that value merely for later launch-state persistence
- **THEN** no launch-state disk write SHALL occur

#### Scenario: The TUI exits normally
- **WHEN** orderly teardown requests the selected destination's bounded launch-state identities
- **THEN** the destination SHALL return its current main Selector pill and selected library-item identities
- **THEN** the shell SHALL combine them with the selected tab and Panel focus and persist one snapshot
- **THEN** no unselected destination SHALL be queried or persisted

#### Scenario: Ordinary projection follows snapshot extraction
- **WHEN** shell-owned content is projected to a destination before or after launch-state extraction
- **THEN** the projection SHALL still exclude component-owned pill, cursor, scroll, and selection values
- **THEN** extraction SHALL NOT become a reverse synchronization path used during normal rendering or refresh

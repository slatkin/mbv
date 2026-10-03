# Spec Delta

## MODIFIED Requirements

### Requirement: Launch state is written only at orderly TUI exit

A TUI SHALL load the saved launch snapshot once during startup, keep subsequent launch-state changes in that TUI process's memory, and replace the saved snapshot only as part of orderly TUI exit. Cursor movement, tab changes, pill changes, item changes, Panel-focus changes, refreshes, and rendering SHALL NOT write the launch snapshot while the TUI remains open.

The explicit F2 Reset UI State action SHALL clear the saved launch snapshot immediately and abandon any pending in-memory launch restoration. Clearing is the sole explicit UI-reset exception; it SHALL NOT write a continuously updated or default replacement snapshot. Later orderly exit SHALL again save that Client's current bounded launch location under the existing last-completed-exit rule. Another running Client SHALL retain its live location and remain able to supply a later exit snapshot.

Explicit configuration changes, playback lifecycle and progress, queue persistence, caches, Service-owned state, and auto-reconnect state SHALL retain their own persistence lifecycles and SHALL NOT be delayed by this requirement.

#### Scenario: Two TUI Clients diverge while open
- **WHEN** two TUI Clients start from the same saved launch snapshot
- **WHEN** each Client selects a different tab, pill, item, or Panel focus
- **THEN** each Client SHALL retain its own launch-state changes in memory
- **THEN** neither Client SHALL change the saved launch snapshot before orderly exit unless the user explicitly invokes Reset UI State

#### Scenario: Concurrent Clients exit in sequence
- **WHEN** two TUI Clients have different in-memory launch locations
- **WHEN** one Client completes orderly exit and the other Client completes orderly exit later
- **THEN** the later completed exit SHALL supply the snapshot loaded by the next TUI launch
- **THEN** no Client identity, field-level merge, or daemon synchronization SHALL be required

#### Scenario: An explicit setting changes
- **WHEN** the user changes explicit configuration while the TUI is open
- **THEN** that configuration SHALL keep its existing persistence behavior
- **THEN** the configuration write SHALL NOT write the TUI launch snapshot as a side effect

#### Scenario: Reset cancels pending startup restoration
- **WHEN** Reset UI State is activated before saved tab, pill, item or Panel focus has finished restoring
- **THEN** pending restoration SHALL be abandoned
- **THEN** later catalog or detail arrivals SHALL NOT restore the discarded launch location

#### Scenario: A Client exits after reset
- **WHEN** the user resets UI state, selects a new launch location and exits normally
- **THEN** the current bounded location SHALL be saved as the next launch's starting point
- **THEN** reset SHALL NOT permanently disable launch-state persistence

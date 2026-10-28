# Spec Delta

## MODIFIED Requirements

### Requirement: Context-sensitive global actions are deferred candidates without timing state

A global action whose eligibility depends on whether the focused component consumes the same key SHALL remain a deferred candidate until the focused component's disposition is known. Consumed local input SHALL cancel the candidate. Unhandled local input SHALL fire the candidate on that press; there SHALL be no candidate timing state, repeated-press window, or deferred arming. Immediate global commands and blocking-overlay swallows SHALL retain their existing precedence. The `Esc` scenarios below apply when the `stop` action is configured to `Esc`; with default bindings `Esc` is not a stop chord.

#### Scenario: Visual Space suppresses playback

- **WHEN** a focused media list consumes `Space` for an active local Visual selection
- **THEN** the playback candidate does not fire
- **AND** the row-local toggle remains applied

#### Scenario: Visual Escape suppresses stop

- **WHEN** the `stop` action is configured to `Esc` and a focused media list consumes `Esc` to clear its active Visual selection
- **THEN** the stop candidate does not fire
- **AND** the local selection clearing remains applied

#### Scenario: Unhandled Space fires immediately

- **WHEN** the focused component reports `Space` as unhandled and a playback candidate is eligible
- **THEN** the playback action fires on that press
- **AND** no timing state is recorded for a later press

#### Scenario: Unhandled Escape fires immediately

- **WHEN** the `stop` action is configured to `Esc` and the focused component reports `Esc` as unhandled and a stop candidate is eligible
- **THEN** the stop action fires on that press
- **AND** no timing state is recorded for a later press

#### Scenario: No candidate timing survives a consumed press

- **WHEN** the focused component consumes a context-sensitive key and a later press of the same key is unhandled
- **THEN** the later press behaves as a first press, with no inherited candidate state

## ADDED Requirements

### Requirement: Focused components report local input disposition explicitly

A focused Interactive Component SHALL report whether each delivered key was unhandled or consumed by its local interaction. This disposition SHALL be independent of whether the component also requests work outside its authority. A purely local mutation SHALL remain local and SHALL NOT require the component to publish its private state merely to prove that it consumed the key.

The shell SHALL NOT infer consumption from the presence or absence of an effect request. A component-local mode, selection, draft, or pane state SHALL NOT be mirrored into shell state solely so the Keyboard Router can decide whether the current key belongs to the component.

#### Scenario: Purely local mutation is distinguishable

- **WHEN** a focused component changes private state for a delivered key and needs no shell-owned effect
- **THEN** it reports a consumed disposition with no effect request
- **AND** the same result cannot be mistaken for an unrecognized key

#### Scenario: Consumed input also requests an effect

- **WHEN** one local interaction both changes component-owned state and requests persistence, navigation, playback, or another shell-owned effect
- **THEN** the component reports a consumed disposition and the typed request independently
- **AND** neither fact is hidden by an exclusive outcome arm

#### Scenario: Router needs no component-state mirror

- **WHEN** a component-local state changes whether that component consumes a key
- **THEN** the component reports its disposition for the current event
- **AND** the shell does not copy that local state into the Keyboard Router's snapshot

### Requirement: Input arbitration is verified through live composition

The final relationship among focused-component delivery, Keyboard Router observation, and central arbitration SHALL be verified through the application's real tick and shell synchronization path. Tests SHALL cover consumed local input, unhandled local input, immediate global commands, deferred global candidates, and blocking-overlay swallows without hand-building the message order that the component framework produces.

#### Scenario: Live tick suppresses a deferred candidate

- **WHEN** a key is injected through the event listener while the focused component consumes it locally
- **THEN** the real application tick delivers both component and router results in framework order
- **AND** central arbitration suppresses the deferred global candidate

#### Scenario: Live tick permits a deferred candidate

- **WHEN** a key is injected while the focused component reports it unhandled
- **THEN** central arbitration permits the deferred candidate to arm or fire
- **AND** the test does not call the component handler or arbiter directly as a substitute for composition

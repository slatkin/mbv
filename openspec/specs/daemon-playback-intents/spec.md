# daemon-playback-intents Specification

## Purpose
TBD - created by archiving change reliable-daemon-playback-intents. Update Purpose after archive.
## Requirements
### Requirement: Immediate direct-daemon intent presentation
For direct `mbvd` playback, the client SHALL visibly present a guarded playback intent on its first input before waiting for a daemon response. The guarded path SHALL be shared by keyboard and mouse dispatch and SHALL NOT require double-Space or double-Escape confirmation.

#### Scenario: First playback input is immediately visible
- **WHEN** the user invokes a guarded playback action for direct daemon playback
- **THEN** the client presents the pending action or target immediately and sends one correlated intent

#### Scenario: Legacy double-key confirmation is absent
- **WHEN** the user presses Space or Escape once for an available direct-daemon playback action
- **THEN** the client dispatches that action without waiting for a second press

### Requirement: Correlated intent lifecycle
Each guarded playback intent SHALL carry a request identity and an ordering generation scoped to its control connection. The daemon SHALL report a correlated lifecycle outcome of Accepted, Applied, Coalesced, Superseded, or Rejected, and the client SHALL reconcile its pending presentation from those outcomes.

#### Scenario: Intent is accepted and applied
- **WHEN** the daemon accepts an intent and later confirms its requested playback effect
- **THEN** it reports Accepted followed by Applied for the same request identity

#### Scenario: Intent is rejected
- **WHEN** the daemon cannot validate, resolve, or perform an intent
- **THEN** it reports Rejected with a structured reason and the client clears or replaces the pending presentation

#### Scenario: Connection is lost
- **WHEN** the controlling connection closes with unresolved intents
- **THEN** the client clears their pending presentation and the daemon invalidates work from that connection

### Requirement: Latest Play intent wins
The daemon SHALL allow a newer different Play intent to supersede an unresolved or starting Play intent. Completion of superseded work MUST NOT mutate authoritative playback state or start playback.

#### Scenario: New target replaces unresolved target
- **WHEN** Play B arrives while a different Play A is unresolved or starting
- **THEN** A receives Superseded, B becomes current, and any later completion for A is ignored

#### Scenario: Stale lookup fails after supersession
- **WHEN** resolution for superseded Play A later returns an error
- **THEN** the error does not reject, replace, or otherwise affect current Play B

### Requirement: Equivalent Play is idempotent only during startup
An equivalent Play intent SHALL be coalesced while the same target is unresolved or starting. Once playback of that target has reached Applied, a subsequent Play for it SHALL be accepted as a deliberate restart.

#### Scenario: Duplicate Play during startup
- **WHEN** the same Play target is requested again before the canonical request reaches Applied
- **THEN** the duplicate receives Coalesced and does not start another resolution or player action

#### Scenario: Same-item Play after startup
- **WHEN** the currently playing target has reached Applied and the user invokes Play for it again
- **THEN** the daemon accepts a new intent and restarts that target using normal playback behavior

### Requirement: Stop always wins
Stop SHALL be available while direct-daemon playback is pending or active. An accepted Stop SHALL supersede all unresolved guarded intents, invalidate their completions, and become the current highest-priority playback intent.

#### Scenario: Stop during Play resolution
- **WHEN** Stop arrives while a Play target is being resolved
- **THEN** the Play receives Superseded, its later completion cannot start playback, and Stop proceeds

#### Scenario: Stop before active status
- **WHEN** a Play is pending but the player has not yet reported active playback
- **THEN** the client still allows Stop to be dispatched

### Requirement: Navigation is single-flight
Next and Previous SHALL each remain single-flight until the requested track change reaches Applied, Rejected, or Superseded. An equivalent repeated navigation input during that interval SHALL be coalesced.

#### Scenario: Repeated Next before confirmation
- **WHEN** Next is invoked again before the first Next changes the confirmed current item
- **THEN** the repeated request is coalesced and playback advances by only one item

#### Scenario: Next after confirmation
- **WHEN** the first Next has changed the confirmed current item and the user invokes Next again
- **THEN** a new navigation intent is accepted for the following item

#### Scenario: Repeated Previous before confirmation
- **WHEN** Previous is invoked again before the first Previous changes the confirmed current item
- **THEN** the repeated request is coalesced and playback moves back by only one item

### Requirement: Pause uses desired state
Direct-daemon Pause/Resume control SHALL transmit an explicit desired paused state rather than a toggle operation. Repeated requests for the same unresolved desired state SHALL be coalesced.

#### Scenario: Pause is pending
- **WHEN** confirmed playback is unpaused and the user invokes Pause/Resume repeatedly before confirmation
- **THEN** the client requests `paused = true` and the daemon applies at most one pause transition

#### Scenario: Resume after pause confirmation
- **WHEN** `paused = true` has been confirmed and the user invokes Pause/Resume
- **THEN** the client submits a new intent requesting `paused = false`

### Requirement: Slow resolution does not block newer intents
Play-target resolution SHALL NOT prevent the daemon control loop from accepting newer intents. Resolution completions SHALL be applied only when their connection identity and generation are still current.

#### Scenario: New command arrives during lookup
- **WHEN** a Play item lookup is slow and a newer Play or Stop arrives
- **THEN** the daemon accepts and orders the newer intent without waiting for the lookup to finish

### Requirement: Confirmed state remains authoritative
Pending intent presentation SHALL be distinct from confirmed playback state. The system MUST NOT report a guarded intent as Applied solely because it was queued to the player or because an early active flag was set.

#### Scenario: Player command is queued but not confirmed
- **WHEN** the daemon has submitted a Play action to the player but has not confirmed the requested target from a concrete player transition
- **THEN** the target remains pending and the daemon does not report the intent as Applied

### Requirement: Scope is limited to direct daemon playback
The guarded playback-intent behavior SHALL apply to playback controlled through the direct `mbvd` route. Local playback and attached Emby-session control SHALL retain their existing behavior.

#### Scenario: Attached Emby session is controlled
- **WHEN** the TUI controls an attached Emby session instead of direct `mbvd` playback
- **THEN** the new daemon intent lifecycle and idempotency policy are not applied to that session command


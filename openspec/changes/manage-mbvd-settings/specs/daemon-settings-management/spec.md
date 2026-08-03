## Purpose

Allow mbv users to inspect and manage a deliberately small set of daemon-wide operational settings while preserving host configuration as the bootstrap and inherited baseline.

## ADDED Requirements

### Requirement: F2 separates local and daemon settings
The F2 settings panel SHALL provide `LOCAL` and `DAEMON` scopes through a pill selector. `LOCAL` SHALL retain the existing client-side settings and persistence behavior. `DAEMON` SHALL contain only remotely manageable daemon settings and SHALL NOT reinterpret the selected playback target or modify client-side settings.

#### Scenario: User opens F2 settings
- **WHEN** the user opens the F2 settings panel
- **THEN** the panel SHALL show the `LOCAL` and `DAEMON` scope pills
- **THEN** the existing client-side settings SHALL appear under `LOCAL`

#### Scenario: User selects daemon scope
- **WHEN** the user selects the `DAEMON` pill using keyboard or mouse input
- **THEN** the panel SHALL replace the local setting rows with the remotely manageable daemon setting rows
- **THEN** changing a daemon row SHALL NOT change the client's local configuration

### Requirement: Daemon scope reflects service availability
The `DAEMON` scope SHALL remain discoverable when daemon-settings management is unavailable. It SHALL explain whether the shared-data connection is unavailable or the connected daemon lacks the required capability, and SHALL prevent edits while no current daemon-settings snapshot can be safely updated.

#### Scenario: Shared-data connection is unavailable
- **WHEN** the user opens the `DAEMON` scope without an active shared-data connection
- **THEN** the panel SHALL show that daemon settings are unavailable
- **THEN** it SHALL NOT present stale local values as current daemon settings or permit writes

#### Scenario: Connected daemon lacks the capability
- **WHEN** the shared-data connection is active but the daemon does not advertise daemon-settings management
- **THEN** the panel SHALL report that the connected daemon does not support remote settings
- **THEN** ordinary shared-state and playback behavior SHALL remain available

### Requirement: The remotely manageable setting set is explicit
The initial daemon-settings allowlist SHALL contain exactly `always_play_next`, `broadcast_ms`, and `audio_pipe_playout_delay_ms`. Requests SHALL identify settings through the typed allowlist rather than arbitrary configuration paths, and the service SHALL reject unknown fields or setting identifiers.

#### Scenario: Client requests the daemon settings snapshot
- **WHEN** a capable client requests daemon settings
- **THEN** the response SHALL describe exactly the three allowlisted settings
- **THEN** it SHALL expose no credentials, listener configuration, shared-data bootstrap configuration, server identity, or arbitrary `Config` field

#### Scenario: Client submits an unknown setting
- **WHEN** a client submits a setting not present in the allowlist
- **THEN** the daemon SHALL reject the request without changing the stored override document

### Requirement: Snapshot reports effective, inherited, and application state
For every allowlisted setting, the daemon SHALL report the effective value, whether an override is present, the effective value's source as `override`, `config`, or `default`, the setting's apply mode, and whether the effective value is active in the running daemon. The daemon SHALL derive this snapshot from its own defaults, host configuration, stored overrides, and runtime state; the client SHALL NOT reconstruct these facts from local knowledge.

The initial apply modes SHALL be `restart_required` for `always_play_next`, `restart_required` for `broadcast_ms`, and `next_playback` for `audio_pipe_playout_delay_ms`.

#### Scenario: Stored override wins
- **WHEN** a valid stored override exists for an allowlisted setting
- **THEN** the snapshot SHALL report that value as effective with source `override`

#### Scenario: Explicit host configuration is inherited
- **WHEN** no stored override exists and the daemon host explicitly configures the setting
- **THEN** the snapshot SHALL report the host-configured value as effective with source `config`

#### Scenario: Compiled default is inherited
- **WHEN** neither a stored override nor explicit host configuration exists for the setting
- **THEN** the snapshot SHALL report the compiled default as effective with source `default`

#### Scenario: Persisted value is not active yet
- **WHEN** an accepted override has not reached the boundary named by its apply mode
- **THEN** the snapshot SHALL distinguish the effective persisted value from the value active in the running daemon
- **THEN** the UI SHALL visibly report the pending apply mode rather than claiming the change is active

### Requirement: Override values are typed and validated
The daemon SHALL validate every proposed override before persistence. `always_play_next` SHALL accept a boolean, `broadcast_ms` SHALL accept an integer of at least 100 milliseconds, and `audio_pipe_playout_delay_ms` SHALL accept either a nonnegative integer in milliseconds or an explicit disabled value. Invalid values SHALL leave both stored and active settings unchanged.

#### Scenario: Valid value is proposed
- **WHEN** a client proposes a value valid for the selected setting
- **THEN** the daemon SHALL evaluate the update against the current document revision

#### Scenario: Invalid value is proposed
- **WHEN** a client proposes a value with an invalid type or range
- **THEN** the daemon SHALL reject it with a setting-specific explanation
- **THEN** it SHALL not mutate or increment the override document

### Requirement: Reset removes an override
The daemon settings UI SHALL let the user reset an overridden setting. Reset SHALL remove that field from the stored override document rather than storing a copy of the inherited value. The resulting snapshot SHALL immediately expose the value inherited from host configuration or compiled defaults.

#### Scenario: User resets an overridden setting
- **WHEN** the user resets a setting whose source is `override`
- **THEN** the daemon SHALL durably remove that setting's override
- **THEN** the acknowledged snapshot SHALL report the inherited effective value and its `config` or `default` source

#### Scenario: User resets an inherited setting
- **WHEN** the user requests reset for a setting with no stored override
- **THEN** the setting SHALL remain inherited without creating or changing an override

### Requirement: The override document is daemon-wide and independent of roaming state
The service SHALL maintain one daemon-wide, schema-versioned override document independently of authenticated users and independently of each user's roaming documents. A change accepted from one shared-data session SHALL therefore be observable by all connected sessions that support daemon-settings management.

#### Scenario: Different users inspect one daemon
- **WHEN** shared-data sessions authenticated as different Emby users request daemon settings from the same daemon
- **THEN** they SHALL receive the same daemon-wide override revision and resolved setting values

#### Scenario: Daemon setting changes
- **WHEN** a daemon setting update commits
- **THEN** per-user queue, library-position, reconnect, and roaming-settings documents SHALL remain unchanged

### Requirement: Writes use durable optimistic concurrency
The daemon SHALL assign the override document an independent monotonic revision. Updates and resets SHALL include the revision on which they are based. A matching update SHALL be validated and durably committed before acknowledgement; a stale update SHALL be rejected without mutation and return the current resolved snapshot. Revision zero SHALL represent an absent override document, and the first mutation SHALL create revision one.

#### Scenario: Current revision is updated
- **WHEN** a valid mutation supplies the current override revision
- **THEN** the daemon SHALL durably commit the resulting document at a higher revision before acknowledging it
- **THEN** the acknowledgement SHALL contain the resolved post-commit snapshot

#### Scenario: Stale revision is updated
- **WHEN** a mutation supplies an older override revision
- **THEN** the daemon SHALL leave the override document unchanged
- **THEN** it SHALL return the current resolved snapshot
- **THEN** the client SHALL adopt that snapshot and notify the user without retrying the rejected mutation

#### Scenario: Durable commit fails
- **WHEN** the override document cannot be committed
- **THEN** the daemon SHALL not acknowledge or broadcast the proposed value
- **THEN** the previously committed document and active behavior SHALL remain authoritative

### Requirement: Committed changes propagate to settings clients
After a durable override commit, the daemon SHALL notify other connected sessions that support daemon-settings management with the new resolved snapshot. It SHALL also notify those sessions when an effective value becomes active at its declared runtime boundary. Snapshots SHALL carry both the document revision and a runtime generation so clients can accept active-state changes that do not mutate the document. Notifications SHALL occur only after commit or runtime activation and SHALL not alter playback authority or produce per-user shared-document notifications.

#### Scenario: Another client commits a setting
- **WHEN** one settings client commits an override mutation
- **THEN** other capable connected settings clients SHALL adopt the newer snapshot
- **THEN** clients SHALL ignore snapshots older than or equal to the document-revision and runtime-generation pair they already hold

#### Scenario: Effective value becomes active
- **WHEN** a pending setting reaches its declared application boundary without another document mutation
- **THEN** capable connected settings clients SHALL receive a snapshot with the same document revision and a higher runtime generation

### Requirement: Existing shared-data authorization is reused
Daemon-settings reads and writes SHALL be available only after the existing shared-data connection has authenticated successfully. This capability SHALL NOT add administrator roles, authorization-policy configuration, or a second credential exchange.

#### Scenario: Authenticated shared-data client requests settings
- **WHEN** an authenticated shared-data session negotiates the daemon-settings capability
- **THEN** it SHALL be allowed to read and propose updates to the daemon-wide override document

#### Scenario: Connection has not authenticated
- **WHEN** a connection requests or mutates daemon settings before shared-data authentication succeeds
- **THEN** the daemon SHALL reject the operation without exposing a settings snapshot

### Requirement: Unsupported peers remain compatible
Daemon-settings protocol messages SHALL be guarded by an additive capability string. The change SHALL NOT alter the shared-data or ctrl protocol version, and peers without the capability SHALL continue their existing shared-state and playback behavior.

#### Scenario: Older client connects to a newer daemon
- **WHEN** a client does not request daemon-settings management
- **THEN** its existing shared-data session SHALL continue without receiving unsolicited daemon-settings messages

#### Scenario: Newer client connects to an older daemon
- **WHEN** the daemon hello omits the daemon-settings capability
- **THEN** the client SHALL disable only the `DAEMON` settings scope's remote operations

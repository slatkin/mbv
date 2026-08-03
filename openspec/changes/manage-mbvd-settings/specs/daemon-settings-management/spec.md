## Purpose

Allow mbv users to inspect and manage a deliberate set of packaged-`mbvd` playback-runtime settings while preserving host configuration as the inherited baseline.

## ADDED Requirements

### Requirement: F2 separates local and daemon settings
The F2 settings panel SHALL provide `LOCAL` and `DAEMON` scopes through a pill selector. `LOCAL` SHALL retain the existing client-side settings and persistence behavior. `DAEMON` SHALL contain only remotely manageable packaged-`mbvd` settings and SHALL NOT modify client-side settings or select a playback target.

#### Scenario: User opens F2 settings
- **WHEN** the user opens the F2 settings panel
- **THEN** the panel SHALL show the `LOCAL` and `DAEMON` scope pills
- **THEN** the existing client-side settings SHALL appear under `LOCAL`

#### Scenario: User selects daemon scope
- **WHEN** the user selects the `DAEMON` pill using keyboard or mouse input
- **THEN** the panel SHALL replace the local rows with packaged-`mbvd` runtime-setting rows
- **THEN** changing a daemon row SHALL NOT change the client's local configuration

### Requirement: Daemon scope reflects packaged-service availability
The `DAEMON` scope SHALL remain discoverable when daemon-settings management is unavailable. It SHALL explain whether the shared-data connection is unavailable or the packaged daemon lacks the required capability, and SHALL prevent edits without a current authoritative snapshot. The hidden `mbv --__local-daemon` stay-alive process SHALL NOT advertise or implement this capability.

#### Scenario: Shared-data connection is unavailable
- **WHEN** the user opens `DAEMON` without an active shared-data connection
- **THEN** the panel SHALL show that daemon settings are unavailable
- **THEN** it SHALL NOT present a cached snapshot as current or permit writes

#### Scenario: Connected service lacks the capability
- **WHEN** the shared-data connection is active but the service does not advertise daemon-settings management
- **THEN** the panel SHALL report that remote settings are unsupported
- **THEN** ordinary shared-state and playback behavior SHALL remain available

#### Scenario: Hidden local daemon is running
- **WHEN** mbv is attached to `mbv --__local-daemon`
- **THEN** that process SHALL neither expose nor apply managed daemon overrides

### Requirement: The remotely manageable setting set is explicit
The initial daemon-settings allowlist SHALL contain exactly `use_mpv_config`, `no_scripts`, `audio_pipe_enabled`, `audio_pipe_path`, `audio_pipe_samplerate`, `audio_pipe_bitdepth`, `audio_pipe_playout_delay_ms`, and `progress_interval_secs`. Requests SHALL identify settings through this typed allowlist rather than arbitrary configuration paths. Bootstrap, networking, security, client-preference, and restart-required settings SHALL NOT be remotely manageable.

#### Scenario: Client requests the daemon settings snapshot
- **WHEN** an authenticated client requests daemon settings from a capable packaged daemon
- **THEN** the response SHALL describe exactly the eight allowlisted settings
- **THEN** it SHALL expose no credentials, listeners, TLS configuration, shared-data bootstrap configuration, client playback preferences, or arbitrary `Config` fields

#### Scenario: Client submits an unknown setting identifier
- **WHEN** a client submits a setting identifier outside the allowlist
- **THEN** the daemon SHALL reject the request without changing the override document

### Requirement: Settings apply without daemon restart
The daemon SHALL apply `use_mpv_config`, `no_scripts`, `audio_pipe_enabled`, `audio_pipe_path`, `audio_pipe_samplerate`, `audio_pipe_bitdepth`, and `progress_interval_secs` when the next playback session begins. It SHALL capture `audio_pipe_playout_delay_ms` when the next pipe playback intent is accepted and SHALL use that captured value for the lifetime of that intent. A later mutation SHALL NOT change an in-flight playback session or intent.

#### Scenario: Playback-session setting changes
- **WHEN** a playback-session override commits while another session is active
- **THEN** the active session SHALL continue with its captured settings
- **THEN** the next playback session SHALL use the new effective value without restarting `mbvd`

#### Scenario: Playout delay changes
- **WHEN** a playout-delay override commits after a pipe playback intent has been accepted
- **THEN** the accepted intent SHALL retain its previously captured delay
- **THEN** the next accepted pipe playback intent SHALL capture the new effective delay

### Requirement: Snapshot distinguishes inherited, overridden, and active values
For every allowlisted setting, the packaged daemon SHALL report the effective value, the active value, whether the effective value is `inherited` or `override`, and its application boundary. The daemon SHALL derive these facts from ordinary configuration/default resolution, the stored override document, and runtime state; the client SHALL NOT reconstruct them from local knowledge.

#### Scenario: Stored override wins
- **WHEN** a valid stored override exists for an allowlisted setting
- **THEN** the snapshot SHALL report that value as effective with source `override`

#### Scenario: No stored override exists
- **WHEN** an allowlisted setting has no stored override
- **THEN** the snapshot SHALL report the daemon's ordinarily resolved value with source `inherited`

#### Scenario: Effective value is pending
- **WHEN** a committed effective value has not reached its application boundary
- **THEN** the snapshot SHALL distinguish it from the active value
- **THEN** the UI SHALL report the pending boundary rather than claiming the value is active

### Requirement: Override values are typed and validated
The packaged daemon SHALL validate every proposed override before persistence. `use_mpv_config`, `no_scripts`, and `audio_pipe_enabled` SHALL accept booleans. `audio_pipe_path` SHALL accept a nonempty path. `audio_pipe_samplerate` SHALL accept a positive integer representable by the playback runtime. `audio_pipe_bitdepth` SHALL accept exactly 16, 24, or 32. `progress_interval_secs` SHALL accept a positive integer. `audio_pipe_playout_delay_ms` SHALL accept either a safely representable nonnegative integer in milliseconds or an explicit disabled value. Invalid values SHALL leave stored and active settings unchanged.

#### Scenario: Valid value is proposed
- **WHEN** a client proposes a value valid for the selected setting
- **THEN** the daemon SHALL evaluate the mutation against the current document revision

#### Scenario: Invalid value is proposed
- **WHEN** a client proposes a value with an invalid type, range, or empty required path
- **THEN** the daemon SHALL reject it with a setting-specific explanation
- **THEN** it SHALL not mutate or increment the override document

### Requirement: Reset removes an override
The daemon settings UI SHALL let the user reset an overridden setting. Reset SHALL remove that field from the stored override document rather than storing a copy of the inherited value. A mutation whose revision is current but whose result is unchanged SHALL succeed without incrementing the revision, writing storage, or notifying subscribers.

#### Scenario: User resets an overridden setting
- **WHEN** the user resets a setting whose source is `override`
- **THEN** the daemon SHALL durably remove that setting's override
- **THEN** the acknowledged snapshot SHALL report the inherited effective value

#### Scenario: User resets an inherited setting
- **WHEN** the user requests reset using the current revision for a setting with no override
- **THEN** the daemon SHALL acknowledge the unchanged snapshot without incrementing its revision or notifying subscribers

#### Scenario: User sets the existing override value
- **WHEN** the user submits the current override value using the current revision
- **THEN** the daemon SHALL acknowledge the unchanged snapshot without incrementing its revision or notifying subscribers

### Requirement: The override document is daemon-wide and independent of roaming state
The service SHALL maintain one packaged-daemon-wide, schema-versioned override document independently of authenticated users and independently of each user's roaming documents. A change accepted from one authenticated shared-data session SHALL therefore be observable by every session subscribed to daemon-settings notifications.

#### Scenario: Different users inspect one daemon
- **WHEN** shared-data sessions authenticated as different Emby users request settings from the same packaged daemon
- **THEN** they SHALL receive the same daemon-wide override revision and resolved values

#### Scenario: Daemon setting changes
- **WHEN** a daemon setting update commits
- **THEN** per-user queue, library-position, reconnect, and roaming-settings documents SHALL remain unchanged
- **THEN** existing shared-data export behavior SHALL remain unchanged

### Requirement: Writes use durable optimistic concurrency
The daemon SHALL assign the override document an independent monotonic revision. Set and reset operations SHALL include the revision on which they are based. The daemon SHALL check that revision before determining whether the mutation is a no-op. A matching change SHALL be validated and durably committed before acknowledgement; a stale mutation SHALL be rejected without mutation and return the current snapshot. Revision zero SHALL represent an absent override document, and the first changing mutation SHALL create revision one.

#### Scenario: Current revision changes the document
- **WHEN** a valid changing mutation supplies the current override revision
- **THEN** the daemon SHALL durably commit the resulting document at a higher revision before acknowledging it
- **THEN** the acknowledgement SHALL contain the resolved post-commit snapshot

#### Scenario: Stale mutation would be a no-op against current state
- **WHEN** any mutation supplies an older override revision
- **THEN** the daemon SHALL reject it as stale even if applying it to the current document would make no change
- **THEN** it SHALL return the current snapshot without mutation

#### Scenario: Durable commit fails
- **WHEN** the override document cannot be committed
- **THEN** the daemon SHALL not acknowledge or broadcast the proposed value
- **THEN** the previously committed document and active behavior SHALL remain authoritative

### Requirement: Client mutations are serialized
The client SHALL maintain a typed mutation queue with at most one daemon-settings request in flight. Each queued mutation SHALL use the revision acknowledged by the preceding response. A correlated response SHALL complete its pending request even when its embedded snapshot is not newer than a notification already received.

#### Scenario: User makes several edits quickly
- **WHEN** the user submits another edit while a mutation is in flight
- **THEN** the client SHALL queue the later typed intent
- **THEN** it SHALL send that intent only after the in-flight request completes

#### Scenario: In-flight mutation is stale
- **WHEN** the daemon rejects the in-flight mutation as stale
- **THEN** the client SHALL adopt the current snapshot and SHALL NOT retry the rejected mutation
- **THEN** it SHALL preserve later queued intents and submit them against the adopted revision

#### Scenario: Connection closes with queued mutations
- **WHEN** the shared-data connection closes while mutations are pending or queued
- **THEN** the client SHALL clear those mutations
- **THEN** it SHALL visibly report that unsaved daemon-setting changes were discarded

### Requirement: Snapshot request establishes notification subscription
Requesting a daemon-settings snapshot SHALL subscribe that authenticated connection to post-commit and runtime-activation snapshots. Snapshots SHALL carry both the document revision and a runtime generation so clients can accept active-state changes that do not mutate the document. A reconnecting client SHALL request a fresh snapshot before enabling edits and thereby establish a new subscription.

#### Scenario: Another client commits a setting
- **WHEN** one client commits a daemon-setting mutation
- **THEN** other subscribed clients SHALL receive the resolved post-commit snapshot
- **THEN** unsubscribed connections SHALL receive no daemon-settings notification

#### Scenario: Effective value becomes active
- **WHEN** a pending value reaches its application boundary without another document mutation
- **THEN** subscribed clients SHALL receive a snapshot with the same document revision and a higher runtime generation

#### Scenario: Client reconnects
- **WHEN** a subscribed connection closes and later reconnects
- **THEN** the client SHALL discard its prior authoritative snapshot
- **THEN** it SHALL disable editing until a fresh snapshot request succeeds and establishes a new subscription

### Requirement: Existing shared-data authorization is the trust boundary
Daemon-settings reads and writes SHALL be available only after the existing shared-data connection authenticates successfully. Every authenticated shared-data user SHALL be trusted to manage the packaged daemon. This capability SHALL NOT add administrator roles, authorization-policy configuration, or a second credential exchange.

#### Scenario: Authenticated shared-data client requests settings
- **WHEN** an authenticated session requests daemon settings from a capable packaged daemon
- **THEN** it SHALL be allowed to read and propose mutations to the daemon-wide override document

#### Scenario: Connection has not authenticated
- **WHEN** a connection requests or mutates daemon settings before shared-data authentication succeeds
- **THEN** the daemon SHALL reject the operation without exposing a settings snapshot

### Requirement: Invalid stored settings fail safely
The packaged daemon SHALL accept only the supported override-document schema version. If the stored document has an unsupported version or is malformed, daemon-settings management SHALL remain unavailable for that run, all managed values SHALL use inherited behavior, and playback and per-user shared documents SHALL remain operational. The daemon SHALL log the error and SHALL NOT delete, rewrite, or partially recover the invalid record.

#### Scenario: Stored schema version is unsupported
- **WHEN** the packaged daemon reads an override document with an unsupported schema version
- **THEN** it SHALL ignore all overrides and disable daemon-settings management for that run
- **THEN** it SHALL preserve the stored record and continue playback from inherited settings

#### Scenario: Stored document is malformed
- **WHEN** the packaged daemon cannot strictly validate the supported-version document
- **THEN** it SHALL apply the same non-destructive fallback behavior as an unsupported version

### Requirement: Unsupported peers remain compatible
Daemon-settings protocol messages SHALL be guarded by an additive capability string. The change SHALL NOT alter the shared-data or ctrl protocol version, and peers without the capability SHALL continue their existing shared-state and playback behavior.

#### Scenario: Older client connects to a newer daemon
- **WHEN** a client does not request daemon-settings management
- **THEN** its existing shared-data session SHALL continue without receiving unsolicited daemon-settings messages

#### Scenario: Newer client connects to an older daemon
- **WHEN** the daemon hello omits the daemon-settings capability
- **THEN** the client SHALL disable only the `DAEMON` settings scope's remote operations

## Purpose

Defines verified just-in-time Audiobookshelf playback-session resolution, direct/HLS credential isolation, authoritative resume, and one-file mpv projection while user-facing playback remains disabled.

## ADDED Requirements

### Requirement: Playback-session decoding follows the validated 2.36 contract
Audiobookshelf playback-session, synchronization, and close payloads SHALL be decoded from sanitized live Audiobookshelf 2.36 fixtures. The implementation SHALL NOT guess unsupported response fields or older-server fallbacks.

#### Scenario: Live contract is not yet captured
- **WHEN** direct, forced-transcode, sync, close, and representative failure fixtures have not been validated
- **THEN** source and Player implementation SHALL NOT begin

#### Scenario: REST-only HLS is not viable
- **WHEN** live mpv validation proves HLS startup or ordinary seeking requires Socket.IO
- **THEN** this change SHALL stop for specification revision rather than adding Socket.IO implicitly

### Requirement: Playback resolution is just in time for the active item
The source boundary SHALL create an Audiobookshelf playback session only when an Audiobookshelf episode is prepared as the active canonical queue item. Inactive Audiobookshelf slots SHALL remain unresolved and SHALL NOT hold open server sessions or transcodes.

#### Scenario: Mixed queue starts on another item
- **WHEN** a queue contains an inactive Audiobookshelf episode after the active slot
- **THEN** no playback session or stream SHALL be created for that inactive episode

#### Scenario: Audiobookshelf slot is prepared
- **WHEN** the owner-driven source boundary prepares an Audiobookshelf episode
- **THEN** it SHALL create that episode's session using the current owner-local context
- **THEN** it SHALL validate the returned media identity before producing a source

### Requirement: Direct and HLS sources preserve credential isolation
The prepared source SHALL support validated direct and HLS session responses. Bearer authentication SHALL be scoped to a direct Audiobookshelf file only; session-scoped HLS, Emby, and Feed sources SHALL receive no Audiobookshelf credential.

#### Scenario: Direct source is returned
- **WHEN** Audiobookshelf returns a direct audio track
- **THEN** the prepared source SHALL carry the Bearer header only as a per-file option

#### Scenario: HLS source is returned
- **WHEN** Audiobookshelf returns a session-scoped HLS track
- **THEN** mbv SHALL wait for readiness within a bounded interval and load it without the Service credential

#### Scenario: Following source belongs to another Service
- **WHEN** a non-Audiobookshelf source follows direct Audiobookshelf preparation
- **THEN** the following source SHALL receive no Audiobookshelf Authorization header

### Requirement: Audiobookshelf determines the prepared start position
The playback-session response's current position SHALL be the authoritative prepared start position. Generic audio resume thresholds SHALL NOT override it.

#### Scenario: Server returns resumable progress
- **WHEN** session creation returns a positive current position
- **THEN** the prepared source SHALL start at that position

#### Scenario: Server resets a finished episode
- **WHEN** session creation returns zero for a previously finished episode
- **THEN** the prepared source SHALL start at zero

### Requirement: Opened sessions are cleaned up on source failure
If session creation succeeds but validation, readiness, or mpv load/start fails, mbv SHALL make a bounded best-effort close request and clear local lifecycle state.

#### Scenario: Prepared source cannot start
- **WHEN** an Audiobookshelf session opens but the source cannot be loaded
- **THEN** mbv SHALL close the session within a bound and report a redacted failure
- **THEN** no false active lifecycle SHALL remain

### Requirement: Source infrastructure remains unavailable to users and ctrl
This capability SHALL NOT make any Player owner eligible for Audiobookshelf items, activate episode play/enqueue, transmit prepared state through ctrl, or periodically report listening progress.

#### Scenario: Source change is applied without playback activation
- **WHEN** this change is applied before the final playback child
- **THEN** explicit Audiobookshelf submission SHALL remain visibly unsupported
- **THEN** credentials and prepared state SHALL remain inside owner-local source tests and runtime boundaries

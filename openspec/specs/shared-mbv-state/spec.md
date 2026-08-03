# shared-mbv-state Specification

## Purpose

Provide optional, durable, per-user roaming of mbv-owned state through a canonical `mbvd` while preserving local operation whenever that service is unavailable.

## Requirements

### Requirement: Shared-data hosting and use are explicit opt-ins

The system SHALL host shared data only when the canonical daemon has shared-data hosting enabled. A client SHALL use shared data only when configured with an explicit shared-data endpoint. The shared-data endpoint SHALL be independent of the client's playback endpoint and library routes.

#### Scenario: Hosting is disabled

- **WHEN** the daemon starts without shared-data hosting enabled
- **THEN** it SHALL open no shared-data listener or database
- **THEN** ordinary daemon playback behavior SHALL remain unchanged

#### Scenario: Client use is disabled

- **WHEN** a client has no shared-data endpoint configured
- **THEN** it SHALL use the existing local persistence behavior

#### Scenario: Playback route changes

- **WHEN** a participating client changes its playback route
- **THEN** its shared-data endpoint and authenticated shared-data session SHALL remain unchanged

### Requirement: Shared documents are isolated per Emby user

The service SHALL maintain an independent document set for each unambiguously authenticated Emby user. The initial document set SHALL consist of the complete existing queue state, complete existing library-position state, complete existing last-remote-connection state, and a roaming-settings document containing exactly `auto_reconnect` and `library_routes`.

Live Player state, media or library caches, and all other settings SHALL NOT be stored by this capability.

#### Scenario: Two users access the same daemon

- **WHEN** two clients authenticated as different Emby users read or update the same document type
- **THEN** each client SHALL access only the document belonging to its own user ID

#### Scenario: Queue state is stored

- **WHEN** a client writes queue state
- **THEN** the shared document SHALL preserve the existing queue-state schema and all its fields

#### Scenario: Cache data remains local

- **WHEN** a client participates in shared state
- **THEN** its media and library caches SHALL remain machine-local

### Requirement: Shared identity is verified and fail-closed

Before granting access, the service SHALL validate the presented Emby token as one specific user through the configured Emby server. Validation SHALL require a successful current-user response with a non-empty user ID. API-key fallback that selects or infers a user from the server's user list SHALL NOT authorize shared-state access.

The service SHALL NOT persist or log presented bearer tokens.

#### Scenario: User-scoped token is valid

- **WHEN** current-user token validation succeeds with a non-empty user ID
- **THEN** the connection SHALL be authorized only for that user's document set

#### Scenario: API key has no user identity

- **WHEN** current-user validation does not identify one user but the token can list server users as an API key
- **THEN** the service SHALL reject shared-state access

#### Scenario: Identity validation is unavailable

- **WHEN** the daemon cannot complete token validation
- **THEN** it SHALL reject the new shared-data connection without exposing any documents

### Requirement: Shared-data transport is local or private-network only

The service SHALL accept shared-data connections only over Unix-domain sockets, loopback TCP, or private-network TCP endpoints. Public/WAN TCP endpoints SHALL be rejected before an Emby token is sent. Plaintext TCP MAY be used for loopback and private-network endpoints because shared-data hosting is explicitly opt-in and LAN-scoped. TLS MAY be enabled for TCP endpoints as an additional protection; when enabled, clients SHALL validate the server certificate before sending an Emby token. A local Unix-domain connection MAY rely on operating-system transport isolation without TLS.

#### Scenario: Private plaintext endpoint

- **WHEN** a client is configured with a loopback or private-network `tcp://` endpoint
- **THEN** the client MAY proceed with Emby token authentication over that endpoint

#### Scenario: Valid TLS endpoint

- **WHEN** a client connects to a configured `tls://` endpoint whose certificate is valid for the endpoint
- **THEN** the client MAY proceed with Emby token authentication

#### Scenario: Invalid server certificate

- **WHEN** certificate validation fails for a configured `tls://` endpoint
- **THEN** the client SHALL send no Emby token and SHALL enter local fallback

#### Scenario: WAN endpoint

- **WHEN** a client or daemon is configured with a public/WAN TCP shared-data endpoint
- **THEN** it SHALL reject the endpoint before sending or accepting an Emby token
- **THEN** it SHALL enter or remain in local fallback

### Requirement: First writer initializes an absent document

When a user's shared document does not exist, a client SHALL attempt to create it from the corresponding current local state. Creation SHALL be atomic and conditional on the document remaining absent. If clients race to initialize a document, the first accepted creation SHALL win and other clients SHALL adopt the created shared value.

#### Scenario: First participating client connects

- **WHEN** the user's queue document is absent and a client has local queue state
- **THEN** the service SHALL atomically create the shared queue document from that local state

#### Scenario: Clients race to initialize

- **WHEN** two clients concurrently attempt to create the same absent document
- **THEN** exactly one creation SHALL succeed
- **THEN** the losing client SHALL adopt the winning shared document

### Requirement: Shared roaming settings override local configuration

While shared state or its local mirror is active, `auto_reconnect` and `library_routes` from that state SHALL override local `config.toml` values. The client SHALL NOT rewrite `config.toml`. At each shared-data connection, the client SHALL log once for each explicitly configured local value that differs from the shared value; compiled defaults SHALL NOT be reported as conflicts.

#### Scenario: Shared and explicit local values differ

- **WHEN** a shared-data connection supplies a roaming value that differs from an explicitly configured local value
- **THEN** the shared value SHALL take effect
- **THEN** the mismatch SHALL be logged once for that connection

#### Scenario: No shared value has ever been mirrored

- **WHEN** shared data is unavailable and no local shared-state mirror exists
- **THEN** the client SHALL use its ordinary local configuration values

### Requirement: Documents use independent optimistic revisions

Each shared document SHALL have an independent monotonically increasing revision. Create operations SHALL require the document to be absent, and update operations SHALL include the revision on which they are based. The service SHALL atomically reject an update whose expected revision is stale and return the current document.

No operation SHALL require an atomic transaction across different document types.

#### Scenario: Update uses the current revision

- **WHEN** a client updates a document using its current revision
- **THEN** the service SHALL durably commit the new value with a higher revision before acknowledging it

#### Scenario: Update uses a stale revision

- **WHEN** a client updates a document using an older revision
- **THEN** the service SHALL leave the stored document unchanged
- **THEN** the client SHALL adopt the returned current shared document and show a notification toast

#### Scenario: Different documents update concurrently

- **WHEN** clients update different document types concurrently
- **THEN** each update SHALL be evaluated against only its document's revision

### Requirement: Committed updates propagate to connected clients

After committing a document update, the service SHALL notify every connected shared-data client authenticated as that user. It SHALL NOT notify clients authenticated as other users. Notifications SHALL occur only after durable commit and SHALL carry enough information for recipients to adopt the committed revision without overwriting it.

Shared-data activity SHALL NOT acquire playback authority or cause playback queue/status broadcasts.

#### Scenario: Another client commits an update

- **WHEN** one client commits a queue document update
- **THEN** other shared-data clients for the same user SHALL adopt that committed revision
- **THEN** clients for other users SHALL receive no notification

#### Scenario: Data-only client connects

- **WHEN** a client establishes a shared-data connection without a playback connection
- **THEN** the connection SHALL neither acquire playback authority nor receive live playback broadcasts

### Requirement: Connected state is mirrored locally

Every shared document accepted by a client SHALL be atomically persisted to its corresponding local mirror after the shared commit is known. Existing local state document schemas SHALL remain unchanged. The roaming-settings mirror SHALL be stored separately and SHALL NOT modify `config.toml`.

#### Scenario: Shared update is accepted

- **WHEN** a client accepts a committed shared queue document
- **THEN** it SHALL atomically update its local queue-state file with the same value

#### Scenario: Roaming settings are accepted

- **WHEN** a client accepts shared roaming settings
- **THEN** it SHALL update the local roaming-settings mirror without rewriting `config.toml`

### Requirement: Shared failure falls back locally and retries

If initial connection, restoration, or a shared write fails, the client SHALL continue using local state, show a notification toast explaining that shared data is unavailable, and retry the configured shared endpoint in the background with bounded exponential backoff.

A failed shared write SHALL be persisted locally before normal operation continues. Browsing and playback SHALL remain available during fallback.

#### Scenario: Shared service is unavailable at startup

- **WHEN** the client cannot restore from its configured shared endpoint
- **THEN** it SHALL restore from local state
- **THEN** it SHALL show one fallback toast for that transition and begin background retries

#### Scenario: Shared write fails mid-session

- **WHEN** a shared update cannot be committed or acknowledged
- **THEN** the client SHALL persist the update locally
- **THEN** it SHALL enter local fallback, show one fallback toast for that transition, and begin background retries

### Requirement: Shared state regains authority after reconnection

When a fallback client reconnects, existing shared documents SHALL replace divergent local fallback documents without prompting the user. The client SHALL mirror and apply the shared values, show a reconnection notification toast, and resume shared writes. An absent shared document SHALL instead follow first-writer initialization.

#### Scenario: Shared and fallback state diverged

- **WHEN** a client reconnects after changing local fallback state and the corresponding shared document exists
- **THEN** the client SHALL discard the divergent fallback value and adopt the shared document
- **THEN** it SHALL notify the user without asking which value to keep

#### Scenario: Shared document is absent after reconnection

- **WHEN** a fallback client reconnects and the corresponding shared document is absent
- **THEN** it SHALL attempt conditional initialization from its local value

### Requirement: Storage failure is isolated from playback

The daemon SHALL acknowledge a write only after the database commits it. Database open, corruption, serialization, disk-full, and commit failures SHALL fail shared-data hosting or the affected operation without stopping daemon playback or damaging a previously committed value.

#### Scenario: Database cannot open

- **WHEN** shared-data hosting is enabled but its database cannot be opened safely
- **THEN** shared-data hosting SHALL remain unavailable
- **THEN** daemon playback SHALL continue

#### Scenario: Commit fails

- **WHEN** durable commit of an update fails
- **THEN** the service SHALL not acknowledge or broadcast the proposed revision
- **THEN** the previously committed document SHALL remain authoritative

### Requirement: Shared documents are exportable as JSON

An administrator on the daemon host SHALL be able to export all committed documents and their revisions as JSON without exposing bearer tokens. Export SHALL operate locally rather than through the shared-data network protocol and SHALL not mutate the database.

#### Scenario: Administrator exports shared data

- **WHEN** a local administrator requests an export from a readable database
- **THEN** the output SHALL contain each user-scoped document value, type, and revision as JSON
- **THEN** it SHALL contain no authentication token

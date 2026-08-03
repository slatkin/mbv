## Context

See `proposal.md` for motivation and `specs/shared-mbv-state/spec.md` for observable behavior. Existing mbv persistence writes Serde JSON documents to local files. The ctrl transport is newline-delimited JSON, authenticates a presented Emby token, and currently falls back from `/Users/Me` to selecting the first user returned by API-key access. Its remote TCP path is plaintext and tightly coupled to playback state and authority.

The shared store spans daemon configuration, transport/authentication, durable storage, state restoration, settings precedence, concurrent clients, and TUI notifications. It therefore needs a separate logical service even if it reuses framing and lower-level connection utilities.

## Goals / Non-Goals

**Goals:**

- Keep the daemon the sole process that opens the shared database.
- Reuse existing state structures and local persistence paths wherever possible.
- Make authorization fail closed and prevent bearer-token exposure on remote links.
- Serialize durable mutations simply without holding playback or network locks.
- Make outage behavior predictable: local operation continues, while shared state wins whenever available.
- Keep protocol evolution additive through an advertised capability.

**Non-Goals:**

- Replication, clustering, high availability, offline write queues, or document merging.
- Sharing caches or live Player state.
- A generic remote key-value, JSON, SQL, or daemon-settings API.
- Automatic certificate provisioning or a plaintext compatibility mode.
- Automatic backups beyond `redb` durability and a local JSON export command.

## Decisions

### Use a separate shared-data listener and session role

Shared data uses its own configured endpoint and connection registry. Its handshake advertises a versioned capability string such as `shared-mbv-state-v1`; unsupported peers fail only this optional connection. The connection carries only shared-data requests, responses, and same-user document notifications.

This is preferable to adding storage commands to playback connections because playback routes change, playback authority has unrelated rules, and current ctrl clients receive queue/status broadcasts. It also avoids changing `CTRL_PROTOCOL_VERSION`: this is an additive capability with a separate role.

### Require TLS for TCP and permit local Unix sockets

Remote endpoints use TLS with normal hostname and trust-chain validation. The daemon is configured with a TLS server identity; the client does not offer an ignore-certificate-errors switch. The implementation can use the workspace's existing `native-tls` dependency. Unix-domain endpoints may omit TLS because credentials and data do not leave the host and filesystem permissions protect the socket.

Plain TCP was rejected because the handshake carries a reusable Emby bearer token and the documents reveal viewing and routing history. Certificate pinning and generated self-signed identities were rejected for the initial design because they add a second trust-distribution mechanism; operators may use a private CA if public certificates are unsuitable.

The TLS handshake completes before the client serializes or sends its Emby token. Connection and authentication have hard time bounds and happen outside database and playback locks.

### Authenticate only through current-user validation

The service derives its authorization key only from a successful `/Users/Me` response containing a non-empty ID. It does not use the current `/Users` API-key fallback. The authenticated user ID is stored in connection memory, while the bearer token is discarded after validation and is never logged or written to disk.

This uses Emby as the existing identity authority while removing the ambiguity that currently maps an admin API key to the first listed user. Authentication is performed once per connection; reconnecting requires fresh validation. Temporary Emby failure rejects new shared sessions but does not revoke already authenticated sessions.

### Store four independent per-user documents in redb

Use a single `redb` database owned by the daemon with records keyed by `(emby_user_id, document_kind)`. The four kinds are:

- `queue_state`: the existing complete `QueueState` JSON.
- `library_position_state`: the existing complete `LibraryPositionState` JSON.
- `last_remote_connection`: the existing complete reconnect JSON.
- `roaming_settings`: exactly `auto_reconnect` and `library_routes`.

Each record contains a `u64` revision and Serde JSON bytes. Revision zero is reserved for absence; the first committed value receives revision one. Keeping JSON as the value format avoids duplicate models and enables export, while `redb` supplies atomic commit and crash consistency without rewrite/rename churn for the database as a whole.

The database and parent directory use owner-only permissions. Database format metadata is stored separately from document revisions so physical migrations do not alter concurrency semantics.

### Serialize mutations through one storage worker

A dedicated storage worker owns database operations. Connection handlers send bounded requests to it and await bounded responses without holding connection-registry, playback, queue, or Player locks. The worker processes one write transaction at a time:

1. Read the current record.
2. Verify absent/create or expected revision/update semantics.
3. Serialize and commit the replacement with revision incremented by one.
4. Return the committed record or a stale-write response containing the current record.

After a successful commit, the connection layer acknowledges the requester and fans the committed record out to other same-user shared-data sessions. It snapshots recipients before sending and never holds the registry or database transaction across socket writes. Failed commits produce neither acknowledgement nor broadcast.

Reads may obtain all four documents in one read transaction for an efficient connection snapshot, but clients treat their revisions independently. No cross-document atomic write API is exposed.

An unbounded thread-per-write design was rejected because it complicates ordering and lock safety. Last-writer-wins was rejected because stale whole-document queue writes could silently destroy newer work.

### Use authoritative snapshots rather than offline synchronization

Clients retain one revision per document while connected. A write supplies its expected revision. On stale rejection, the client immediately adopts the returned shared record, mirrors it locally, and shows a toast. It does not retry the rejected mutation.

When a document is absent, a client conditionally creates it from its current local value. Concurrent initializers race through the same create-if-absent operation; the winner becomes authoritative and losers adopt it.

This gives deterministic conflict handling with no merge engine. It intentionally discards fallback changes if an existing shared document is found after reconnection, matching the user's explicit opt-in to shared authority.

### Keep local files as a continuously refreshed fallback mirror

Once a shared record is accepted, the client applies it and atomically writes the corresponding local mirror. The three existing state documents keep their current schemas and files. Roaming settings use a separate local mirror so `config.toml` remains an expression of machine-local intent.

While the roaming-settings mirror is active, it overrides local `auto_reconnect` and `library_routes`; explicit mismatches are logged once per shared connection. If no mirror has ever been obtained, ordinary local configuration applies during fallback.

Client state is a small state machine:

```text
Disabled -> Local
Configured -> Connecting
Connecting --success--> Shared
Connecting --failure--> LocalFallback --retry--> Connecting
Shared --write/connection failure--> LocalFallback
```

On a shared write failure, the client atomically persists the proposed value locally, enters fallback once, and shows one transition toast. Retries use exponential backoff with jitter, capped at 60 seconds, and reset after a successful authenticated snapshot. Reconnection applies existing shared documents before resuming shared writes, mirrors them, and shows one reconnection toast. No modal conflict choice is offered.

### Keep storage failures feature-local

Shared hosting startup occurs after playback-critical local daemon setup. If the database cannot be opened, the daemon logs the failure and does not expose the shared listener, but playback continues. The database is never automatically deleted or recreated after corruption.

Disk-full, serialization, and commit failures reject the operation. Because redb commits before acknowledgement, the previous record remains authoritative. Clients interpret a failed or timed-out write as loss of shared availability and follow the fallback path.

### Export through a local administrative command

Add a local `mbvd` export operation that opens the database read-only/exclusively as required and writes a JSON snapshot containing user IDs, document kinds, revisions, and parsed values. It includes no credentials and is not exposed over the network protocol. The output is created with owner-only permissions.

Export is the minimal inspection and manual-recovery mechanism. Import, automated backup scheduling, and online database repair are deferred because they introduce destructive policy decisions not required for safe initial operation.

## Risks / Trade-offs

- [Fallback edits are intentionally lost when shared state returns] -> Use clear transition toasts and preserve deterministic shared authority rather than presenting conflict management.
- [Whole queue documents can conflict during active use on two computers] -> Independent CAS revisions prevent silent overwrite; stale writers immediately adopt the committed shared snapshot.
- [TLS certificate setup adds host administration] -> Reuse standard platform trust and the existing TLS stack; do not introduce an insecure mode or custom pin database.
- [Emby downtime prevents new shared connections] -> Existing authenticated sessions continue, and disconnected clients remain functional from local mirrors.
- [A malicious valid user token exposes that user's roaming state] -> Scope every database and broadcast operation to the validated user ID and never accept a caller-supplied user ID.
- [A corrupt database disables roaming] -> Fail without recreation, preserve the file for recovery, keep playback operational, and provide local JSON export when readable.
- [Broadcasting after commit can fail for some clients] -> The committed revision remains authoritative; disconnected clients recover through a fresh snapshot and revision comparison.

## Migration Plan

1. Add daemon hosting configuration with no listener or database created by default.
2. Add the storage and export layer before exposing the network service.
3. Add the authenticated TLS/Unix shared-data service and capability handshake.
4. Add client configuration, restoration, mirroring, fallback, and notification behavior.
5. On first opt-in connection, initialize each absent document independently from that client's local state. Existing local files are retained as mirrors.
6. Roll back by disabling the client endpoint and daemon hosting. Clients continue from their mirrored local files; the redb database is preserved for re-enablement or export.

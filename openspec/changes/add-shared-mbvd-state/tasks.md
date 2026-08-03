## 1. Configuration And Data Model

- [x] 1.1 Add the `redb` dependency and define shared document kinds, revisioned record envelopes, protocol requests, responses, notifications, and the additive shared-state capability string.
- [x] 1.2 Add disabled-by-default daemon configuration for the shared database, dedicated listener, and TLS server identity, with validation that prevents plaintext remote hosting.
- [x] 1.3 Add optional client shared-data endpoint configuration and endpoint validation that rejects plaintext non-local TCP before credentials are sent.
- [x] 1.4 Add the roaming-settings document type containing only `auto_reconnect` and `library_routes`, plus a separate owner-only local mirror path that does not rewrite `config.toml`.

## 2. Durable Store And Export

- [x] 2.1 Implement the daemon-owned redb store keyed by authenticated Emby user ID and document kind, with database-format metadata and owner-only filesystem permissions.
- [x] 2.2 Implement one-transaction snapshot reads and independent create-if-absent and expected-revision updates that return either the committed record or the current stale-write winner.
- [x] 2.3 Run store operations through one bounded worker that never holds playback, Player, queue, connection-registry, or socket locks while waiting or committing.
- [x] 2.4 Add local administrative JSON export of user IDs, document kinds, revisions, and parsed values with owner-only output and no credentials.
- [x] 2.5 Verify failed database open and commit paths preserve prior committed data and leave daemon playback operational.

## 3. Secure Shared-Data Service

- [x] 3.1 Implement the dedicated shared-data listener and capability handshake independently of playback ctrl clients, authority, and broadcasts.
- [x] 3.2 Add TLS wrapping for network connections with normal certificate validation and Unix-domain support protected by socket permissions.
- [x] 3.3 Add bounded shared-session authentication that accepts only successful `/Users/Me` validation with a non-empty user ID and never invokes API-key user-list fallback.
- [x] 3.4 Ensure authentication tokens are discarded after validation and excluded from logs, database records, protocol diagnostics, and JSON export.
- [x] 3.5 Implement post-commit acknowledgements and same-user notification fan-out after snapshotting recipients, without holding store or registry locks across network writes.

## 4. Client Synchronization And Fallback

- [ ] 4.1 Implement a shared-state client connection independent of playback routing, with bounded connect/authenticate/restore operations and per-document revision tracking.
- [ ] 4.2 On the initial snapshot, adopt existing shared documents and conditionally initialize absent documents from current local state, adopting the winner of any initialization race.
- [ ] 4.3 Atomically mirror every accepted queue, library-position, reconnect, and roaming-settings document locally while preserving the three existing state schemas.
- [ ] 4.4 Route connected writes through expected-revision updates; after acknowledgement update the local mirror, and on stale rejection adopt and mirror the returned shared winner without retrying the rejected mutation.
- [ ] 4.5 On initial connection, restore, or write failure, persist pending state locally, enter fallback once, and keep browsing and playback operational.
- [ ] 4.6 Add background reconnection with jittered exponential backoff capped at 60 seconds and reset the backoff after an authenticated snapshot.
- [ ] 4.7 On reconnection, replace divergent fallback documents with existing shared documents without prompting, initialize only absent shared documents, then resume shared writes.
- [ ] 4.8 Apply committed same-user notifications only when their revision is newer, then update application state and its local mirror without producing an echo write.

## 5. Settings And User Feedback

- [ ] 5.1 Apply mirrored/shared `auto_reconnect` and `library_routes` above local configuration while active, falling back to ordinary local configuration only when no mirror exists.
- [ ] 5.2 Log each explicitly configured local roaming-setting mismatch once per shared connection while ignoring differences from compiled defaults.
- [ ] 5.3 Add one notification toast per transition into fallback, one after successful reconnection, and one when a stale write is replaced by the current shared document.
- [ ] 5.4 Confirm shared-data connection and notification handling never changes playback authority or subscribes to live playback queue/status events.

## 6. Verification And Operator Documentation

- [ ] 6.1 Verify hosting and client use remain disabled by default and disabled operation creates no database or listener and preserves existing local-only behavior.
- [ ] 6.2 Verify per-user isolation, API-key rejection, invalid-certificate rejection before token transmission, and absence of token material from logs and exports.
- [ ] 6.3 Verify concurrent create and update races produce one committed winner, monotonic independent revisions, stale-writer adoption, and post-commit same-user fan-out.
- [ ] 6.4 Verify startup outage, mid-session write failure, local mirroring, background reconnection, shared-authority restoration, and transition toast behavior.
- [ ] 6.5 Document canonical `mbvd` database/listener/TLS configuration, client endpoint opt-in, certificate trust setup, JSON export, fallback semantics, and rollback by disabling shared state.
- [ ] 6.6 Run the relevant workspace formatting, lint, unit, and integration verification commands and resolve regressions attributable to this change.

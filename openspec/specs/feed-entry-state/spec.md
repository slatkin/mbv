# feed-entry-state Specification

## Purpose
Provide per-user, per-entry feed playback state (resume position and watched flag) as durable roaming state on the existing shared-data transport, stored as independent keyed rows with last-write-wins semantics so unbounded feed entries never share a single revisioned document.
## Requirements
### Requirement: Feed entry state is keyed and per-user

The service SHALL store feed entry playback state as independent rows keyed by `(user_id, feed_id, entry_guid)`, each holding at least `position_ticks` and a `played` flag. Rows SHALL be isolated per authenticated Emby user exactly as the existing shared documents are: a client SHALL access only rows belonging to its own authenticated user ID.

Feed entry state SHALL be stored in its own keyed table, separate from the revisioned shared-documents store, and SHALL NOT be one of the fixed shared-document kinds.

#### Scenario: Round-trip of one entry

- **WHEN** a client writes state for `(user_id, feed_id, entry_guid)` and later reads that same key
- **THEN** the service SHALL return the most recently written `position_ticks` and `played`

#### Scenario: Two users, same feed and entry

- **WHEN** two clients authenticated as different Emby users write the same `(feed_id, entry_guid)`
- **THEN** each client SHALL read back only the value it wrote under its own user ID

### Requirement: Feed entry writes are last-write-wins

Feed entry writes SHALL NOT use optimistic revisions or compare-and-swap. A write SHALL unconditionally replace any existing row for its key. No feed entry operation SHALL require an atomic transaction spanning multiple entries or any shared document.

#### Scenario: Concurrent writes to the same entry

- **WHEN** two writes for the same key are committed in sequence
- **THEN** the later committed write SHALL be the value subsequently read, with no stale-revision rejection

#### Scenario: Write to an absent entry

- **WHEN** a client writes state for a key that has no existing row
- **THEN** the service SHALL create the row from that value without requiring the row to be absent or present

### Requirement: A feed's entries can be scanned by prefix

The service SHALL support reading all stored entry rows for a given `(user_id, feed_id)` prefix in a single operation, returning each entry's `entry_guid`, `position_ticks`, and `played`.

#### Scenario: Prefix scan returns a feed's entries

- **WHEN** a client has written state for several entries under one `(user_id, feed_id)`
- **THEN** a prefix scan for that `(user_id, feed_id)` SHALL return exactly those entries and no entries of other feeds or other users

#### Scenario: Prefix scan of a feed with no state

- **WHEN** a client scans a `(user_id, feed_id)` for which no rows exist
- **THEN** the service SHALL return an empty result rather than an error

### Requirement: Feed entry state is negotiated as an additive capability

Support for feed entry state operations SHALL be advertised as an additive shared-data capability string during the handshake, without changing the shared-data protocol version. A client SHALL use feed entry operations only against a daemon that advertises the capability; against a daemon that does not, the client SHALL treat feed entry state as unavailable and fall back to local behavior without error.

#### Scenario: Daemon advertises the capability

- **WHEN** a client connects to a daemon whose handshake advertises the feed-entry-state capability
- **THEN** the client MAY issue feed entry get, put, and prefix-scan operations

#### Scenario: Daemon lacks the capability

- **WHEN** a client connects to a daemon that does not advertise the feed-entry-state capability
- **THEN** the client SHALL NOT issue feed entry operations and SHALL treat feed entry state as unavailable without reporting a protocol error

### Requirement: Feed entry storage failure is isolated from playback

The daemon SHALL acknowledge a feed entry write only after it is durably committed. Feed entry storage failures SHALL fail the affected operation without stopping daemon playback and without corrupting previously committed rows. When feed entry state is unavailable, browsing and playback SHALL remain available.

#### Scenario: Feed entry commit fails

- **WHEN** durable commit of a feed entry write fails
- **THEN** the service SHALL not acknowledge the write as committed
- **THEN** daemon playback SHALL continue and previously committed rows SHALL remain intact


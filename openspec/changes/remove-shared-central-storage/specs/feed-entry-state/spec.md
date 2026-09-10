## MODIFIED Requirements

### Requirement: Feed entry state is keyed and per-user

The client SHALL store feed entry playback state in a local state file under `state_dir()`, as independent rows keyed by `(user_id, feed_id, entry_guid)`, each holding at least `position_ticks` and a `played` flag. Rows SHALL remain isolated per Emby user: a client SHALL read and write only rows belonging to its own authenticated user ID.

Feed entry state SHALL be stored separately from every other persisted document and SHALL NOT be written into `config.toml`. It SHALL NOT be a revisioned state document and SHALL NOT use compare-and-swap.

#### Scenario: Round-trip of one entry

- **WHEN** a client writes state for `(user_id, feed_id, entry_guid)` and later reads that same key
- **THEN** the client SHALL return the most recently written `position_ticks` and `played`

#### Scenario: Two users, same feed and entry

- **WHEN** two Emby users have state for the same `(feed_id, entry_guid)` on the same machine
- **THEN** each SHALL read back only the value written under its own user ID

#### Scenario: State survives a restart

- **WHEN** a client exits after writing entry state and starts again on the same machine
- **THEN** the stored position and played flag SHALL still be readable

### Requirement: Feed entry writes are last-write-wins

Feed entry writes SHALL NOT use optimistic revisions or compare-and-swap. A write SHALL unconditionally replace any existing row for its key. The client SHALL be the only writer of the state file, and no feed entry operation SHALL require a transaction spanning multiple entries or any other persisted document.

#### Scenario: Concurrent writes to the same entry

- **WHEN** two writes for the same key are stored in sequence
- **THEN** the later write SHALL be the value subsequently read, with no stale-revision rejection

#### Scenario: Write to an absent entry

- **WHEN** a client writes state for a key that has no existing row
- **THEN** the client SHALL create the row from that value without requiring the row to be absent or present

### Requirement: A feed's entries can be scanned by prefix

The client SHALL support reading all stored entry rows for a given `(user_id, feed_id)` prefix in a single operation, returning each entry's `entry_guid`, `position_ticks`, and `played`.

#### Scenario: Prefix scan returns a feed's entries

- **WHEN** a client has written state for several entries under one `(user_id, feed_id)`
- **THEN** a prefix scan for that `(user_id, feed_id)` SHALL return exactly those entries and no entries of other feeds or other users

#### Scenario: Prefix scan of a feed with no state

- **WHEN** a client scans a `(user_id, feed_id)` for which no rows exist
- **THEN** the scan SHALL return an empty result rather than an error

### Requirement: Feed entry storage failure is isolated from playback

A feed entry write SHALL be complete only once the state file has been durably replaced. A failed write SHALL leave the previously written state intact and SHALL NOT be reported as committed.

Feed entry state failures — a missing, unreadable, or invalid state file, or a failed write — SHALL NOT stop playback, block feed browsing, or prevent startup. The client SHALL record the failure and continue with unplayed, zero-position entries.

#### Scenario: Feed entry commit fails

- **WHEN** a durable write of feed entry state fails
- **THEN** the client SHALL not treat the write as complete
- **THEN** previously written rows SHALL remain intact and readable
- **THEN** playback SHALL continue

#### Scenario: State file is unreadable

- **WHEN** the state file is missing, unreadable, or invalid
- **THEN** feed browsing and playback SHALL remain available
- **THEN** fetched entries SHALL be treated as unplayed with zero position

## REMOVED Requirements

### Requirement: Feed entry state is negotiated as an additive capability

**Reason**: There is no shared-data handshake and no peer to negotiate a capability with. The client reads and writes its own state file, so the question of whether the counterparty supports feed entry operations cannot arise.

**Migration**: None required at the client. Behavior previously conditioned on a missing capability — treat entry state as unavailable and fall back without error — is preserved by the local store's failure behavior.

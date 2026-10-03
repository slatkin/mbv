# Spec Delta

## MODIFIED Requirements

### Requirement: Queue persistence round-trips every QueueItem

Persisted queue state SHALL serialize the canonical tagged `QueueItem` sequence and restore every supported item kind in the same order. Persisted items SHALL exclude Service credentials and ephemeral playback state. Persisted Emby and Audiobookshelf items SHALL carry no playback position. Saving SHALL clear it, and restoring state written by an earlier version SHALL also clear it. Feed entries SHALL keep their playback position. Legacy untagged Emby-only state SHALL remain readable.

#### Scenario: Restore a mixed queue

- **WHEN** persisted state contains Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** restoration SHALL preserve each slot's item kind, provider-qualified content identity, and ordering
- **THEN** restored Feed entries SHALL keep their playback position
- **THEN** owner admission SHALL run before restored slots enter a Bound queue

#### Scenario: Provider positions are not persisted

- **WHEN** a queue holding an Emby video at 20 minutes and an Audiobookshelf episode at 5 minutes is persisted and restored
- **THEN** both restored items SHALL have no playback position

#### Scenario: Earlier state with provider positions

- **WHEN** persisted state written by an earlier version carries a playback position on an Emby item
- **THEN** the restored item SHALL have no playback position

#### Scenario: Restore legacy state

- **WHEN** persisted state contains the legacy untagged Emby-item shape
- **THEN** restoration SHALL interpret those values as Emby queue items without error

#### Scenario: Inspect persisted Audiobookshelf item

- **WHEN** an Audiobookshelf podcast episode is persisted
- **THEN** its representation SHALL contain no Service credential, playback `sessionId`, resolved URL, or request header

### Requirement: A Player owner refreshes progress on a cold-adopted persisted queue

When a Player owner (Local daemon or packaged `mbvd`) builds its queue from persisted state, either by adopting a client's persisted queue snapshot while it has no queue of its own or by restoring its own Stay-alive queue at startup, it SHALL asynchronously refresh progress for the queue's Emby items from Emby. The refresh SHALL NOT block playback of the queue. It SHALL apply the refreshed values to the owner's own canonical queue, not only to a client-side snapshot, and SHALL broadcast the refreshed queue to attached clients once applied. A refreshed inactive slot SHALL take Emby's position and watched state as returned.

#### Scenario: Cold daemon adopts a persisted queue with server-side progress

- **WHEN** a Local daemon with no existing queue receives an adoption request
  carrying a persisted snapshot whose items have resume progress on the
  owning Service
- **THEN** the daemon fetches current progress for those items from the
  Service asynchronously, merges it into its own canonical queue, and
  broadcasts the refreshed queue to attached clients — without requiring any
  item to be played first

#### Scenario: Stay-alive process restores its queue at startup

- **WHEN** the Stay-alive process starts and restores its persisted queue containing Emby items
- **THEN** it fetches current progress for those items from Emby asynchronously and broadcasts the refreshed queue once applied

#### Scenario: Adopted item is played before the refresh completes

- **WHEN** the user plays an item from the adopted queue and the refresh result for that slot arrives while it is playing
- **THEN** the slot SHALL keep its live playback position

#### Scenario: Refresh is scoped to the owning Service

- **WHEN** the adopted queue contains items from more than one Service, or
  Feed entries
- **THEN** the refresh updates or prunes only the slots belonging to the
  Service it queried, and leaves every other slot's progress untouched

#### Scenario: Refresh does not regress adopted positions

- **WHEN** the refresh returns a position for an inactive slot that is lower than the position the slot currently shows
- **THEN** the slot SHALL take the fetched position; a lower server position is not a regression, because Emby's position is the only resume source for Emby items

### Requirement: A slot resumes the same however it is reached

The resume position used when an occurrence starts SHALL follow the `playback-resume` source rule for its medium, and SHALL be the same whether the occurrence is reached by Next, Previous, a direct jump, an on-screen Next-Up accept, or the initial queue load. This applies to full-playlist and active-file playback alike. Emby video items SHALL resume from the position Emby reports when the start position is decided. Audiobookshelf items SHALL resume from the Audiobookshelf playback session's position. Feed entries SHALL resume from the canonical queue's recorded position.

#### Scenario: Previous back to a video left under 30 seconds

- **WHEN** an Emby video occurrence is played for 12 seconds, playback moves to the next slot, and the user invokes Previous
- **THEN** the video SHALL resume from the position Emby reports for it at that moment, the same position a direct jump to that slot would use

#### Scenario: Previous back to an audio track

- **WHEN** an audio occurrence is left mid-track by Next and the user invokes Previous
- **THEN** it SHALL start from the same position a direct jump to that slot would use

#### Scenario: Non-Audiobookshelf item in an active-file queue

- **WHEN** a queue plays in active-file mode and the user jumps to a non-Audiobookshelf occurrence
- **THEN** an Emby video occurrence SHALL resume from the position Emby reports for it, and a feed entry SHALL resume from the canonical queue's position for it

## REMOVED Requirements

### Requirement: An accepted stop report protects reported progress until the server confirms it

**Reason**: Emby resume positions are now always read from Emby, so a locally recorded position is never used to resume and needs no protection from refreshes.

**Migration**: None. The positions queue copies record for Emby slots are display state that the next refresh replaces. The `progress_report_accepted` ctrl field is dropped; it was optional on the wire, so peers that still send it are unaffected.

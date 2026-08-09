# feed-queue-item Specification

## Purpose
The playback queue is a play mechanism that can hold and play either an Emby library item or a feed entry, choosing the correct play path by item type. Library rendering remains Emby-only. Remote (ctrl) playback carries feed entries as live, capability-gated queue state.
## Requirements
### Requirement: A queue slot holds either an Emby item or a feed entry

Each playback-queue slot SHALL carry exactly one of two item kinds: an Emby library item or a feed entry. The queue SHALL treat a slot uniformly for ordering, selection, and playback regardless of which kind it holds.

#### Scenario: Emby item in a slot

- **WHEN** a slot holds an Emby item
- **THEN** the queue SHALL expose its title, duration, media kind, and artwork through the same accessors used for any slot

#### Scenario: Feed entry in a slot

- **WHEN** a slot holds a feed entry
- **THEN** the queue SHALL expose the entry's title, its duration if known (else none), its media kind, and no artwork, through the same accessors

### Requirement: A feed entry carries only identity and playback fields

A feed entry in the queue SHALL carry the fields needed to identify and play it: a stable identifier, a title, an enclosure URL when present, a link URL when present, a MIME type when present, and a duration in ticks when known. It SHALL NOT carry playback-progress state (position or played/watched) — that state is out of scope for this capability.

#### Scenario: Entry with an enclosure

- **WHEN** a feed entry has an enclosure URL
- **THEN** that URL SHALL be available as the entry's primary playable source

#### Scenario: Entry without an enclosure

- **WHEN** a feed entry has no enclosure URL but has a link
- **THEN** the link SHALL be available as the fallback playable source

### Requirement: Playback selects the play path by item kind

At the play boundary the system SHALL choose the play path from the slot's item kind. An Emby item SHALL play through the existing Emby streaming-URL path, unchanged. A feed entry SHALL play by handing its enclosure URL — or its link when there is no enclosure — to the player directly.

#### Scenario: Play an Emby item

- **WHEN** a slot holding an Emby item is played
- **THEN** the item SHALL play via the existing Emby streaming path with no change in behavior

#### Scenario: Play a feed entry with an enclosure

- **WHEN** a slot holding a feed entry with an enclosure URL is played
- **THEN** the enclosure URL SHALL be handed to the player directly

#### Scenario: Play a feed entry without an enclosure

- **WHEN** a slot holding a feed entry with no enclosure URL but a link is played
- **THEN** the link SHALL be handed to the player directly

### Requirement: Queue persistence reads legacy state and writes the tagged shape

Saved queue state SHALL round-trip through a tagged item shape that distinguishes Emby items from feed entries. Loading SHALL remain backward-compatible: queue state written before this change (bare Emby items, untagged) SHALL load as Emby slots. Saving SHALL always write the tagged shape.

#### Scenario: Load legacy queue state

- **WHEN** queue state written before this change (untagged bare items) is loaded
- **THEN** each item SHALL be interpreted as an Emby slot and the queue SHALL restore without error

#### Scenario: Round-trip tagged state

- **WHEN** a queue containing both Emby items and feed entries is saved and then reloaded
- **THEN** the reloaded queue SHALL contain the same slots, each with its original item kind preserved

### Requirement: Feed items cross the ctrl boundary as capability-gated state

Feed playback over the ctrl protocol SHALL be gated on an additive `feed-playback` capability; a peer that does not advertise it SHALL receive Emby items only, exactly as before. `CTRL_PROTOCOL_VERSION` SHALL NOT change.

Between capable peers, the atomic ctrl state snapshot SHALL carry feed entries in a `feed_items` tail field that defaults to empty for absent or legacy senders. A mixed queue SHALL be ordered as Emby items followed by feed items, so a feed entry's position is `emby_items.len() + n` and no absolute mixed-queue indices are transmitted. While any feed entry is present, the daemon SHALL reject Emby queue mutations that would break the Emby-then-feed tail invariant (append, move, replace, adopt); only mutations preserving it remain available. Adoption is rejected because it would discard live Feed state without a corresponding Feed-removal event. Feed-entry additions and removals SHALL be reflected atomically in the next state snapshot and in the reconnect snapshot.

#### Scenario: Capable peer receives a mixed queue

- **WHEN** a queue of Emby items followed by feed entries is synchronized to a peer advertising `feed-playback`
- **THEN** the Emby items SHALL be transmitted in their existing wire shape
- **AND** the feed entries SHALL be carried in the `feed_items` tail of the same atomic snapshot
- **AND** the peer SHALL reconstruct a slot-identical mixed queue from the two fields

#### Scenario: Legacy peer receives a mixed queue

- **WHEN** a queue containing feed entries is synchronized to a peer that does not advertise `feed-playback`
- **THEN** only the Emby items SHALL be transmitted in their existing wire shape
- **AND** the feed entries SHALL be omitted

#### Scenario: Emby mutation that breaks the tail invariant is rejected

- **WHEN** feed entries are present and an Emby queue mutation would place an Emby item after a feed entry
- **THEN** the daemon SHALL reject the mutation

#### Scenario: Queue adoption with live Feed state is rejected

- **WHEN** feed entries are present and a peer requests adoption of a replacement Emby queue
- **THEN** the daemon SHALL reject the request rather than silently discard the live Feed tail

### Requirement: Capability-gated Feed playback reaches a Player owner

The ctrl protocol SHALL advertise an additive `feed-playback` capability. A peer supporting that capability SHALL accept a `LoadFeed` command carrying one FeedEntry and append/play it through the Player owner's Feed play path. The resulting live Feed state SHALL follow the capability-gated ctrl-state requirement. The protocol version SHALL not change.

#### Scenario: Capability-supporting peer plays a Feed entry

- **WHEN** a peer advertises `feed-playback` and receives `LoadFeed` for a Feed entry with a playable source
- **THEN** the Player owner SHALL append the Feed entry and begin playback

#### Scenario: Capable peer has no Player owner

- **WHEN** a peer advertising `feed-playback` receives `LoadFeed` but no Player owner is available
- **THEN** the daemon SHALL reject the command and SHALL NOT add the Feed entry to its live queue state

#### Scenario: Peer lacks Feed-playback capability

- **WHEN** a controlling client attempts Feed playback through a peer that does not advertise `feed-playback`
- **THEN** the command SHALL not be sent or interpreted as another command

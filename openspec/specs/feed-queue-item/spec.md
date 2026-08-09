# feed-queue-item Specification

## Purpose
The playback queue is a play mechanism that can hold and play either an Emby library item or a feed entry, choosing the correct play path by item type, while remote (ctrl) playback and library rendering remain Emby-only.
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

- **WHEN** a queue is saved and then reloaded
- **THEN** the reloaded queue SHALL contain the same slots, each with its original item kind preserved

### Requirement: Feed items do not cross the ctrl boundary

Feed playback SHALL be local-player only for this capability. When queue items are sent over the ctrl protocol to a remote peer, feed entries SHALL be omitted and only Emby items SHALL be transmitted. The ctrl wire shape SHALL remain unchanged (no new capability string, no version bump).

#### Scenario: Queue with a feed entry is sent over ctrl

- **WHEN** a queue containing both Emby items and feed entries is transmitted over the ctrl protocol
- **THEN** the Emby items SHALL be transmitted in their existing wire shape and the feed entries SHALL be omitted


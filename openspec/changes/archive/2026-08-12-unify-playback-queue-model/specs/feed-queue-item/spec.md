## MODIFIED Requirements

### Requirement: A queue slot holds either an Emby item or a feed entry

Each playback-queue slot SHALL carry exactly one of two item kinds: an Emby library item or a feed entry. The queue SHALL treat a slot uniformly for ordering, selection, mutation, persistence, synchronization, and playback regardless of which kind it holds. Neither kind SHALL be stored in a parallel prefix or tail outside the canonical slot sequence.

#### Scenario: Emby item in a slot

- **WHEN** a slot holds an Emby item
- **THEN** the queue SHALL expose its title, duration, media kind, and artwork through the same accessors used for any slot

#### Scenario: Feed entry in a slot

- **WHEN** a slot holds a feed entry
- **THEN** the queue SHALL expose the entry's title, its duration if known (else none), its canonical media kind, and no artwork, through the same accessors
- **AND** a recognized enclosure MIME type SHALL refine that media kind
- **AND** the subscription's stored `FeedKind` SHALL remain canonical when MIME is absent or unrecognized

#### Scenario: Mixed item order

- **WHEN** Emby items and Feed entries occupy arbitrary positions in one queue
- **THEN** every queue consumer SHALL preserve that slot order without projecting either kind into a separate collection

### Requirement: Playback selects source resolution by item kind

At the source-resolution boundary the system SHALL choose the media source from the slot's item kind while preserving one shared playback lifecycle. An Emby item SHALL resolve through the existing authenticated Emby streaming-URL path. A feed entry SHALL resolve to its enclosure URL, or its link when there is no enclosure. Item kind SHALL NOT select a separate append, queue-state, Player-lifecycle, or mpv-loading path.

#### Scenario: Play an Emby item

- **WHEN** a slot holding an Emby item is played
- **THEN** the item SHALL resolve through the authenticated Emby streaming path
- **AND** SHALL enter the same Player lifecycle used by any queue slot

#### Scenario: Play a feed entry with an enclosure

- **WHEN** a slot holding a feed entry with an enclosure URL is played
- **THEN** the enclosure URL SHALL be handed to the player directly through the shared playback lifecycle

#### Scenario: Play a feed entry without an enclosure

- **WHEN** a slot holding a feed entry with no enclosure URL but a link is played
- **THEN** the link SHALL be handed to the player directly through the shared playback lifecycle

## REMOVED Requirements

### Requirement: Feed items cross the ctrl boundary as capability-gated state

**Reason**: The `items` plus `feed_items` tail representation creates two coordinate systems, forbids ordinary mixed-queue mutations, and requires every consumer to reconstruct queue truth. It contradicts the canonical `PlaybackQueue<QueueItem>` model.

**Migration**: Capable peers exchange one ordered queue-slot sequence under the unified queue capability. Legacy Emby-only fields may remain at the ctrl compatibility boundary for peers lacking that capability, but no Player owner, daemon, or TUI queue stores a Feed tail.

### Requirement: Capability-gated Feed playback reaches a Player owner

**Reason**: `LoadFeed` conflates append and play, duplicates existing slots when replayed, and creates a Feed-specific Player lifecycle that has already diverged across bare, cold-daemon, and remote playback.

**Migration**: Current clients use item-generic append, replace, and play-existing-slot operations. A decode-only compatibility handler may translate legacy `LoadFeed` from an older capable peer into the shared queue-submission boundary; it SHALL NOT maintain a Feed-only queue or lifecycle path.

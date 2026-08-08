## Purpose

Store feed subscriptions and entry playback state in the mbvd-hosted shared store, making feeds roam across machines.

## ADDED Requirements

### Requirement: Feeds require shared-store connectivity

The Feeds feature SHALL be available only when the client has an active shared-store connection to mbvd. When the shared store is unreachable, feeds SHALL be unavailable — the Feeds tab SHALL NOT appear and feed operations SHALL NOT be attempted.

#### Scenario: Shared store connected

- **WHEN** the client establishes a shared-store connection
- **THEN** the Feeds feature SHALL become available
- **THEN** feed subscriptions and entry state SHALL be loaded from the shared store

#### Scenario: Shared store unreachable

- **WHEN** the client cannot connect to the shared store
- **THEN** the Feeds tab SHALL NOT appear in the tab bar
- **THEN** no feed operations SHALL be attempted

#### Scenario: Shared store disconnects mid-session

- **WHEN** an active shared-store connection is lost
- **THEN** the Feeds tab SHALL be removed from the UI
- **THEN** any in-progress feed playback SHALL continue but progress SHALL NOT be persisted

### Requirement: Feed subscriptions persist per user

Each Emby user SHALL have an independent set of feed subscriptions stored in the shared store. A subscription SHALL contain at minimum: feed URL, user-assigned title (optional override), feed kind (audio or video), and creation timestamp.

#### Scenario: User adds a feed

- **WHEN** a user subscribes to a feed URL
- **THEN** the subscription SHALL be stored in the shared store under that user's scope
- **THEN** other machines connected to the same shared store SHALL see the subscription

#### Scenario: Two users subscribe to the same URL

- **WHEN** two Emby users subscribe to the same feed URL
- **THEN** each user SHALL have an independent subscription record
- **THEN** each user's entry state SHALL be isolated from the other

### Requirement: Feed kind is per-feed, not per-entry

Each feed subscription SHALL have exactly one kind: audio or video. The kind SHALL be inferred from enclosure MIME types on first fetch and MAY be overridden by the user. Mixed audio/video within a single feed is not supported.

#### Scenario: Feed contains audio enclosures

- **WHEN** a feed's first fetch reveals audio MIME types (audio/*)
- **THEN** the feed kind SHALL default to audio

#### Scenario: User overrides inferred kind

- **WHEN** a user changes a feed's kind from audio to video
- **THEN** subsequent playback SHALL treat entries as video items

### Requirement: Entry identity uses guid with fallback

Each feed entry SHALL be identified by a stable key for position tracking. The key SHALL be the entry's guid when present. When guid is absent, the key SHALL fall back to enclosure URL, then to a hash of title and publication date.

#### Scenario: Entry has guid

- **WHEN** a feed entry contains a guid element
- **THEN** position lookups SHALL use that guid as the key

#### Scenario: Entry lacks guid but has enclosure

- **WHEN** a feed entry has no guid but has an enclosure URL
- **THEN** position lookups SHALL use the enclosure URL as the key

#### Scenario: Entry changes enclosure URL but retains guid

- **WHEN** a feed re-publishes an entry with the same guid but different enclosure URL
- **THEN** the entry's playback position SHALL be preserved

### Requirement: Entry playback state persists per entry

For each entry the user has interacted with, the shared store SHALL persist: playback position (ticks), watched status (boolean), and last-played timestamp. State SHALL be keyed by user, feed URL, and entry identity key.

#### Scenario: User partially plays an entry

- **WHEN** a user stops playback at position 5:00 of an entry
- **THEN** the position SHALL be stored in the shared store
- **THEN** resuming on another machine SHALL start at 5:00

#### Scenario: User marks entry as watched

- **WHEN** playback reaches completion threshold or user toggles watched
- **THEN** watched status SHALL be updated in the shared store

### Requirement: Feed metadata is fetched, not stored

Feed metadata (title, description, entry list) SHALL be fetched from the feed URL at runtime, not persisted in the shared store. Only subscriptions and entry playback state are stored.

#### Scenario: Feed updates its entries

- **WHEN** a feed publishes new entries between client sessions
- **THEN** the client SHALL see new entries on refresh
- **THEN** playback state for existing entries SHALL be preserved by entry key

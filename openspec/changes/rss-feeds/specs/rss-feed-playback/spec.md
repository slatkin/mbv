## Purpose

Enable feed entries to be queued and played like Emby items, with progress reporting to the shared store.

## ADDED Requirements

### Requirement: Feed entries queue alongside Emby items

Feed entries SHALL be queueable through the same mechanisms as Emby items: play, enqueue, play next. The queue SHALL support mixed Emby and feed items.

#### Scenario: Play feed entry

- **WHEN** the user plays a feed entry
- **THEN** the entry SHALL be added to the queue and playback SHALL start
- **THEN** the enclosure URL SHALL be passed to mpv

#### Scenario: Enqueue feed entry

- **WHEN** the user enqueues a feed entry
- **THEN** it SHALL be appended to the current queue

#### Scenario: Mixed queue

- **WHEN** the queue contains both Emby items and feed entries
- **THEN** playback SHALL proceed through both seamlessly

### Requirement: Audio-only owner respects feed kind

When directly controlling an audio-only Player owner, feed entries with video kind SHALL fall through to the local queue, following the same fall-through behavior as Emby video items.

#### Scenario: Video feed entry to audio-only owner

- **WHEN** the user plays a video-kind feed entry while controlling an audio-only owner
- **THEN** the entry SHALL fall through to the local queue

#### Scenario: Audio feed entry to audio-only owner

- **WHEN** the user plays an audio-kind feed entry while controlling an audio-only owner
- **THEN** the entry SHALL be accepted by that owner's queue

### Requirement: Progress reports to shared store

Playback progress for feed entries SHALL be reported to the shared store using the same cadence as Emby progress reporting. The existing player lifecycle (started, progress, stopped) SHALL route to the shared store instead of Emby for feed entries.

#### Scenario: Progress during playback

- **WHEN** playback of a feed entry is in progress
- **THEN** position SHALL be reported to the shared store at regular intervals

#### Scenario: Playback stopped

- **WHEN** playback of a feed entry stops
- **THEN** final position SHALL be written to the shared store

#### Scenario: Playback completes

- **WHEN** a feed entry reaches completion threshold
- **THEN** watched status SHALL be set to true in the shared store

### Requirement: Feed entry URLs resolve directly

Feed entry playback URLs SHALL be the enclosure URL from the feed. No Emby stream resolution or transcoding applies to feed entries.

#### Scenario: Entry has enclosure

- **WHEN** a feed entry with an enclosure URL is played
- **THEN** mpv SHALL receive the enclosure URL directly

#### Scenario: Entry lacks enclosure

- **WHEN** a feed entry has no enclosure URL
- **THEN** it SHALL NOT be playable
- **THEN** attempting to play it SHALL show an error

### Requirement: Queue serialization includes feed entries

Queue persistence SHALL support feed entries alongside Emby items. The serialized form SHALL carry enough information to reconstruct feed entries without re-fetching the feed (entry key, title, enclosure URL, duration, feed URL).

#### Scenario: Queue saved with feed entries

- **WHEN** the queue containing feed entries is persisted
- **THEN** feed entry data SHALL be included in the serialization
- **THEN** restoring SHALL not require network access for entry metadata

#### Scenario: Old clients encounter feed entries

- **WHEN** an old client without feed support reads a queue containing feed entries
- **THEN** it SHALL gracefully ignore or skip entries it cannot deserialize

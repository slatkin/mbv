## ADDED Requirements

### Requirement: User-initiated queue replacements confirm before replacing a populated queue
When the user explicitly picks content to replace the playback queue, the system SHALL ask for confirmation before replacing that queue if it is populated. An empty target queue SHALL be replaced without the prompt. This applies to album and artist track plays, playlist loads, shuffle-folder plays, context-menu Play and Shuffle, and grouped-tree track plays. It SHALL NOT apply to library autoplay, single-item play, or replays that restore existing session state. Cancelling SHALL leave the queue and playback unchanged. After confirmation, the replacement SHALL behave exactly as it did before this gate existed, including any later "play locally instead" or unsaved-playlist prompt.

#### Scenario: Album track play over a populated queue
- **WHEN** the queue is populated and the user plays a track from an album
- **THEN** confirmation is requested before the queue is replaced
- **AND** cancelling leaves the queue and playback unchanged

#### Scenario: Playlist load over a populated queue
- **WHEN** the queue is populated and the user loads a playlist
- **THEN** confirmation is requested before the playlist replaces the queue

#### Scenario: Empty queue plays immediately
- **WHEN** the queue is empty and the user plays an album track, shuffles a folder, or loads a playlist
- **THEN** no replace-queue confirmation is shown

#### Scenario: Confirmation precedes later prompts
- **WHEN** the queue is populated and the chosen items cannot play on the attached owner
- **THEN** the replace-queue confirmation is shown first
- **AND** only after confirming is the "play locally instead" prompt shown

#### Scenario: Autoplay and single-item play are not gated
- **WHEN** the queue is populated and library autoplay starts, or the user plays a single movie or episode
- **THEN** no replace-queue confirmation is shown

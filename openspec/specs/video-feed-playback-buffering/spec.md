# video-feed-playback-buffering Specification

## Purpose
This capability keeps network-backed video feed playback supplied through short
throughput dips by retaining a larger playback cache window without changing
feed URL resolution or media format selection.
## Requirements
### Requirement: Network video feed playback uses configurable video cache budgets

When a Player owner plays a network-backed video feed entry with a video window,
the playback cache SHALL use the configured video forward and retained/back
budgets. When either setting is omitted or invalid, its current default of
50 MiB forward or 100 MiB retained/back SHALL apply independently. Headless runs
SHALL instead use the fixed audio-sized cache budget defined by
`headless-playback-memory-footprint`.

#### Scenario: High-quality video feed tolerates a short throughput dip

- **WHEN** a video feed entry is played at a high bitrate with a video window
  using default configuration and the source throughput temporarily falls below
  the playback bitrate
- **THEN** playback SHALL have the 100 MiB retained/back cache budget and 50 MiB
  forward budget available before entering repeated buffering

#### Scenario: User configures video cache budgets

- **WHEN** playback with a video window starts with valid non-default forward and
  retained/back video cache budgets
- **THEN** the playback cache SHALL use both configured budgets

#### Scenario: One video cache setting is invalid

- **WHEN** one video cache setting is valid and the other is omitted or invalid
- **THEN** the valid setting SHALL apply and only the omitted or invalid setting
  SHALL use its default

#### Scenario: Normal video feed playback starts

- **WHEN** a video feed entry is loaded through the normal feed play path with a
  video window
- **THEN** the player SHALL use the configured video cache policy without
  changing the feed's resolved source URL or selected format

#### Scenario: Headless run does not reserve the video retained cache

- **WHEN** a headless playback run starts while non-default video cache budgets
  are configured
- **THEN** it SHALL use the fixed audio-sized cache budget and SHALL NOT change
  source resolution or format selection

### Requirement: Buffering policy preserves mixed-queue playback behavior

The buffering policy SHALL apply consistently to a playback run containing both
Emby items and feed entries. Changing the current queue item kind SHALL NOT
alter queue ordering, submission destination, or the URL-resolution path for
either item kind.

#### Scenario: Feed entry follows an Emby item

- **WHEN** playback transitions from an Emby item to a video feed entry in the
  same queue
- **THEN** the feed entry SHALL play with the retained-cache policy and the
  existing direct feed-source path

#### Scenario: Emby item follows a feed entry

- **WHEN** playback transitions from a video feed entry to an Emby item in the
  same queue
- **THEN** the Emby item SHALL continue using the existing Emby streaming path
  without feed-specific URL handling


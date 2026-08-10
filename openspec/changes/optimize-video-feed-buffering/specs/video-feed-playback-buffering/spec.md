## Purpose

This capability keeps network-backed video feed playback supplied through short
throughput dips by retaining a larger playback cache window without changing
feed URL resolution or media format selection.

## ADDED Requirements

### Requirement: Network video feed playback uses a 100MiB retained cache window

When a Player owner plays a network-backed video feed entry, the playback cache
SHALL be configured to retain up to 100MiB of previously demuxed data. The
existing 50MiB forward buffering limit SHALL remain unchanged.

#### Scenario: High-quality video feed tolerates a short throughput dip

- **WHEN** a video feed entry is played at a high bitrate and the source
  throughput temporarily falls below the playback bitrate
- **THEN** playback SHALL have the 100MiB retained cache budget available before
  entering repeated buffering

#### Scenario: Normal video feed playback starts

- **WHEN** a video feed entry is loaded through the normal feed play path
- **THEN** the player SHALL use the configured retained-cache policy without
  changing the feed's resolved source URL or selected format

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

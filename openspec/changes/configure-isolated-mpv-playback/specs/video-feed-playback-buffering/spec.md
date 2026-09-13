## RENAMED Requirements

- FROM: `Network video feed playback uses a 100MiB retained cache window`
- TO: `Network video feed playback uses configurable video cache budgets`

## MODIFIED Requirements

### Requirement: Network video feed playback uses a 100MiB retained cache window

When a Player owner plays a network-backed video feed entry with a video window, the playback cache SHALL use the configured video forward and retained/back budgets. When either setting is omitted or invalid, its current default of 50 MiB forward or 100 MiB retained/back SHALL apply independently. Headless runs SHALL instead use the fixed audio-sized cache budget defined by `headless-playback-memory-footprint`.

#### Scenario: High-quality video feed tolerates a short throughput dip

- **WHEN** a video feed entry is played at a high bitrate with a video window using default configuration and the source throughput temporarily falls below the playback bitrate
- **THEN** playback SHALL have the 100 MiB retained/back cache budget and 50 MiB forward budget available before entering repeated buffering

#### Scenario: User configures video cache budgets

- **WHEN** playback with a video window starts with valid non-default forward and retained/back video cache budgets
- **THEN** the playback cache SHALL use both configured budgets

#### Scenario: One video cache setting is invalid

- **WHEN** one video cache setting is valid and the other is omitted or invalid
- **THEN** the valid setting SHALL apply and only the omitted or invalid setting SHALL use its default

#### Scenario: Normal video feed playback starts

- **WHEN** a video feed entry is loaded through the normal feed play path with a video window
- **THEN** the player SHALL use the configured video cache policy without changing the feed's resolved source URL or selected format

#### Scenario: Headless run does not reserve the video retained cache

- **WHEN** a headless playback run starts while non-default video cache budgets are configured
- **THEN** it SHALL use the fixed audio-sized cache budget and SHALL NOT change source resolution or format selection

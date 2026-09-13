# mpv-playback-policy Specification

## Purpose
Defines the safe default policy for embedded mpv and the precedence between mbv-owned playback settings and a user's complete mpv configuration.
## Requirements
### Requirement: Isolated video playback SHALL request safe automatic hardware decoding

When a Player owner starts playback with a video window while `use_mpv_config` is disabled, mbv SHALL request mpv's `auto-safe` hardware-decoding policy. Hardware-decoder unavailability or failure SHALL fall back to software decoding rather than preventing playback.

#### Scenario: Compatible hardware decoder is available
- **WHEN** video-window playback starts with `use_mpv_config` disabled and mpv finds a compatible safe hardware decoder
- **THEN** mpv SHALL be allowed to use that hardware decoder

#### Scenario: Hardware decoding is unavailable
- **WHEN** video-window playback starts with `use_mpv_config` disabled and no safe hardware decoder can decode the content
- **THEN** playback SHALL continue using software decoding

#### Scenario: Headless playback starts
- **WHEN** a Player owner starts a headless playback run
- **THEN** mbv SHALL NOT enable video hardware decoding for that run

### Requirement: Full user mpv configuration SHALL control hardware-decoding choice

When `use_mpv_config` is enabled, mbv SHALL NOT impose its `auto-safe` hardware-decoding default. The hardware-decoding value resolved from the user's mpv configuration, including mpv's own default when the user specifies none, SHALL remain effective.

#### Scenario: User selects a hardware decoder
- **WHEN** `use_mpv_config` is enabled and the user's mpv configuration selects a hardware-decoding policy
- **THEN** video playback SHALL use that policy instead of mbv's isolated-path default

#### Scenario: User does not configure hardware decoding
- **WHEN** `use_mpv_config` is enabled and the user's mpv configuration does not set hardware decoding
- **THEN** mbv SHALL leave hardware-decoding selection to mpv

### Requirement: Playback-class cache ownership SHALL remain distinct

Headless playback SHALL keep its fixed audio-sized cache policy. Video cache configuration SHALL apply only to playback with a video window and SHALL NOT enlarge or otherwise alter a headless run's cache budget.

#### Scenario: Configured video cache exists during headless playback
- **WHEN** a headless playback run starts while non-default video cache values are configured
- **THEN** the run SHALL still use the fixed headless cache budget

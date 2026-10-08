## ADDED Requirements

### Requirement: Audio-track cycling and mute are separate actions

Audio-track cycling and mute SHALL be two declared actions, `cycle_audio` and `toggle_mute`. No declared action SHALL choose between muting and cycling based on the playing item. The default chord of `cycle_audio` SHALL be `#`, matching mpv's default cycle-audio key. The default chord of `toggle_mute` SHALL stay `m`. No declared action SHALL have `a` as a default chord, so with the default configuration `a` SHALL reach the focused leaf in every playback state.

#### Scenario: `#` cycles the audio track

- **WHEN** playback is active with the default configuration and the user presses `#`
- **THEN** the current playback target's audio-track cycle runs, for audio and video items alike
- **AND** no mute-only toggle runs in its place

#### Scenario: `a` reaches the focused leaf during playback

- **WHEN** playback is active with the default configuration and the user presses `a` while the Music tree has focus
- **THEN** the router does not run a playback command
- **AND** the Music tree's own `a` action runs

#### Scenario: The removed action identifier is unknown

- **WHEN** the config assigns a chord to `toggle_mute_or_cycle_audio`
- **THEN** configuration fails to load because no such action is declared

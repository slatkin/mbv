## Purpose

Ensures playback policy and media-selection preferences remain owned by the controlling mbv client across local and direct-daemon playback.

## ADDED Requirements

### Requirement: Client owns automatic episode expansion

The controlling mbv client SHALL apply its `always_play_next` preference before dispatching playback. When enabled for an episode with available following episodes, mbv SHALL submit the expanded ordered sequence to the selected playback destination; the destination SHALL execute that sequence without independently fetching or appending episodes.

#### Scenario: Direct-daemon episode playback expands at the client
- **WHEN** an mbv client with `always_play_next` enabled starts an episode on direct `mbvd`
- **THEN** mbv submits the resolved episode sequence to `mbvd`
- **THEN** `mbvd` does not fetch or append additional episodes

#### Scenario: Automatic episode expansion is disabled
- **WHEN** an mbv client with `always_play_next` disabled starts an episode
- **THEN** the selected playback destination receives only the explicitly requested item or sequence

### Requirement: Client owns automatic intro skipping

Playback execution SHALL report an intro-start boundary without deciding whether to seek. The controlling mbv client SHALL seek to the reported intro end and dismiss the intro affordance when its `always_skip_intro` preference is enabled; otherwise it SHALL retain the existing skip-intro prompt flow.

#### Scenario: Client automatically skips a reported intro
- **WHEN** the active playback target reports an intro start to an mbv client with `always_skip_intro` enabled
- **THEN** mbv seeks the target to the reported intro end and dismisses the intro affordance

#### Scenario: Client presents the manual intro prompt
- **WHEN** the active playback target reports an intro start to an mbv client with `always_skip_intro` disabled
- **THEN** mbv retains the existing prompt and explicit user skip action

### Requirement: Daemon ignores daemon-host client preferences

Packaged `mbvd` and the hidden local-daemon entrypoint SHALL NOT load or apply daemon-host values for `always_play_next`, `always_skip_intro`, `subtitle_mode`, `subtitle_lang`, or `audio_lang` to playback behavior.

#### Scenario: Daemon host preferences conflict with the controlling client
- **WHEN** daemon-host configuration specifies any client playback preference that differs from the connected mbv client's preference
- **THEN** direct-daemon playback follows the controlling client's submitted preferences
- **THEN** the daemon-host value does not affect playback policy or track selection

### Requirement: Client controls track-selection preferences

The controlling mbv client SHALL use its current subtitle mode, subtitle language, and audio language preferences for local playback and SHALL provide those preferences to direct `mbvd` before daemon playback uses track selection. Changing a preference during direct-daemon playback SHALL update the active daemon target.

#### Scenario: Direct daemon receives preferences before playback
- **WHEN** mbv starts or attaches to direct `mbvd` playback
- **THEN** the daemon uses the controlling client's current subtitle and audio-language preferences for subsequent track selection

#### Scenario: Client changes a preference during direct playback
- **WHEN** a user changes subtitle mode, subtitle language, or audio language while controlling direct `mbvd`
- **THEN** mbv sends the updated preference to that daemon target

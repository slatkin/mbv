## ADDED Requirements

### Requirement: Client preference synchronization for direct playback

The ctrl protocol SHALL support synchronization of controlling-client subtitle mode, subtitle language, and audio language preferences to a direct daemon. The daemon SHALL apply the most recently received client preferences to track selection and SHALL not substitute daemon-host configuration values.

#### Scenario: Supported peer receives client preferences
- **WHEN** a compatible mbv client establishes direct playback control with `mbvd`
- **THEN** the client can synchronize its subtitle mode, subtitle language, and audio language preferences before playback begins

#### Scenario: Preference update is ordered before playback
- **WHEN** a client updates track-selection preferences and then dispatches direct-daemon playback on the same control connection
- **THEN** the daemon applies the updated preferences to that playback request

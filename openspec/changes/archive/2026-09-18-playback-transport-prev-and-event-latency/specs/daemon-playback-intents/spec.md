# daemon-playback-intents

## ADDED Requirements

### Requirement: Player-side mpv events are observed within a bounded latency

The player run SHALL observe mpv-originated events — mpv's own playlist navigation, file end, and
observed property changes — within a fixed ceiling, and that ceiling SHALL NOT depend on a wakeup
notification being delivered for every event. A wakeup notification that arrives before its event is
observable SHALL cost at most the ceiling rather than the run's whole idle wait.

#### Scenario: mpv-initiated navigation is observed promptly

- **WHEN** mpv changes the playing entry itself, as an mpv-window previous or next command does
- **THEN** the run observes the change within the ceiling and its reporting follows the new entry,
  instead of waiting out a multi-second idle deadline

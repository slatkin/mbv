## ADDED Requirements

### Requirement: Music grouping metadata is warmed at startup

For a configured music library, the system SHALL begin resolving the artist
metadata used for album grouping in the background once its Service is
connected, without waiting for a grouped music view to be opened. Warm-up
SHALL NOT delay application startup or the availability of any other Service
or feature, and a warm-up failure SHALL leave ordinary grouped browsing
usable through the existing settle-and-fallback behavior.

#### Scenario: Warm-up begins after the Service connects

- **WHEN** mbv has started and the music library's Service becomes connected
- **THEN** the system begins resolving grouping artist metadata for the
  library's music grouping levels in the background, before any grouped
  music view is opened

#### Scenario: Grouped view opens from warmed metadata

- **WHEN** the user opens a grouped music album level whose background
  metadata resolution has already completed
- **THEN** the settled artist-grouped ordering is published without an
  organizing wait for artist lookups

#### Scenario: Warm-up is incomplete or fails

- **WHEN** the user opens a grouped music album level whose background
  metadata resolution has not completed or could not obtain metadata
- **THEN** the level settles through the existing organizing state and
  deterministic fallback within the grouping resolution window, and browsing
  remains available

#### Scenario: Warm-up does not gate startup

- **WHEN** background grouping-metadata resolution is still running or has
  failed
- **THEN** the TUI, other Services, and non-music browsing remain fully
  available and unaffected

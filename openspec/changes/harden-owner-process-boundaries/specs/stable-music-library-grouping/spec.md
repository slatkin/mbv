# Spec Delta

## MODIFIED Requirements

### Requirement: Music grouping metadata is warmed at startup
For a configured music library, the system SHALL begin resolving the artist
metadata used for album grouping in the background once its Service is
connected, without waiting for a grouped music view to be opened. Warm-up
SHALL NOT delay application startup or the availability of any other Service
or feature, and a warm-up failure SHALL leave ordinary grouped browsing
usable through the existing settle-and-fallback behavior. Each warm-up request
SHALL be bounded in time and response size like other Service requests; a
request that exceeds either bound SHALL count as a failure, and a failed level
SHALL be retried by later warm-up.

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

#### Scenario: The Service stops responding during warm-up

- **WHEN** a warm-up request is sent and the Service connection then goes silent
- **THEN** the request SHALL fail once its time bound passes and its level SHALL
  be marked failed
- **THEN** the request SHALL no longer count against the number of warm-up
  requests allowed in flight, so other levels can still warm up

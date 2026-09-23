# Spec Delta

## MODIFIED Requirements

### Requirement: Latest mode presents the library's newest episodes
The `Latest` mode SHALL present the TV library's newest episodes from its library-scoped Latest source. Selecting the mode SHALL obtain that content independently of Home; Home SHALL NOT contain a matching Latest section. The rows SHALL be a flat list of episodes; the list SHALL NOT nest.

#### Scenario: Latest shows the library's newest episodes
- **WHEN** the user selects the `Latest` mode for a TV library
- **THEN** the list shows that library's newest episodes as a flat episode list with no nesting

#### Scenario: Latest loads without Home
- **WHEN** the user opens a TV library and selects `Latest` without visiting Home
- **THEN** the library loads and shows the newest episodes

## REMOVED Requirements

### Requirement: TV Latest renders identically to Home Latest (ADDED 2026-09-22)
**Reason**: Home no longer presents TV Latest rows.
**Migration**: The TV Latest rows retain their existing text and metadata on the library surface.

### Requirement: Home and the library present one shared Latest list (ADDED 2026-09-23)
**Reason**: Only the TV destination presents TV Latest, so cross-surface snapshot synchronization is obsolete.
**Migration**: Keep one TV Latest content snapshot used for TV browsing; remove Home projection and duplicate fetch paths.

### Requirement: The Latest mode reflects the library's shared new-content marker
**Reason**: TV Latest no longer shares a marker with Home.
**Migration**: The TV Latest pill alone shows and acknowledges the destination marker under `destination-latest-modes`.

# Spec Delta

## MODIFIED Requirements

### Requirement: Home Latest pills identify content new since the previous client launch
At client startup, mbv SHALL read the previously recorded client-launch timestamp and immediately replace it with the current launch timestamp. For each Home Latest section, mbv SHALL mark the section when at least one item carries a valid provider timestamp strictly later than the previous launch and no later than the current launch. Emby items SHALL use their provider date-added timestamp; Audiobookshelf podcast episodes and Feed entries SHALL use their provider publication timestamp.

The marker SHALL be one `•` after the relevant pill label, painted in the Iris role. For an Emby library section, the same marker SHALL also appear on that library's own `Latest` content mode, because both surfaces stand for the same section (see `tv-library-content-modes`). Continue SHALL never receive the marker. A missing, invalid, equal-to-cutoff, or future provider timestamp SHALL NOT make an item new. When no previous launch timestamp exists, startup SHALL establish the first baseline and show no new-content markers.

#### Scenario: Emby library gained content between launches
- **WHEN** an Emby Latest section contains an item whose date-added timestamp is later than the previous launch and no later than the current launch
- **THEN** that section's Home Latest pill SHALL show an Iris `•`
- **AND** that library's own `Latest` content mode SHALL show the same marker

#### Scenario: Podcast and Feed sections use publication time
- **WHEN** an Audiobookshelf podcast or Feed Latest section contains an item whose publication timestamp falls within the launch interval
- **THEN** that section's Home Latest pill SHALL show an Iris `•`

#### Scenario: Timestamp does not establish new content
- **WHEN** an item has no valid provider timestamp, has a timestamp at or before the previous launch, or has a timestamp after the current launch
- **THEN** that item SHALL NOT cause its Latest pill to show a marker
- **AND** it SHALL NOT cause a library `Latest` content mode to show a marker

#### Scenario: First launch establishes the baseline
- **WHEN** no previous client-launch timestamp exists
- **THEN** mbv SHALL record the current launch timestamp
- **THEN** Home SHALL show no new-content markers for that launch

#### Scenario: Continue is not a Latest section
- **WHEN** one or more Latest pills show new-content markers
- **THEN** the Continue pill SHALL remain unmarked

### Requirement: Visiting a Home Latest pill clears its new-content marker
Selecting a Home Latest pill SHALL acknowledge that section and clear its marker immediately for the remainder of that client run. A Latest pill that is already selected when its content arrives SHALL count as visited and SHALL NOT show a marker. Acknowledgement SHALL be keyed by the section's provider identity so asynchronous section replacement, merging, or reordering cannot restore the marker during the same run.

For an Emby library section, acknowledgement SHALL be shared with that library's `Latest` content mode: selecting Home's `Latest` pill for a library SHALL clear the marker on that library's `Latest` mode, and selecting the library's `Latest` mode SHALL clear it on Home's pill for that library. Because two surfaces now read this acknowledgement, it SHALL be owned by the shell rather than by a single surface.

New content discovered after startup SHALL NOT create a marker during the current run; launch-relative marker evaluation SHALL remain bounded by the current launch timestamp.

#### Scenario: User selects a marked Latest pill
- **WHEN** the user selects a Home Latest pill showing `•`
- **THEN** its marker SHALL clear before the next frame
- **THEN** later refresh or asynchronous replacement of that section during the same run SHALL NOT restore it

#### Scenario: Acknowledging on the library surface clears Home
- **WHEN** the user selects a library's `Latest` content mode while that library's section shows a marker
- **THEN** the marker SHALL clear on the library's `Latest` mode and on Home's `Latest` pill for that library

#### Scenario: Selected section arrives asynchronously
- **WHEN** a Latest section is selected before or as its content arrives
- **THEN** that section SHALL count as visited
- **THEN** its pill SHALL NOT show `•`

#### Scenario: Content appears after the launch instant
- **WHEN** an item carries a provider timestamp later than the current launch timestamp
- **THEN** it SHALL NOT add a marker during the current run
- **THEN** it MAY qualify against the launch interval of a later run

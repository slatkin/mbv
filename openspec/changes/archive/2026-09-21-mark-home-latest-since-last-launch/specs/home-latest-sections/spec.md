# Spec Delta

## ADDED Requirements

### Requirement: Home Latest pills identify content new since the previous client launch
At client startup, mbv SHALL read the previously recorded client-launch timestamp and immediately replace it with the current launch timestamp. For each Home Latest section, mbv SHALL mark the section when at least one item carries a valid provider timestamp strictly later than the previous launch and no later than the current launch. Emby items SHALL use their provider date-added timestamp; Audiobookshelf podcast episodes and Feed entries SHALL use their provider publication timestamp.

The marker SHALL be one `•` after the pill label, painted in the Iris role. Continue SHALL never receive the marker. A missing, invalid, equal-to-cutoff, or future provider timestamp SHALL NOT make an item new. When no previous launch timestamp exists, startup SHALL establish the first baseline and show no new-content markers.

#### Scenario: Emby library gained content between launches
- **WHEN** an Emby Latest section contains an item whose date-added timestamp is later than the previous launch and no later than the current launch
- **THEN** that section's Home Latest pill SHALL show an Iris `•`

#### Scenario: Podcast and Feed sections use publication time
- **WHEN** an Audiobookshelf podcast or Feed Latest section contains an item whose publication timestamp falls within the launch interval
- **THEN** that section's Home Latest pill SHALL show an Iris `•`

#### Scenario: Timestamp does not establish new content
- **WHEN** an item has no valid provider timestamp, has a timestamp at or before the previous launch, or has a timestamp after the current launch
- **THEN** that item SHALL NOT cause its Latest pill to show a marker

#### Scenario: First launch establishes the baseline
- **WHEN** no previous client-launch timestamp exists
- **THEN** mbv SHALL record the current launch timestamp
- **THEN** Home SHALL show no new-content markers for that launch

#### Scenario: Continue is not a Latest section
- **WHEN** one or more Latest pills show new-content markers
- **THEN** the Continue pill SHALL remain unmarked

### Requirement: Visiting a Home Latest pill clears its new-content marker
Selecting a Home Latest pill SHALL acknowledge that section and clear its marker immediately for the remainder of that client run. A Latest pill that is already selected when its content arrives SHALL count as visited and SHALL NOT show a marker. Acknowledgement SHALL be keyed by the section's provider identity so asynchronous section replacement, merging, or reordering cannot restore the marker during the same run.

New content discovered after startup SHALL NOT create a marker during the current run; launch-relative marker evaluation SHALL remain bounded by the current launch timestamp.

#### Scenario: User selects a marked Latest pill
- **WHEN** the user selects a Home Latest pill showing `•`
- **THEN** its marker SHALL clear before the next frame
- **THEN** later refresh or asynchronous replacement of that section during the same run SHALL NOT restore it

#### Scenario: Selected section arrives asynchronously
- **WHEN** a Latest section is selected before or as its content arrives
- **THEN** that section SHALL count as visited
- **THEN** its pill SHALL NOT show `•`

#### Scenario: Content appears after the launch instant
- **WHEN** an item carries a provider timestamp later than the current launch timestamp
- **THEN** it SHALL NOT add a marker during the current run
- **THEN** it MAY qualify against the launch interval of a later run

### Requirement: Home Latest rows show their provider dates in the canonical gutter
Each item in a Home Latest section that carries a valid provider timestamp SHALL show that date in the canonical fixed-width right-aligned media-row gutter, formatted as unpadded day plus abbreviated month (for example, `7 Sep` or `17 Sep`) in the existing green gutter role. Emby rows SHALL show date added; Audiobookshelf podcast and Feed rows SHALL show publication date.

Continue rows SHALL NOT gain a date gutter from this behavior. An item without a valid provider timestamp SHALL reserve no date gutter.

#### Scenario: Latest rows from every supported source carry dates
- **WHEN** dated Emby, Audiobookshelf podcast, and Feed items render in their respective Home Latest sections
- **THEN** each row SHALL show its provider date in the same canonical right-aligned green gutter

#### Scenario: Continue keeps its playback-oriented presentation
- **WHEN** a dated item renders in Continue
- **THEN** this capability SHALL NOT add a date gutter to that row

#### Scenario: Latest item has no usable date
- **WHEN** a Home Latest item has no valid provider timestamp
- **THEN** its row SHALL reserve and paint no date gutter

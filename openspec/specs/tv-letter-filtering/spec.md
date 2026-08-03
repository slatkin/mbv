# tv-letter-filtering Specification

## Purpose
Provide scalable alphabet navigation for large TV libraries so users can browse and filter television show names without paging through unrelated episodes or seasons.

## Requirements

### Requirement: TV libraries expose alphabet range pills
The system SHALL display the same alphabet range pills used by movie libraries for an eligible TV library at its top-level browse view, including `A-C`, `D-F`, `G-I`, `J-L`, `M-O`, `P-R`, `S-U`, `V-Z`, and `#`.

#### Scenario: Large TV library displays pills
- **WHEN** a TV library's top-level show count exceeds the configured large-library threshold and the user is not searching or viewing a drill-down level
- **THEN** the TV tab displays the alphabet range pill row above the show list

#### Scenario: Ineligible TV view hides pills
- **WHEN** the user is searching, viewing a nested TV level, or the top-level TV library does not exceed the threshold
- **THEN** the alphabet range pill row is not displayed

### Requirement: Selecting a TV range filters show names
The system SHALL use the selected alphabet range to fetch and display TV series whose effective show sort name falls within that range, without using episode or season names as the filter key.

#### Scenario: Select a range
- **WHEN** the user selects the `M-O` pill
- **THEN** the TV list displays only series sorted from `M` inclusively through `P` exclusively, with the list cursor reset to the beginning

#### Scenario: Non-letter range
- **WHEN** the user selects the `#` pill
- **THEN** the TV list displays series whose effective sort name falls before `A`, including names beginning with digits or other non-letter characters

#### Scenario: Article-stripped show names
- **WHEN** a series has a display name beginning with a leading article and an Emby sort name without that article
- **THEN** the series is included according to its effective sort name rather than the leading article

### Requirement: TV range navigation preserves existing interactions
The system SHALL support selecting TV alphabet pills with the mouse, cycling them with the existing keyboard controls, and restoring the selected range when a saved TV library position is reopened.

#### Scenario: Cycle ranges with the keyboard
- **WHEN** the user cycles forward or backward while the TV pill row is active
- **THEN** the selected range changes and wraps from the final range to the first, or vice versa

#### Scenario: Restore a selected range
- **WHEN** the user reopens a TV library position saved with an alphabet range selected
- **THEN** the same range is selected and the TV list is loaded with that range applied

# tv-letter-filtering Specification

## Purpose
Provide scalable alphabet navigation for large TV libraries so users can browse and filter television show names without paging through unrelated episodes or seasons.

## Requirements


### Requirement: TV libraries expose alphabet range pills

For an eligible TV library, the system SHALL display three TV-specific alphabet
ranges at the library's top-level browse view: `A-I`, covering every show whose
effective sort name sorts before `J` (including names whose first character is
not a letter, which render under their `#` in-list header), `J-R`, covering
sort names from `J` up to but not including `S`, and `S-Z`, covering every sort
name from `S` onward with no upper bound. The TV ranges SHALL be a TV-specific
set and SHALL NOT change the alphabet ranges used by movie libraries. Whether
the TV range row is displayed, its composition alongside the `Latest`,
`Upcoming`, and `All` modes, and the mode selected on entry SHALL be governed
by `tv-library-content-modes`.

#### Scenario: Large TV library displays pills
- **WHEN** a TV library's top-level show count exceeds the pill threshold and the user is not searching or viewing a drill-down level
- **THEN** the TV tab displays the `A-I`, `J-R`, and `S-Z` range pills above the list

#### Scenario: Ineligible TV view hides pills
- **WHEN** the user is searching, viewing a nested TV level, or the top-level TV library does not exceed the threshold
- **THEN** the TV alphabet range pills are not displayed

### Requirement: Selecting a TV range filters show names

The system SHALL use the selected alphabet range to fetch and display TV series
whose effective show sort name falls within that range, without using episode
or season names as the filter key.

#### Scenario: Select a range
- **WHEN** the user selects the `J-R` range
- **THEN** the TV list displays only series sorted from `J` inclusively through `S` exclusively, with the list cursor reset to the beginning

#### Scenario: Non-letter range
- **WHEN** a series's effective sort name begins with a digit or other non-letter character
- **THEN** it is displayed by the `A-I` range, under a `#` in-list header

#### Scenario: Article-stripped show names
- **WHEN** a series has a display name beginning with a leading article and an Emby sort name without that article
- **THEN** the series is included according to its effective sort name rather than the leading article

### Requirement: TV range navigation preserves existing interactions

The system SHALL support selecting a TV alphabet range with the mouse, cycling
through the available TV content modes with the existing keyboard controls, and
restoring the selected TV content mode when a saved TV library position is
reopened. Mode ordering, cycling wrap, and persistence SHALL be governed by
`tv-library-content-modes`.

#### Scenario: Cycle ranges with the keyboard
- **WHEN** the user cycles forward or backward while the TV content-mode row is active
- **THEN** the selection advances or retreats through the modes in row order and wraps at the ends

#### Scenario: Restore a selected range
- **WHEN** the user reopens a TV library position saved with an alphabet range selected
- **THEN** the same range is selected and the TV list is loaded with that range applied

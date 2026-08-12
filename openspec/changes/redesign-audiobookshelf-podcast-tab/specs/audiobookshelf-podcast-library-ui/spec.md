## Purpose

Provides a consistent, TV-style browsing experience for Audiobookshelf podcast shows and their downloaded episodes without adding playback behavior.

## ADDED Requirements

### Requirement: Podcast libraries use a hierarchical TV-style presentation
An Audiobookshelf podcast library SHALL present podcast shows as the primary list and SHALL present the selected show's details separately, using the established TV-library visual hierarchy and selection treatment.

#### Scenario: Podcast library is displayed
- **WHEN** an Audiobookshelf podcast library is selected
- **THEN** the main content area SHALL show podcast shows using the standard library list or grid presentation
- **THEN** the selected show SHALL have a distinct detail area rather than being flattened into the same row stream as its episodes

#### Scenario: Show selection changes
- **WHEN** the user moves selection between podcast shows
- **THEN** the detail area SHALL follow the selected show
- **THEN** the show list SHALL retain stable provider-native selection identity across loaded-page changes

### Requirement: Selected podcasts provide played-state filters
The selected podcast detail SHALL expose exactly three episode filters: `All`, `Played`, and `Unplayed`. The filters SHALL use the same visual and navigation treatment as TV season selectors.

#### Scenario: All filter is active
- **WHEN** the `All` filter is selected
- **THEN** the episode table SHALL include every downloaded episode for the selected podcast

#### Scenario: Played filter is active
- **WHEN** the `Played` filter is selected
- **THEN** the episode table SHALL include only downloaded episodes with a completed Audiobookshelf progress record

#### Scenario: Unplayed filter is active
- **WHEN** the `Unplayed` filter is selected
- **THEN** the episode table SHALL include only downloaded episodes without a completed Audiobookshelf progress record

#### Scenario: Filter changes
- **WHEN** the user changes the active episode filter
- **THEN** the episode cursor SHALL be reset or clamped to a valid visible episode
- **THEN** the selected podcast SHALL remain selected

### Requirement: Downloaded episodes use a structured episode table
The selected podcast detail SHALL render downloaded episodes in a structured table matching the TV episode-list style, including title, publication information when available, duration when available, and read-only progress or completion state.

#### Scenario: Podcast has downloaded episodes
- **WHEN** the selected podcast has one or more downloaded episodes
- **THEN** the detail area SHALL render one selectable table row per downloaded episode
- **THEN** each row SHALL retain the provider-native episode identity

#### Scenario: Podcast has no matching episodes
- **WHEN** the selected podcast has no downloaded episodes matching the active filter
- **THEN** the detail area SHALL show a scoped empty state
- **THEN** the podcast show list SHALL remain usable

#### Scenario: Episode is selected
- **WHEN** the user moves selection onto an episode row
- **THEN** the row SHALL use the established focused/unfocused episode-row styling
- **THEN** selection SHALL not enqueue, play, or mutate the episode

### Requirement: Personalized shelves are not supported by the podcast UI
The Audiobookshelf podcast library UI SHALL NOT render personalized shelf rows, shelf headings, or shelf-based navigation targets.

#### Scenario: Catalog includes personalized shelves
- **WHEN** Audiobookshelf returns personalized shelf data for a podcast library
- **THEN** the podcast tab SHALL omit that data from its visible presentation
- **THEN** the primary podcast show list and selected-show episode detail SHALL remain unaffected

### Requirement: Podcast activation remains read-only
The redesigned podcast UI SHALL preserve the existing read-only boundary until Audiobookshelf playback is separately introduced.

#### Scenario: User activates a podcast episode
- **WHEN** the user presses the ordinary activation key on a selected episode
- **THEN** mbv SHALL retain or safely update the selection without starting playback, enqueueing an item, opening a playback session, or writing progress

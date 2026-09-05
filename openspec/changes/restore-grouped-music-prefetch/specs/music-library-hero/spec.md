# music-library-hero Specification Delta

## ADDED Requirements

### Requirement: Grouped Music pre-warms neighbour album artwork

While grouped Music is visible and image fetching is idle-gated open, the system SHALL initiate artwork fetches for the albums neighbouring the painted album cursor in display order: up to one behind and up to three ahead, skipping the selected album itself (its artwork fetch is already covered by painting). This SHALL apply in both the narrow inline presentation and the wide hero-on-left presentation. The neighbour window SHALL be keyed off the cursor and display order actually being painted, not a separately resolved cursor. While the search-results grid is active, neighbour prefetch SHALL NOT fire (the grid is not the canonical album rail).

#### Scenario: Scrolling narrow grouped albums warms neighbours

- **WHEN** the user moves the album cursor in the narrow grouped Music view while image fetches are idle-allowed
- **THEN** artwork fetches are initiated for the neighbouring albums in the ±3-ahead/±1-behind display-order window around the painted cursor

#### Scenario: Scrolling the wide right rail warms neighbours

- **WHEN** the user moves the album cursor in the wide grouped Music right rail while image fetches are idle-allowed
- **THEN** artwork fetches are initiated for the neighbouring albums in the same display-order window around the painted cursor

#### Scenario: Rapid navigation suppresses prefetch

- **WHEN** the user is actively navigating (image fetches are idle-gated closed)
- **THEN** no neighbour artwork fetches are initiated

#### Scenario: Search grid suppresses prefetch

- **WHEN** the grouped Music search-results grid is active
- **THEN** no neighbour album-artwork prefetch is initiated for the underlying album order

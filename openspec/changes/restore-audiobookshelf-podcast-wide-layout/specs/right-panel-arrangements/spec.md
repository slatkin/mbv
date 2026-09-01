# Right-panel arrangements

## MODIFIED Requirements

### Requirement: Audiobookshelf podcast Wide uses shared Hero-on-left
The Audiobookshelf podcast browse surface MUST select `shared_hero_presentation` for Wide geometry, using the shared width breakpoint and minimum-height guard. When it applies, the selected Audiobookshelf show detail MUST occupy the left workspace and the right rail MUST contain the browser in one fixed-row column.

#### Scenario: Sufficient Wide terminal
- **GIVEN** the Audiobookshelf podcast content area meets the shared width and height guards
- **WHEN** the podcast surface renders
- **THEN** the hero is on the left and a single-column show/episode browser is on the right
- **AND** the right rail contains the Wide pill row and shared rail framing

#### Scenario: Narrow or short Wide terminal
- **GIVEN** the content area is below the shared width breakpoint or minimum height
- **WHEN** the podcast surface renders
- **THEN** it uses the existing Normal/Narrow inline presentation (or suppression permitted by the shared guard)
- **AND** it does not reserve a separate hero panel or right rail

### Requirement: Provider workspace remains local
The Audiobookshelf podcast episode-selection workspace, provider-specific rows, artwork behavior, selection, and playback intents MUST remain unchanged except for their placement within the shared Wide left/right geometry.

#### Scenario: Episode workspace in Wide mode
- **GIVEN** a selected show has episode selection active
- **WHEN** the Wide surface renders
- **THEN** the existing provider-specific episode pills/table remain available in their designated workspace
- **AND** episode targets and artwork use the existing models and image-enabled/disabled behavior.

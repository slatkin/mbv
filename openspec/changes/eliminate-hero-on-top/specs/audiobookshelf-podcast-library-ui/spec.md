## RENAMED Requirements

- FROM: the requirement for a separate podcast detail arrangement
  TO: `### Requirement: Podcast libraries use responsive hero presentations`

## MODIFIED Requirements

### Requirement: Podcast libraries use responsive hero presentations

An Audiobookshelf podcast library SHALL use the shared hero-on-left presentation when it meets the wide geometry conditions and selected-row replacement otherwise. In hero-on-left, the selected podcast's cover, metadata, played-state filter, and downloaded-episode workspace SHALL occupy the left pane while the single-column podcast-show browser occupies the right rail. In the replacement presentation, the same selected-show detail SHALL replace the active podcast-show row in list flow. The podcast tab SHALL obtain placement from the shared arrangement and SHALL NOT define a separate fallback.

The podcast tab SHALL supply podcast-native data without changing the shared placement rule: Podcast show for Series, Audiobookshelf cover for Series Primary image, `All` / `Played` / `Unplayed` for season selector, and matching downloaded episodes for season episodes. Image shape, metadata lines and order, colour variant, element presence, and image source MAY remain podcast-specific declarations.

#### Scenario: Podcast library is displayed wide
- **WHEN** an Audiobookshelf podcast library meets the shared wide geometry conditions
- **THEN** selected-show detail and downloaded episodes render in the left pane
- **AND** podcast shows render in the single-column right rail

#### Scenario: Podcast library is displayed narrow
- **WHEN** an Audiobookshelf podcast library does not meet the shared wide geometry conditions
- **THEN** podcast shows render in one scrolling column
- **AND** selected-show detail replaces the active show row
- **AND** no separate hero area is reserved above the show browser

#### Scenario: Podcast selection changes
- **WHEN** the user moves selection between podcast shows
- **THEN** the hero or detail workspace updates to the newly selected podcast
- **AND** the show list retains provider-native selection identity across loaded-page changes

#### Scenario: Selected show scrolls in the inline presentation
- **WHEN** the active podcast show moves through the narrow browser
- **THEN** scrolling keeps its media row and inline detail addressable together
- **AND** the replacement block owns the selected parent target while explicit child targets take precedence

#### Scenario: Terminal height cannot fit hero-on-left
- **WHEN** the width meets the shared breakpoint but the minimum-height guard fails
- **THEN** the podcast tab uses selected-row replacement
- **AND** it restores the ordinary selected row if detail cannot fit

#### Scenario: Shared placement changes
- **WHEN** the shared hero-on-left or inline presentation changes
- **THEN** the podcast tab renders the placement change without an individual geometry edit

#### Scenario: Podcast library is displayed
- **WHEN** an Audiobookshelf podcast library is displayed
- **THEN** it uses hero-on-left when wide geometry fits and inline selected-show detail otherwise

#### Scenario: Selected show scrolls outside the visible list rows
- **WHEN** the selected show scrolls outside visible right-rail rows in hero-on-left
- **THEN** the left workspace continues projecting that selected show

#### Scenario: Terminal width crosses the TV list column breakpoint
- **WHEN** the podcast tab crosses the shared width breakpoint
- **THEN** it recomputes hero-on-left versus selected-row replacement rather than changing a detail layout column count

#### Scenario: Terminal height cannot fit the hero
- **WHEN** selected detail cannot fit with a usable active row
- **THEN** detail is suppressed and the browser retains the available area

#### Scenario: The retired separate placement changes
- **WHEN** the obsolete separate placement is removed
- **THEN** Audiobookshelf podcasts continue through only hero-on-left and selected-row replacement

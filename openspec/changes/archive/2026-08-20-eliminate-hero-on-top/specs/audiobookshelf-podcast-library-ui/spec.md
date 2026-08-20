## RENAMED Requirements

- FROM: `### Requirement: Podcast libraries use the hero-on-top arrangement`
  TO: `### Requirement: Podcast libraries use responsive hero presentations`

## MODIFIED Requirements

### Requirement: Podcast libraries use responsive hero presentations

An Audiobookshelf podcast library SHALL use the shared hero-on-left presentation when it meets the wide geometry conditions and the shared inline presentation otherwise. In hero-on-left, the selected podcast's cover, metadata, played-state filter, and downloaded-episode workspace SHALL occupy the left pane while the single-column podcast-show browser occupies the right rail. In the inline presentation, the same selected-show detail SHALL render in list flow at the active podcast-show row. The podcast tab SHALL obtain placement from the shared arrangement and SHALL NOT define a pinned top fallback.

The podcast tab SHALL supply podcast-native data without changing the shared placement rule: Podcast show for Series, Audiobookshelf cover for Series Primary image, `All` / `Played` / `Unplayed` for season selector, and matching downloaded episodes for season episodes. Image shape, metadata lines and order, colour variant, element presence, and image source MAY remain podcast-specific declarations.

#### Scenario: Podcast library is displayed wide
- **WHEN** an Audiobookshelf podcast library meets the shared wide geometry conditions
- **THEN** selected-show detail and downloaded episodes render in the left pane
- **AND** podcast shows render in the single-column right rail

#### Scenario: Podcast library is displayed narrow
- **WHEN** an Audiobookshelf podcast library does not meet the shared wide geometry conditions
- **THEN** podcast shows render in one scrolling column
- **AND** selected-show detail renders inline at the active show row
- **AND** no hero area is pinned above the show browser

#### Scenario: Podcast selection changes
- **WHEN** the user moves selection between podcast shows
- **THEN** the hero or detail workspace updates to the newly selected podcast
- **AND** the show list retains provider-native selection identity across loaded-page changes

#### Scenario: Selected show scrolls in the inline presentation
- **WHEN** the active podcast show moves through the narrow browser
- **THEN** scrolling keeps its media row and inline detail addressable together
- **AND** hero-only rows do not activate another show

#### Scenario: Terminal height cannot fit hero-on-left
- **WHEN** the width meets the shared breakpoint but the minimum-height guard fails
- **THEN** the podcast tab uses inline selected-show detail
- **AND** it does not use a top-pinned fallback

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
- **THEN** it recomputes hero-on-left versus inline placement rather than changing a top-layout column count

#### Scenario: Terminal height cannot fit the hero
- **WHEN** selected detail cannot fit with a usable active row
- **THEN** detail is suppressed and the browser retains the available area

#### Scenario: The hero-on-top arrangement changes
- **WHEN** the obsolete top arrangement is removed
- **THEN** Audiobookshelf podcasts continue through only hero-on-left and inline presentations

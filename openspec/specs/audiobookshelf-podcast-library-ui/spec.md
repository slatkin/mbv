# audiobookshelf-podcast-library-ui Specification

## Purpose

Provides an Audiobookshelf podcast browsing experience whose presentation and interaction are structurally identical to the TV Shows tab, with podcast-native data substituted for TV-native data and without adding playback behavior.

## Requirements

### Requirement: The selected podcast hero uses Audiobookshelf cover artwork
The selected podcast hero SHALL place the selected podcast's Audiobookshelf cover in the same right-aligned image slot, with the same dimensions, scaling, text wrapping, loading treatment, and images-disabled behavior as the selected Series Primary image in the TV Shows hero. The cover SHALL be fetched from the configured Audiobookshelf Service using the selected podcast's provider-native library item identity.

Podcast title and available author metadata SHALL occupy the corresponding TV hero text area. Missing metadata SHALL collapse without moving the image or changing the TV hero's structural rules.

#### Scenario: Selected podcast has a cover
- **WHEN** images are enabled and the selected podcast has an Audiobookshelf cover
- **THEN** that cover SHALL be fetched and rendered in the TV Series image position within selected detail
- **THEN** the cover SHALL NOT be rendered as a thumbnail in the lower show list

#### Scenario: Selected podcast cover is loading
- **WHEN** images are enabled and the selected podcast cover request is pending
- **THEN** the hero SHALL reserve and paint the same image placeholder area used while a TV Series image is loading

#### Scenario: Selected podcast has no usable cover
- **WHEN** images are enabled but the selected podcast has no usable cover
- **THEN** the hero SHALL follow the same missing-Primary-image behavior as the TV Shows hero without breaking its text, filter, or episode layout

#### Scenario: Images are disabled
- **WHEN** images are disabled
- **THEN** the podcast hero SHALL omit cover fetching and rendering
- **THEN** its text SHALL use the same image-disabled width and row budgeting as the TV Shows hero

### Requirement: Selected podcasts map TV season selection to played-state filters
The selected podcast hero SHALL expose exactly three episode filters: `All`, `Played`, and `Unplayed`. These filters SHALL occupy the same selector row and use the same pill appearance, overflow behavior, focus treatment, and selection-mode visibility as TV season selectors.

#### Scenario: Podcast show is selected but episode selection is inactive
- **WHEN** a podcast show is selected and the user has not entered episode-selection mode
- **THEN** the hero SHALL present the filter summary in the same state and position in which the TV hero presents its season summary
- **THEN** the episode rows SHALL have the same visibility as TV episode rows outside season-selection mode

#### Scenario: User enters episode selection
- **WHEN** the user activates the selected podcast show
- **THEN** the `All`, `Played`, and `Unplayed` pills SHALL become selectable in the TV season-selector position
- **THEN** focus SHALL enter the filtered episode rows using the same visual mode transition as the TV Shows tab

#### Scenario: Played and unplayed filters
- **WHEN** `Played` or `Unplayed` is selected
- **THEN** Played SHALL include only completed progress and Unplayed SHALL include missing or incomplete progress

#### Scenario: Filter changes
- **WHEN** the user changes the active episode filter using the controls corresponding to TV season navigation
- **THEN** the episode cursor SHALL reset to a valid visible episode
- **THEN** the selected podcast SHALL remain selected

### Requirement: Downloaded episodes use the TV episode-list presentation
Downloaded podcast episodes SHALL render in the same table area and with the same row height, marker position, title and duration column geometry, truncation, focused and unfocused colors, cursor styling, and available row budget as TV episodes. The podcast implementation SHALL substitute podcast-native episode data without converting it to an Emby item.

#### Scenario: Podcast has downloaded episodes
- **WHEN** the selected podcast has matching downloaded episodes and episode selection is active
- **THEN** the hero SHALL render one selectable TV-style episode row per matching episode with provider-native identities

#### Scenario: Podcast detail is empty or loading
- **WHEN** matching episodes are empty or detail is loading
- **THEN** the episode-table area SHALL show a scoped state without collapsing the hero or hiding the lower show list

### Requirement: Personalized shelves are absent from the podcast tab
The Audiobookshelf podcast tab SHALL NOT render or navigate personalized shelf data, and shelf data SHALL NOT affect show order, selection, scrolling, hit testing, or pagination.

#### Scenario: Catalog includes personalized shelves
- **WHEN** Audiobookshelf returns personalized shelf data
- **THEN** the top selected-podcast hero and lower podcast show list SHALL remain unaffected

### Requirement: Podcast activation remains read-only
Activating a podcast show SHALL only enter episode-selection mode. Activating a podcast episode SHALL consume the activation without starting playback, enqueueing an item, opening a playback run or Session, or writing progress.

#### Scenario: User activates a podcast episode
- **WHEN** the user activates a selected podcast episode
- **THEN** mbv SHALL retain selection without playback, queue, Session, or progress side effects

### Requirement: Podcast libraries use responsive hero presentations

An Audiobookshelf podcast library SHALL use the shared hero-on-left presentation when it meets the wide geometry conditions and selected-row replacement otherwise. In hero-on-left, the selected podcast's cover, metadata, played-state filter, and downloaded-episode workspace SHALL occupy the left pane while the single-column podcast-show browser occupies the right rail. In the replacement presentation, the same selected-show detail SHALL replace the active podcast-show row in list flow. The podcast tab SHALL obtain placement from the shared arrangement and SHALL NOT define a separate detail fallback.

The podcast tab SHALL supply podcast-native data without changing the shared placement rule: Podcast show for Series, Audiobookshelf cover for Series Primary image, `All` / `Played` / `Unplayed` for season selector, and matching downloaded episodes for season episodes. Image shape, metadata lines and order, colour variant, element presence, and image source MAY remain podcast-specific declarations.

#### Scenario: Podcast library is displayed wide
- **WHEN** an Audiobookshelf podcast library meets the shared wide geometry conditions
- **THEN** selected-show detail and downloaded episodes render in the left pane
- **AND** podcast shows render in the single-column right rail

#### Scenario: Podcast library is displayed narrow
- **WHEN** an Audiobookshelf podcast library does not meet the shared wide geometry conditions
- **THEN** podcast shows render in one scrolling column
- **AND** selected-show detail replaces the active show row
- **AND** no separate detail area is reserved above the show browser

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
- **THEN** the podcast tab uses inline selected-show detail
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
- **WHEN** the obsolete top arrangement is removed
- **THEN** Audiobookshelf podcasts continue through only hero-on-left and selected-row replacement

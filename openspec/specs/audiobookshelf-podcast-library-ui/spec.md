# audiobookshelf-podcast-library-ui Specification

## Purpose

Provides an Audiobookshelf podcast browsing experience whose presentation and interaction are structurally identical to the TV Shows tab, with podcast-native data substituted for TV-native data and without adding playback behavior.

## Requirements

### Requirement: Podcast libraries use the TV Shows tab composition
An Audiobookshelf podcast library SHALL use the same outer composition as the TV Shows tab at the same terminal dimensions and image setting. The selected podcast hero SHALL occupy the dedicated full-width area pinned above the podcast show list. The podcast show list SHALL occupy the remaining area below the hero and SHALL NOT appear beside the hero or detail content at any terminal width.

The following substitutions SHALL be the only domain changes to that composition:

| TV Shows tab | Audiobookshelf podcast tab |
|---|---|
| Series | Podcast show |
| Series Primary image | Audiobookshelf podcast cover |
| Season selector | `All` / `Played` / `Unplayed` filter selector |
| Episodes in the selected season | Downloaded episodes matching the selected filter |

All other observable layout behavior SHALL match the TV Shows tab, including the hero shell and placement, list-below-hero ordering, content padding, image slot, row budgeting, list column count, selected-cell treatment, focus styling, scrolling, narrow-terminal fallback, and loading placeholder stability.

#### Scenario: Podcast library is displayed
- **WHEN** an Audiobookshelf podcast library and a TV Shows library are displayed at the same terminal dimensions and image setting
- **THEN** both tabs SHALL divide the content area into the same top hero and lower list geometry
- **THEN** the podcast tab SHALL render podcast shows in the lower list positions occupied by Series rows in the TV Shows tab
- **THEN** the podcast tab SHALL NOT render a left catalog column beside a right detail column

#### Scenario: Podcast selection changes
- **WHEN** the user moves selection between podcast shows
- **THEN** the fixed hero SHALL update to the newly selected podcast without changing its screen position
- **THEN** the show list SHALL retain provider-native selection identity across loaded-page changes

#### Scenario: Selected show scrolls outside the visible list rows
- **WHEN** the selected podcast's row is outside the visible portion of the lower show list
- **THEN** the pinned hero SHALL continue to display that selected podcast in the same fixed position

#### Scenario: Terminal width crosses the TV list column breakpoint
- **WHEN** the podcast tab crosses a width at which the TV Shows tab changes between one and two list columns
- **THEN** the podcast show list SHALL change column count at the same breakpoint
- **THEN** the hero SHALL remain full-width above the list and SHALL NOT move to the side

#### Scenario: Terminal height cannot fit the hero
- **WHEN** the TV Shows tab would suppress its hero because the available height cannot fit the minimum hero and a usable list
- **THEN** the podcast tab SHALL suppress its hero under the same condition and give the show list the corresponding area

### Requirement: The selected podcast hero uses Audiobookshelf cover artwork
The selected podcast hero SHALL place the selected podcast's Audiobookshelf cover in the same right-aligned image slot, with the same dimensions, scaling, text wrapping, loading treatment, and images-disabled behavior as the selected Series Primary image in the TV Shows hero. The cover SHALL be fetched from the configured Audiobookshelf Service using the selected podcast's provider-native library item identity.

Podcast title and available author metadata SHALL occupy the corresponding TV hero text area. Missing metadata SHALL collapse without moving the image or changing the TV hero's structural rules.

#### Scenario: Selected podcast has a cover
- **WHEN** images are enabled and the selected podcast has an Audiobookshelf cover
- **THEN** that cover SHALL be fetched and rendered in the TV Series image position within the top hero
- **THEN** the cover SHALL NOT be rendered as a thumbnail in the lower show list or in a separate side panel

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

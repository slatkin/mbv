## REMOVED Requirements

### Requirement: Podcast libraries use alphabetical panel pills

**Reason**: Surname-range pills over show titles collapse to near-no-op bars on personal podcast
libraries and have no keyboard path on this tab; podcast-native organisation (play state and
publish recency) replaces them.
**Migration**: The state-and-show pill selector requirement in this delta defines the podcast
tab's Selector row; Audiobookshelf Books keep their surname ranges unchanged.

### Requirement: The selected podcast hero uses Audiobookshelf cover artwork

**Reason**: The selected-show hero is removed; the hero now presents the selected episode with
the parent show's cover.
**Migration**: The episode-hero requirement in this delta defines artwork, content lines, and
loading/images-disabled behavior.

### Requirement: Podcast libraries use responsive hero presentations

**Reason**: The show-browser composition (show list, show hero workspace, episode workspace,
selection modal) is retired; the tab becomes a flat episode browser.
**Migration**: The flat-episode-browser requirement in this delta defines the shared Wide and
non-Wide presentations the tab now follows.

### Requirement: Downloaded episodes use the selection modal

**Reason**: The show browser and its constituent-list modal are removed; episodes are directly
listed and playable in the flat list.
**Migration**: Episodes are selectable rows in the flat episode list; activation is defined by
the `audiobookshelf-podcast-browsing` and `audiobookshelf-podcast-playback` capabilities.

### Requirement: Podcast activation remains read-only

**Reason**: Activation now plays the selected episode through the existing podcast playback
boundary, matching Feeds.
**Migration**: Playback and enqueue semantics are owned by `audiobookshelf-podcast-browsing`
(explicit episode actions) and `audiobookshelf-podcast-playback`.

## ADDED Requirements

### Requirement: The episode hero uses the parent show's cover artwork

The podcast tab's hero SHALL present the selected episode's information: the episode title, the
episode description, the duration, and the user's progress (resume position or finished state) as
hero content lines, with the parent podcast show's name and author as the corresponding credits
lines. The hero SHALL place the parent show's Audiobookshelf cover in the same right-aligned
Square image slot, with the same dimensions, scaling, text wrapping, loading treatment, and
images-disabled behavior as the TV Series image position. The cover SHALL be fetched from the
configured Audiobookshelf Service using the parent show's provider-native library item identity.
The hero SHALL NOT present a Workspace, a filter selector row, or an episode list. Missing
metadata SHALL collapse without moving the image or changing the shared hero's structural rules.

#### Scenario: Selected episode has a cover

- **WHEN** images are enabled and the selected episode's parent show has an Audiobookshelf cover
- **THEN** that cover SHALL be fetched and rendered in the TV Series image position within selected detail
- **THEN** the cover SHALL NOT be rendered as a thumbnail on any episode row

#### Scenario: Selected episode cover is loading

- **WHEN** images are enabled and the parent-show cover request is pending
- **THEN** the hero SHALL reserve and paint the same image placeholder area used while a TV Series image is loading

#### Scenario: The parent show has no usable cover

- **WHEN** images are enabled but the parent show has no usable cover
- **THEN** the hero SHALL follow the same missing-Primary-image behavior as the TV Shows hero without breaking its text layout

#### Scenario: Images are disabled

- **WHEN** images are disabled
- **THEN** the episode hero SHALL omit cover fetching and rendering
- **THEN** its text SHALL use the same image-disabled width and row budgeting as the TV Shows hero

#### Scenario: Episode selection changes

- **WHEN** the user moves selection between episode rows
- **THEN** the hero updates to the newly selected episode's facts and parent-show credits

### Requirement: Podcast libraries render a flat episode browser

An Audiobookshelf podcast library SHALL render through the Library panel like every Emby library
as a flat episode browser. The Selector row SHALL carry the podcast tab's pill selector (the
`All` / `Unplayed` / `Played` state pills followed by one pill per podcast show). The list SHALL
render one selectable episode row per matching episode, grouped under the five Feeds age-group
headings. At Wide geometry, the selected episode's hero (facts, credits, cover, no Workspace)
SHALL occupy the Hero pane while the single-column episode browser occupies the left rail.
Otherwise episodes SHALL remain ordinary media rows in the non-Wide browser, and the selected
episode's hero SHALL be revealed through the Library Hero overlay. The podcast tab SHALL obtain
placement from the shared Library panel and SHALL NOT define a separate fallback or any
podcast-specific presentation declaration.

#### Scenario: Podcast library is displayed wide

- **WHEN** an Audiobookshelf podcast library meets the shared wide geometry conditions
- **THEN** the selected episode's hero (facts, credits, cover, no Workspace) renders in the Hero pane
- **AND** grouped episode rows render in the single-column left rail

#### Scenario: Podcast library is displayed narrow

- **WHEN** an Audiobookshelf podcast library does not meet the shared wide geometry conditions
- **THEN** grouped episode rows render in one scrolling column
- **AND** every episode remains an ordinary media row in that column
- **AND** Enter on the selected episode opens the Library Hero overlay for that episode

#### Scenario: Podcast selection changes

- **WHEN** the user moves selection between episode rows
- **THEN** the Hero header updates to the newly selected episode
- **AND** the episode list retains provider-native selection identity across loaded-page changes

#### Scenario: Selected episode scrolls outside the visible list rows

- **WHEN** the selected episode's row is outside the visible portion of the left rail at Wide geometry
- **THEN** the hero continues projecting that selected episode

#### Scenario: Terminal height cannot fit Wide hero

- **WHEN** the width meets the shared breakpoint but the minimum-height guard fails
- **THEN** the podcast tab uses the non-Wide presentation
- **AND** episode rows are retained in the available area

#### Scenario: Shared placement changes

- **WHEN** the shared Wide or Narrow library panel presentation changes
- **THEN** the podcast tab renders the change without an individual geometry edit

#### Scenario: Terminal width crosses the breakpoint

- **WHEN** the podcast tab crosses the shared width breakpoint
- **THEN** it recomputes Wide versus non-Wide library presentation rather than changing a detail layout column count

#### Scenario: The retired show-browser placement changes

- **WHEN** the obsolete show-list and show-hero placements are removed
- **THEN** Audiobookshelf podcasts continue through only the shared Wide and non-Wide library panels

### Requirement: Podcast tab uses one state-and-show pill selector

The podcast tab's Selector row SHALL present one mutually exclusive pill selection: the state
pills `All`, `Unplayed`, and `Played`, followed by one pill per podcast show in the library. A
state pill SHALL show matching episodes across all shows; a show pill SHALL show that show's
episodes regardless of play state. State and show selections SHALL NOT combine in one view.
`Unplayed` SHALL include episodes with missing or incomplete (in-progress) progress; `Played`
SHALL include only completed progress. Pills SHALL use the shared `render_pill_bar` widget, follow
its label truncation and overflow contract, and SHALL write `layout.selector_tabs`. The last
active pill SHALL be remembered in session memory across tab switches and SHALL reset to `All`
when mbv restarts.

#### Scenario: Podcast tab renders state-and-show pills

- **WHEN** the Audiobookshelf podcast tab is displayed with shows available
- **THEN** the Selector row renders `All`, `Unplayed`, and `Played` followed by one pill per podcast show
- **AND** no alphabetical range bucket, `#` bucket, or empty-range pill renders

#### Scenario: State filter semantics

- **WHEN** `Unplayed` is active
- **THEN** only episodes with missing or incomplete progress render across all shows
- **WHEN** `Played` is active
- **THEN** only episodes with completed progress render across all shows

#### Scenario: Show pill scope

- **WHEN** a show pill is active
- **THEN** the list renders that show's episodes under the same age-group headings
- **AND** episodes of other shows do not render

#### Scenario: Mutually exclusive selection

- **WHEN** the user picks a show pill while a state pill is active (or the reverse)
- **THEN** the selector moves to the picked pill as the single active selection
- **AND** no combined state-plus-show view exists

#### Scenario: Last pill remembered in session

- **WHEN** the user leaves the podcast tab and returns without restarting mbv
- **THEN** the remembered pill is active again
- **WHEN** mbv restarts
- **THEN** the `All` pill is active

#### Scenario: Pills use the shared widget

- **WHEN** the state-and-show pills are rendered
- **THEN** they use the same `render_pill_bar` widget and overflow behavior as every other library tab
- **AND** they follow the shared label truncation contract (show-name pills truncated like feed-group labels)
- **AND** they write `layout.selector_tabs` for mouse hit-testing

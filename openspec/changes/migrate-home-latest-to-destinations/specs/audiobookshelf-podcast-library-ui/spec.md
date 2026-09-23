# Spec Delta

## MODIFIED Requirements

### Requirement: Podcast libraries render a flat episode browser

An Audiobookshelf podcast library SHALL render through the Library panel as a flat episode browser. The
Selector row SHALL carry the podcast tab's pill selector (`Latest`, then `All` / `Unplayed` / `Played`,
followed by one pill per subscribed podcast). The list SHALL render one selectable episode row per matching
episode, grouped under the five Feeds age-group headings, with every row naming its parent podcast. At Wide
geometry, the selected episode's hero (facts, cover, no Workspace) SHALL occupy the Hero pane while the
single-column episode browser occupies the list rail. Otherwise episodes SHALL remain ordinary media rows in
the non-Wide browser and Enter SHALL play the selected episode. The podcast tab SHALL obtain placement from
the shared Library panel and SHALL NOT define a separate fallback or any podcast-specific presentation
declaration.

#### Scenario: Podcast library is displayed wide

- **WHEN** an Audiobookshelf podcast library meets the shared wide geometry conditions
- **THEN** the selected episode's hero (facts, cover, no Workspace) renders in the Hero pane
- **AND** grouped episode rows render in the single-column list rail

#### Scenario: Podcast library is displayed narrow

- **WHEN** an Audiobookshelf podcast library does not meet the shared wide geometry conditions
- **THEN** grouped episode rows render in one scrolling column
- **AND** every episode remains an ordinary media row in that column
- **AND** Enter on the selected episode plays it

#### Scenario: Podcast selection changes

- **WHEN** the user moves selection between episode rows
- **THEN** the Hero header updates to the newly selected episode
- **AND** the episode list retains provider-native selection identity across loaded-page changes

#### Scenario: Selected episode scrolls outside the visible list rows

- **WHEN** the selected episode's row is outside the visible portion of the list rail at Wide geometry
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
The podcast tab's Selector row SHALL present one mutually exclusive pill selection: `Latest`, the state pills `All`, `Unplayed`, and `Played`, followed by one pill per subscribed podcast. `Latest` SHALL show the library's Newest Episodes shelf independently of the selected show or state filter. A state pill SHALL show matching episodes across all shows; a show pill SHALL show that show's episodes regardless of play state. Exactly one pill SHALL be active, identified by value rather than by position, and state and show selections SHALL NOT combine in one view. `Unplayed` SHALL include episodes with missing or incomplete (in-progress) progress; `Played` SHALL include only completed progress. Pills SHALL use the shared `render_pill_bar` widget, follow its label truncation and overflow contract, and SHALL write `layout.selector_tabs`. `[` and `]` SHALL move through the pill bar as one uniform gesture — there is one pill kind, with no per-kind key, ordering, or behaviour. The last active pill SHALL be remembered in session memory across tab switches and SHALL reset to `All` when mbv restarts.

#### Scenario: Podcast tab renders state-and-show pills
- **WHEN** the Audiobookshelf podcast tab is displayed with shows available
- **THEN** the Selector row renders `Latest`, `All`, `Unplayed`, and `Played` followed by one pill per subscribed podcast
- **AND** no alphabetical range bucket, `#` bucket, or empty-range pill renders

#### Scenario: Latest is independent of other pills
- **WHEN** the user selects `Latest` while a show or state pill was active
- **THEN** the list shows the library's newest episodes from its Newest Episodes shelf
- **AND** no show or played-state filter excludes those episodes

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

#### Scenario: Keyboard navigation
- **WHEN** the user presses `]` or `[` on the podcast tab
- **THEN** the active pill moves to the next or previous pill in the bar, wrapping at either end
- **AND** the movement is identical whether the pill is Latest, a state pill, or a show pill

#### Scenario: Last pill remembered in session
- **WHEN** the user leaves the podcast tab and returns without restarting mbv
- **THEN** the remembered pill is active again
- **WHEN** mbv restarts
- **THEN** the `All` pill is active

#### Scenario: Pills use the shared widget
- **WHEN** the selector pills are rendered
- **THEN** they use the same `render_pill_bar` widget and overflow behavior as every other library tab
- **AND** they follow the shared label truncation contract (show-name pills truncated like feed-group labels)
- **AND** they write `layout.selector_tabs` for mouse hit-testing

## MODIFIED Requirements

### Requirement: Podcast libraries use responsive hero presentations

An Audiobookshelf podcast library SHALL render through the Library panel like every Emby library. At
Wide geometry, the selected podcast's Square Hero header, overview box, and a Workspace (the
`All` / `Played` / `Unplayed` filter pills in the Workspace Selector row over the downloaded-episode
list) SHALL occupy the Hero pane while the single-column podcast-show browser occupies the left rail.
Otherwise the selected show's inline hero (title, author, description and cover only) SHALL replace
the active podcast-show row in list flow, and its filter pills and episodes SHALL NOT render inside it.
The podcast tab SHALL obtain placement from the shared Library panel and SHALL NOT define a separate
fallback or any podcast-specific presentation declaration.

The podcast tab SHALL supply podcast-native data without changing the shared presentation: Podcast
show for Series, Audiobookshelf cover for the Square header artwork, and matching downloaded episodes
for the Workspace and selection modal.

#### Scenario: Podcast library is displayed wide

- **WHEN** an Audiobookshelf podcast library meets the shared wide geometry conditions
- **THEN** selected-show detail and downloaded episodes render in the Hero pane
- **AND** podcast shows render in the single-column left rail

#### Scenario: Podcast library is displayed narrow

- **WHEN** an Audiobookshelf podcast library does not meet the shared wide geometry conditions
- **THEN** podcast shows render in one scrolling column with alphabetical panel pills
- **AND** selected-show detail (title, author, description, cover) replaces the active show row
- **AND** no filter pills and no episode rows render inside that inline detail
- **AND** no separate hero area is reserved above the show browser

#### Scenario: Podcast selection changes

- **WHEN** the user moves selection between podcast shows
- **THEN** the hero or detail workspace updates to the newly selected podcast
- **AND** the show list retains provider-native selection identity across loaded-page changes

#### Scenario: Selected show scrolls in the inline presentation

- **WHEN** the active podcast show moves through the narrow browser
- **THEN** scrolling keeps its media row and inline detail addressable together
- **AND** the replacement block owns the selected parent target while explicit child targets take precedence

#### Scenario: Terminal height cannot fit Wide hero

- **WHEN** the width meets the shared breakpoint but the minimum-height guard fails
- **THEN** the podcast tab uses selected-row replacement
- **AND** it restores the ordinary selected row if detail cannot fit

#### Scenario: Shared placement changes

- **WHEN** the shared Wide or Narrow library panel presentation changes
- **THEN** the podcast tab renders the change without an individual geometry edit

#### Scenario: Podcast library is displayed

- **WHEN** an Audiobookshelf podcast library is displayed
- **THEN** it uses the Wide library panel when wide geometry fits and the Narrow library panel otherwise

#### Scenario: Selected show scrolls outside the visible list rows

- **WHEN** the selected show scrolls outside visible left-rail rows in Wide hero
- **THEN** the right workspace continues projecting that selected show

#### Scenario: Terminal width crosses the TV list column breakpoint

- **WHEN** the podcast tab crosses the shared width breakpoint
- **THEN** it recomputes Wide versus Narrow library panel rather than changing a detail layout column count

#### Scenario: Terminal height cannot fit the hero

- **WHEN** selected detail cannot fit with a usable active row
- **THEN** detail is suppressed and the browser retains the available area

#### Scenario: The retired separate placement changes

- **WHEN** the obsolete separate placement is removed
- **THEN** Audiobookshelf podcasts continue through only the Wide and Narrow library panels

### Requirement: Downloaded episodes use the selection modal

Downloaded podcast episodes SHALL be listed in the constituent-list modal (see `inline-hero-selection-modal`) when the user presses Enter on a selected podcast show in the inline presentation. The modal SHALL render one selectable row per matching episode with the episode title and duration. At Wide geometry the same matching episodes SHALL render in the Hero pane's Workspace using the TV episode-list presentation; they SHALL NOT render inside the inline selected-show detail.

#### Scenario: User opens the episode modal

- **WHEN** the user presses Enter on a selected podcast show in the inline presentation
- **THEN** the constituent-list modal opens with matching downloaded episodes
- **AND** each episode shows its title and duration

#### Scenario: User selects an episode from the modal

- **WHEN** the user navigates to an episode in the modal and presses Enter
- **THEN** the episode is selected according to the podcast tab's existing activation behavior
- **AND** the modal closes

#### Scenario: Podcast detail is empty or loading

- **WHEN** matching episodes are empty or detail is loading
- **THEN** the modal and the Wide Workspace each show their scoped empty or loading state when visible
- **AND** the surrounding selected-show detail remains available

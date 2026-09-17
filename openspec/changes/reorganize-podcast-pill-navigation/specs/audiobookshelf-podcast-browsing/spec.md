## REMOVED Requirements

### Requirement: Podcast shows load incrementally with stable selection

**Reason**: The show browser is removed; the tab lists episodes, and pagination moves to the
expanded library-items contract.
**Migration**: The paged expanded-items requirement in this delta carries the same
bounded-pagination and stable-identity disciplines for episodes.

### Requirement: Podcast libraries use the shared responsive hero presentation

**Reason**: The TV-parallel composition (Series/Season substitution table, show list, show hero
workspace, selection modal) is retired; the tab becomes a flat episode browser mirroring Feeds.
**Migration**: The flat episode feed presentation requirement in this delta defines the shared
Wide and non-Wide presentations the tab now follows.

### Requirement: Selected podcasts map TV season selection to played-state filters

**Reason**: The season-selector position and its hero-Workspace semantics are removed; played
state filtering moves to the panel pill selector.
**Migration**: The `audiobookshelf-podcast-library-ui` state-and-show pill selector requirement
defines `All` / `Unplayed` / `Played` semantics.

### Requirement: Downloaded episodes use the TV episode-list presentation

**Reason**: Episodes no longer render inside a hero Workspace as TV-parallel episode tables; they
are the tab's primary list rows with age-group headings.
**Migration**: The shared list row presentation requirement in this delta defines row semantics,
heading insertion, and stale-result discipline.

## ADDED Requirements

### Requirement: Podcast libraries use the flat episode feed presentation

An Audiobookshelf podcast library SHALL browse episodes directly: the list SHALL render the five
Feeds age-group headings (`New`, `Recent`, `Older than two weeks`, `Older than a month`,
`Unknown date`) with one selectable episode row per matching episode beneath each heading, scoped
by the active pill. The selected episode's hero SHALL occupy the Wide Hero pane when wide geometry
fits and SHALL be revealed through the Library Hero overlay otherwise. The podcast tab SHALL NOT
render a show browser, a show hero workspace, or a constituent-list modal, and SHALL NOT define a
surface-specific geometry rule.

The following substitutions SHALL be the only domain changes to that composition:

| Feeds tab | Audiobookshelf podcast tab |
|---|---|
| Feed entry | Downloaded episode |
| Feed subscription pill | Podcast show pill |
| Watched filter pills | `All` / `Unplayed` / `Played` state pills |
| Feed age-group headings | Feed age-group headings (identical) |

All other observable layout behavior SHALL match the Feeds tab, including the pill row, list
columns, selected-cell treatment, focus styling, scrolling, loading placeholder stability, and the
read-only Wide hero beside a single-column browser.

#### Scenario: Podcast library is displayed

- **WHEN** an Audiobookshelf podcast library and the Feeds tab are displayed at the same terminal dimensions and image setting
- **THEN** both tabs SHALL use the same shared wide or non-Wide presentation for their available geometry
- **THEN** the podcast tab SHALL render episode rows in the browser positions occupied by feed entries
- **AND** the selected episode's hero SHALL occupy the Wide hero position beside the single-column browser

#### Scenario: Podcast episode selection changes

- **WHEN** the user moves selection between episode rows
- **THEN** the hero or overlay SHALL update to the newly selected episode
- **THEN** the episode list SHALL retain provider-native selection identity across loaded-page changes

#### Scenario: Age groups match Feeds criteria

- **WHEN** episode rows are grouped
- **THEN** the headings and their day boundaries SHALL be identical to the Feeds tab's groups, including `Unknown date` for episodes without a publish date

#### Scenario: Selected episode moves through the non-Wide browser

- **WHEN** the selected episode moves through the non-Wide browser
- **THEN** the episode remains an ordinary selectable row and Enter opens its Library Hero overlay

#### Scenario: Terminal width crosses the shared breakpoint

- **WHEN** the podcast tab crosses the shared width breakpoint
- **THEN** it recomputes Wide versus non-Wide library presentation rather than changing a detail layout column count

#### Scenario: Terminal height cannot fit the hero

- **WHEN** the Wide hero cannot fit with a usable list
- **THEN** the podcast tab uses the non-Wide presentation and the browser retains the available area

### Requirement: Downloaded episodes use the shared list row presentation

Downloaded podcast episodes SHALL render as selectable rows in the shared media-list presentation
with the same row height, title and duration column geometry, truncation, focused and unfocused
colors, cursor styling, played/resume semantic state, and available row budget as the Feeds tab's
entry rows. Non-selectable age-group heading and spacer rows SHALL be inserted into the list flow
without changing episode indices or stable episode targeting. The podcast implementation SHALL
substitute podcast-native episode data without converting it to an Emby item.

#### Scenario: Active view has episodes

- **WHEN** the active pill view has matching episodes
- **THEN** the list renders one selectable row per episode with provider-native identities, titles, and durations
- **AND** age-group heading and spacer rows render without shifting episode targeting

#### Scenario: Active view is empty or loading

- **WHEN** the active pill view has no matching episodes or pages are still loading
- **THEN** the list SHALL show its scoped empty or loading state without disturbing the Selector row or hero

#### Scenario: A page completes after selection moved

- **WHEN** a page result arrives after the user has changed the active pill or selection
- **THEN** mbv SHALL NOT replace the current view's rows with stale-page content

### Requirement: Flat episode views load from paged expanded library items

mbv SHALL load the podcast tab's flat episode views by paging the library's items with full
episode expansion, following the same Audiobookshelf 2.36 bounded-pagination contract the show
list used. Each item SHALL contribute its downloaded episodes to the active views; pages SHALL
land progressively with no episode cap. Episode identity SHALL remain the Audiobookshelf Service
kind plus `libraryItemId` and `episodeId`. Every episode result SHALL be reconciled with the
Service setup generation that initiated it, as catalog results already are.

#### Scenario: Pages land progressively

- **WHEN** expanded-items pages complete while the tab is open
- **THEN** each page's episodes appear in the active views without a visible reload of already-listed rows
- **AND** age-group headings remain in place while their member rows fill in

#### Scenario: Navigation approaches the loaded page boundary

- **WHEN** selection approaches the end of the currently loaded episodes and more pages exist
- **THEN** mbv SHALL request the next bounded page and append each episode at most once
- **THEN** existing episodes SHALL remain navigable while the request is pending

#### Scenario: Selection survives page arrival

- **WHEN** a new page lands while an episode is selected
- **THEN** the selected episode keeps its row and cursor position when its identity remains present

#### Scenario: Stale page after Service replacement

- **WHEN** an expanded-items page initiated for the previous Audiobookshelf server arrives after Service replacement
- **THEN** mbv SHALL ignore it without changing the current views, selection, or Service state

#### Scenario: Refresh removes the selected episode

- **WHEN** the episode list refreshes and the selected episode identity is no longer present
- **THEN** mbv SHALL select the nearest valid episode or the active view's empty state

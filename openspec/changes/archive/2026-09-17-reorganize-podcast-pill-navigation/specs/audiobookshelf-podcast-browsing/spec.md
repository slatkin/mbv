## REMOVED Requirements

### Requirement: Podcast shows load incrementally with stable selection

**Reason**: The show browser is removed. Shows survive only as the pill bar's entries, and the tab's
selection is the active pill plus the selected episode, not a show cursor.
**Migration**: The `Flat episode views load from the paged show list and per-show episode fan-out`
requirement in this delta carries the bounded-pagination, append-at-most-once and stable-identity
disciplines for the show list and for the episode views.

### Requirement: Podcast libraries use the shared responsive hero presentation

**Reason**: The TV-parallel composition (Series/Season substitution table, show list, show hero workspace,
selection modal) is retired; the tab becomes a flat episode browser mirroring Feeds.
**Migration**: The `Podcast libraries use the flat episode feed presentation` requirement in this delta
defines the shared Wide and non-Wide presentations the tab now follows.

### Requirement: The selected podcast hero uses Audiobookshelf cover artwork

**Reason**: The selected-show hero is removed; the hero presents the selected episode, with the parent
show's cover.
**Migration**: The `The episode hero uses the parent show's cover artwork` requirement in
`audiobookshelf-podcast-library-ui` defines artwork, hero facts, and images-disabled behavior.

### Requirement: Selected podcasts map TV season selection to played-state filters

**Reason**: The season-selector position and its hero-Workspace semantics are removed; played-state
filtering is the panel pill bar.
**Migration**: The `Podcast tab uses one state-and-show pill selector` requirement in
`audiobookshelf-podcast-library-ui` defines `All` / `Unplayed` / `Played` semantics.

### Requirement: Downloaded episodes use the TV episode-list presentation

**Reason**: Episodes no longer render inside a hero Workspace as TV-parallel episode tables; they are the
tab's primary list rows with age-group headings.
**Migration**: The `Downloaded episodes use the shared list row presentation` requirement in this delta
defines row semantics, heading insertion, and stale-result discipline.

## MODIFIED Requirements

### Requirement: Personalized shelves are absent from the podcast tab

The Audiobookshelf podcast tab SHALL NOT render or navigate personalized shelf data, and shelf data SHALL
NOT affect show order, selection, scrolling, hit testing, or pagination.

#### Scenario: Catalog includes personalized shelves

- **WHEN** Audiobookshelf returns personalized shelf data
- **THEN** the podcast tab's pill row, episode list and hero SHALL remain unaffected

## ADDED Requirements

### Requirement: Podcast libraries use the flat episode feed presentation

An Audiobookshelf podcast library SHALL browse downloaded episodes directly: the list SHALL render the five
Feeds age-group headings (`New`, `Recent`, `Older than two weeks`, `Older than a month`, `Unknown date`)
with one selectable episode row per matching episode beneath each heading, scoped by the single active pill.
The selected episode's hero SHALL occupy the Wide Hero pane when wide geometry fits; in non-Wide geometry
episodes remain ordinary rows and Enter plays the selected episode. The podcast tab SHALL NOT render a show
browser, a show hero workspace, an inline detail block, or a constituent-list modal, and SHALL NOT define a
surface-specific geometry rule. The Wide list rail holds the pill row above the grouped episode list; the
hero holds the selected episode.

The following substitutions SHALL be the only domain changes to that composition:

| Feeds tab | Audiobookshelf podcast tab |
|---|---|
| Feed entry | Downloaded episode |
| Feed subscription pill | Podcast show pill |
| Watched filter pills | `All` / `Unplayed` / `Played` state pills |
| Feed age-group headings | Feed age-group headings (identical) |

All other observable layout behavior SHALL match the Feeds tab, including the single pill row, list
columns, split-row context and title roles, selected-cell treatment, focus styling, scrolling, loading
placeholder stability, and the read-only Wide hero beside a single-column browser.

#### Scenario: Podcast library is displayed

- **WHEN** an Audiobookshelf podcast library and the Feeds tab are displayed at the same terminal dimensions and image setting
- **THEN** both tabs SHALL use the same shared wide or non-Wide presentation for their available geometry
- **THEN** the podcast tab SHALL render episode rows in the browser positions occupied by feed entries
- **AND** the selected episode's hero SHALL occupy the Wide hero pane beside the single-column browser

#### Scenario: Age groups match Feeds criteria

- **WHEN** episode rows are grouped
- **THEN** the headings and their day boundaries SHALL be identical to the Feeds tab's groups, including `Unknown date` for episodes without a publish date

#### Scenario: Podcast episode selection changes

- **WHEN** the user moves selection between episode rows
- **THEN** the hero SHALL update to the newly selected episode
- **AND** the episode list SHALL retain provider-native selection identity across loaded-page changes

#### Scenario: Selected episode moves through the non-Wide browser

- **WHEN** the selected episode moves through the non-Wide browser
- **THEN** the episode remains an ordinary selectable row and Enter plays it

#### Scenario: Terminal width crosses the shared breakpoint

- **WHEN** the podcast tab crosses the shared width breakpoint
- **THEN** it recomputes Wide versus non-Wide library presentation rather than changing a detail layout column count

#### Scenario: Terminal height cannot fit the hero

- **WHEN** the Wide hero cannot fit with a usable list
- **THEN** the podcast tab uses the non-Wide presentation and the browser retains the available area

### Requirement: Downloaded episodes use the shared list row presentation

Downloaded podcast episodes SHALL render as selectable rows in the shared media-list presentation with the
same row height, column geometry, truncation, focused and unfocused colors, cursor styling, played and
in-progress semantic state, and available row budget as the Feeds tab's entry rows. Because a view may span
podcasts, every episode row SHALL be a split row carrying its parent podcast's name in the context role
followed by the episode title, plus the episode's duration. Non-selectable age-group heading and spacer rows
SHALL be inserted into the list flow without changing episode indices or stable episode targeting. The
podcast implementation SHALL substitute podcast-native episode data without converting it to an Emby item.

#### Scenario: Active view has episodes

- **WHEN** the active pill view has matching episodes
- **THEN** the list renders one selectable row per episode with provider-native identities, its parent podcast's name, the episode title, and its duration
- **AND** age-group heading and spacer rows render without shifting episode targeting

#### Scenario: Active view is empty or loading

- **WHEN** the active pill view has no matching episodes or its episodes are still loading
- **THEN** the list SHALL show its scoped empty or loading state without disturbing the pill row or hero

#### Scenario: A result arrives after selection moved

- **WHEN** episodes arrive after the user has changed the active pill or selection
- **THEN** mbv SHALL NOT replace the current view's rows with content that no longer matches the active pill

### Requirement: Flat episode views load from the paged show list and per-show episode fan-out

mbv SHALL fill the podcast tab's views from Audiobookshelf's 2.36 podcast reads: the bounded page fetch of
the library's items, which returns the library's podcast shows, and the per-show expanded-item fetch, which
returns that show's downloaded episodes. Episodes SHALL be requested lazily for the active pill: a show-pill
view SHALL require that show only, and a state-pill view SHALL require every subscribed show, requested with
a bounded number of requests in flight and appended as they arrive. Each show SHALL be fetched at most once
per session and retained until a refresh; there SHALL be no episode cap. Episode identity SHALL remain the
Audiobookshelf Service kind plus `libraryItemId` and `episodeId`, and every result SHALL be reconciled with
the Service setup generation that initiated it, as catalog results already are. A publish date SHALL be
normalised to unix seconds at the wire boundary — the Service reports epoch milliseconds, and existing
fixtures carried ISO text — so that age grouping and the hero's date agree; a missing or unreadable date
SHALL group as `Unknown date`.

#### Scenario: A state pill fills progressively

- **WHEN** the user activates `All`, `Unplayed` or `Played`
- **THEN** mbv SHALL request every subscribed show's episodes with bounded concurrency
- **AND** each show's episodes SHALL appear in the view as they arrive, without a visible reload of already-listed rows
- **AND** age-group headings remain in place while their member rows fill in

#### Scenario: A show pill costs one request

- **WHEN** the user activates a show pill whose episodes have not been fetched
- **THEN** mbv SHALL request that show's episodes only

#### Scenario: Returning to a fetched view

- **WHEN** the user returns to a pill whose shows were already fetched in this session
- **THEN** the view SHALL render from the retained episodes without re-requesting them

#### Scenario: Selection survives arrivals

- **WHEN** episodes arrive while an episode is selected
- **THEN** the selected episode keeps its row and cursor position when its identity remains present

#### Scenario: Refresh follows tab activation and the refresh key

- **WHEN** the user activates the podcast tab or presses the refresh key
- **THEN** the active pill's required shows SHALL be re-requested and their episodes replaced

#### Scenario: Stale result after Service replacement

- **WHEN** a show-list page or episode fetch initiated for the previous Audiobookshelf server arrives after Service replacement
- **THEN** mbv SHALL ignore it without changing the current views, selection, or Service state

#### Scenario: Refresh removes the selected episode

- **WHEN** the episode list refreshes and the selected episode identity is no longer present
- **THEN** mbv SHALL select the nearest valid episode or the active view's empty state

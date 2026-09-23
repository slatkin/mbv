# audiobookshelf-podcast-browsing Specification

## Purpose
Defines read-only discovery and browsing of Audiobookshelf podcast libraries, shows, downloaded episodes, progress, artwork, and personalized shelves before Audiobookshelf playback is introduced.

## Requirements

### Requirement: Ready Audiobookshelf discovers accessible podcast libraries
After Audiobookshelf becomes Ready, mbv SHALL discover the authenticated user's accessible Audiobookshelf libraries using the Audiobookshelf 2.36 API contract. It SHALL expose podcast libraries for browsing through this capability and book libraries for browsing through the `audiobookshelf-book-browsing` capability.

#### Scenario: User has accessible podcast libraries
- **WHEN** Audiobookshelf becomes Ready for a user with one or more accessible podcast libraries
- **THEN** mbv SHALL load those podcast libraries without waiting for Emby or Feeds
- **THEN** each discovered podcast library SHALL become available as a content tab

#### Scenario: User has only audiobook libraries
- **WHEN** Audiobookshelf becomes Ready for a user whose accessible libraries are all book libraries
- **THEN** Audiobookshelf SHALL remain Ready
- **THEN** mbv SHALL add a content tab for each book library through the `audiobookshelf-book-browsing` capability rather than adding no tab

#### Scenario: Audiobookshelf is the only configured content Service
- **WHEN** mbv starts with configured Audiobookshelf content and no configured Emby Service or feed subscriptions
- **THEN** mbv SHALL enter its ordinary content UI rather than opening Services settings as though no content Service were configured
- **THEN** Audiobookshelf initialization and discovery SHALL occur for both bare-mode and attached Local daemon clients

#### Scenario: Catalog request explicitly rejects the credential
- **WHEN** an authenticated catalog request explicitly rejects the persisted Audiobookshelf credential
- **THEN** Audiobookshelf SHALL enter Needs authentication through the existing Service lifecycle
- **THEN** mbv SHALL remove its Audiobookshelf tabs and catalog content while preserving non-secret setup

#### Scenario: Catalog request is unavailable or incompatible
- **WHEN** library discovery cannot complete because the server is unavailable or does not satisfy the required Audiobookshelf 2.36 contract
- **THEN** mbv SHALL present Audiobookshelf as unavailable with a concise retryable or compatibility result
- **THEN** mbv SHALL preserve the configured setup and credential and SHALL NOT use an older-server fallback

### Requirement: Podcast libraries are peer tabs with provider-specific behavior
Each accessible Audiobookshelf podcast library SHALL appear as a peer tab alongside Home, Emby libraries, and Feeds. Selecting an Audiobookshelf tab SHALL dispatch only Audiobookshelf browsing behavior and SHALL NOT fall through to Emby library actions.

#### Scenario: User switches among content tabs
- **WHEN** the user navigates across Home, Emby library, Audiobookshelf podcast library, and Feeds tabs that are present
- **THEN** each tab SHALL retain its correct identity, title, and provider-specific selection state
- **THEN** tab navigation by keyboard or mouse SHALL select the same ordered destination

#### Scenario: User invokes an Emby-specific action from an Audiobookshelf tab
- **WHEN** an Audiobookshelf podcast library is selected
- **THEN** Emby-specific playlist, watched-state, shuffle, route, search, and context-menu actions SHALL NOT operate on the Audiobookshelf selection

#### Scenario: Podcast library is loading or empty
- **WHEN** an Audiobookshelf tab has not finished loading shows or contains no shows
- **THEN** mbv SHALL render a provider-specific loading, error, or empty state without indexing an Emby library

### Requirement: Episode progress is read-only and identity-qualified
mbv SHALL display the authenticated user's Audiobookshelf progress for downloaded podcast episodes using `libraryItemId` and `episodeId`. Catalog browsing SHALL NOT write, infer, or periodically report progress.

#### Scenario: Episode has listening progress
- **WHEN** Audiobookshelf reports current time or completion state for a downloaded episode
- **THEN** mbv SHALL display the corresponding resume position or finished state on that episode

#### Scenario: Episode has no listening progress
- **WHEN** no progress record exists for a downloaded episode
- **THEN** mbv SHALL display it as unstarted rather than borrowing progress from another show or episode

#### Scenario: Progress changes outside mbv while the tab remains open
- **WHEN** progress changes on the server while the Audiobookshelf Socket.IO connection is authenticated and the tab remains open
- **THEN** mbv SHALL update the displayed progress for the matching episode from the resulting `user_item_progress_updated` event, without requiring an explicit REST refresh

#### Scenario: Progress changes while the socket is disconnected
- **WHEN** progress changes on the server while the Audiobookshelf Socket.IO connection is not currently authenticated
- **THEN** mbv MAY continue displaying the last REST-loaded value until the socket reconnects or an explicit REST refresh occurs

### Requirement: Podcast artwork is authenticated and Service-scoped
mbv SHALL fetch Audiobookshelf podcast artwork through the configured Service credential without exposing that credential in cache keys, logs, user-visible errors, or cross-Service state. Artwork state SHALL be isolated from Emby and from a replacement Audiobookshelf server.

#### Scenario: Show artwork is available
- **WHEN** a visible podcast show has authenticated cover artwork
- **THEN** mbv SHALL display it through the configured terminal image protocol and cache it under Service-qualified identity

#### Scenario: Artwork is absent or images are disabled
- **WHEN** a show has no cover or terminal images are disabled
- **THEN** the podcast browser SHALL remain fully usable with its text and placeholder presentation

#### Scenario: Audiobookshelf server is replaced
- **WHEN** the user confirms Audiobookshelf Service replacement
- **THEN** cached artwork belonging to the previous server SHALL NOT be displayed for items from the replacement server

### Requirement: Personalized shelves appear only as podcast Latest

The Audiobookshelf podcast tab SHALL NOT render or navigate personalized shelf data except the `Newest Episodes` shelf, which SHALL be used only by that library's `Latest` pill. Other shelf data SHALL
NOT affect show order, selection, scrolling, hit testing, or pagination.

#### Scenario: Catalog includes personalized shelves

- **WHEN** Audiobookshelf returns personalized shelf data
- **THEN** the podcast tab's state and show pills, episode list and hero SHALL remain unaffected
- **AND** only the `Latest` pill consumes the `Newest Episodes` shelf

### Requirement: Catalog results obey the current Service lifecycle
Every asynchronous Audiobookshelf catalog, detail, progress, shelf, and artwork result SHALL be reconciled with the Service setup generation that initiated it. Replacement, removal, authentication rejection, or a newer setup generation SHALL prevent old-server data from becoming visible.

#### Scenario: Stale result arrives after replacement
- **WHEN** a result initiated for the previous Audiobookshelf server arrives after Service replacement
- **THEN** mbv SHALL ignore it without changing current tabs, selection, progress, shelves, artwork, or Service state

#### Scenario: User removes Audiobookshelf
- **WHEN** Audiobookshelf Service removal is confirmed
- **THEN** mbv SHALL remove its podcast tabs and clear its in-memory catalog, progress, shelf, loading, and artwork state
- **THEN** Emby and Feeds content SHALL remain unaffected

### Requirement: Podcast activation starts supported local playback
Downloaded podcast episodes SHALL support ordinary play and enqueue activation through the Audiobookshelf podcast playback capability. Non-episode rows and unavailable episodes SHALL retain selection without queue or playback side effects.

#### Scenario: User plays a downloaded podcast episode
- **WHEN** the user invokes the ordinary play action on a selected downloaded episode
- **THEN** mbv SHALL submit that provider-native episode through the ordinary queue and owner-admission boundary

#### Scenario: User enqueues a downloaded podcast episode
- **WHEN** the user invokes the ordinary enqueue action on a selected downloaded episode
- **THEN** mbv SHALL add it to the selected Composed or eligible Bound queue without starting it

#### Scenario: User activates a non-episode row
- **WHEN** the selected Audiobookshelf row does not identify an available downloaded episode
- **THEN** mbv SHALL retain selection without creating a QueueItem or opening a playback session

### Requirement: Podcast browsing reaches playback only through explicit episode actions
Catalog discovery, pagination, detail loading, progress hydration, artwork, filtering, and navigation SHALL remain read-oriented and SHALL NOT themselves create queue items, resolve streams, or open playback sessions. Only an explicit play or enqueue action on a downloaded episode SHALL cross into the Audiobookshelf podcast playback capability.

#### Scenario: User browses podcast catalog surfaces
- **WHEN** the user discovers libraries, pages shows, expands episodes, views progress or artwork, changes filters, or moves selection
- **THEN** no Audiobookshelf media SHALL enter a Composed or Bound queue
- **THEN** no Audiobookshelf playback lifecycle request SHALL occur

#### Scenario: User explicitly submits an episode
- **WHEN** the user invokes play or enqueue on a selected downloaded episode
- **THEN** browsing SHALL provide its provider-native identity and snapshot metadata to the playback boundary
- **THEN** browsing state SHALL NOT receive or retain the Service credential, playback `sessionId`, resolved media URL, or request headers

### Requirement: Daemon-acknowledged progress reconciles client browse state
When an attached client applies a daemon owner's acknowledged Audiobookshelf progress event for the current setup generation, it SHALL update the displayed browse progress for the matching `libraryItemId` and `episodeId` and re-evaluate episode filters (such as Unplayed) accordingly, without polling, an explicit REST refresh, or Socket.IO. A superseded-generation event SHALL leave browse state unchanged.

#### Scenario: Acknowledged completion updates the Unplayed filter
- **WHEN** a capable client applies an acknowledged progress event marking a downloaded episode finished for the current generation
- **THEN** that episode SHALL present as finished and SHALL be excluded from the Unplayed filter

#### Scenario: Acknowledged position updates the resume state
- **WHEN** a capable client applies an acknowledged position below completion for the current generation
- **THEN** the matching episode SHALL display the corresponding resume position

#### Scenario: Superseded-generation acknowledgement is ignored for browse
- **WHEN** a received acknowledged progress event belongs to a replaced or removed setup generation
- **THEN** the client SHALL leave displayed browse progress and filters unchanged

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
podcasts, every episode row SHALL be a split row carrying its parent podcast's name in the context role,
with no duration time. The episode list SHALL declare the shared reveal-on-selection title policy, so an
episode row paints only its parent podcast's name until it becomes the list's selection; the selected row
then paints the parent podcast's name followed by the episode title as one marqueed title. A row whose
episode declares a publish date SHALL carry it in the shared fixed right-aligned date gutter, formatted as
day and abbreviated month (`17 Sep`), so two episodes of one show stay distinguishable at rest; a row whose
episode declares none SHALL reserve no gutter. Non-selectable age-group heading and spacer rows SHALL be
inserted into the list flow without changing episode indices or stable episode targeting. The podcast
implementation SHALL substitute podcast-native episode data without converting it to an Emby item.

#### Scenario: Active view has episodes

- **WHEN** the active pill view has matching episodes
- **THEN** the list renders one selectable row per episode with provider-native identities and its parent podcast's name
- **AND** age-group heading and spacer rows render without shifting episode targeting

#### Scenario: The episode title appears on the selected row only

- **WHEN** an episode row is not the podcast list's current selection
- **THEN** the row paints its parent podcast's name and no part of the episode title
- **AND** when that row becomes the selection, it paints the parent podcast's name followed by the episode title, marqueed while the list is focused

#### Scenario: Episode rows carry their publish date at rest

- **WHEN** a podcast episode row's episode declares a publish date
- **THEN** the row paints that date in the right-aligned gutter as day and abbreviated month
- **AND** an episode row with no declared publish date paints no gutter and keeps the full title slot

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

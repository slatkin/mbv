# audiobookshelf-podcast-browsing Specification

## Purpose
Defines read-only discovery and browsing of Audiobookshelf podcast libraries, shows, downloaded episodes, progress, artwork, and personalized shelves before Audiobookshelf playback is introduced.
## Requirements
### Requirement: Ready Audiobookshelf discovers accessible podcast libraries
After Audiobookshelf becomes Ready, mbv SHALL discover the authenticated user's accessible Audiobookshelf libraries using the Audiobookshelf 2.36 API contract. It SHALL expose podcast libraries for browsing and SHALL NOT expose audiobook libraries during this milestone.

#### Scenario: User has accessible podcast libraries
- **WHEN** Audiobookshelf becomes Ready for a user with one or more accessible podcast libraries
- **THEN** mbv SHALL load those podcast libraries without waiting for Emby or Feeds
- **THEN** each discovered podcast library SHALL become available as a content tab

#### Scenario: User has only audiobook libraries
- **WHEN** Audiobookshelf becomes Ready for a user whose accessible libraries are all audiobook libraries
- **THEN** Audiobookshelf SHALL remain Ready
- **THEN** mbv SHALL add no Audiobookshelf content tab

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

### Requirement: Podcast shows load incrementally with stable selection
mbv SHALL list podcast shows from the selected Audiobookshelf library using bounded pagination. Show identity SHALL be the Audiobookshelf Service kind plus `libraryItemId`, and refresh or page loading SHALL preserve the selected show when that identity remains present.

#### Scenario: User reaches the loaded page boundary
- **WHEN** more podcast shows are available beyond the currently loaded page and navigation approaches the boundary
- **THEN** mbv SHALL request the next bounded page and append each show at most once
- **THEN** existing shows SHALL remain navigable while the request is pending

#### Scenario: Show list refresh retains the selected show
- **WHEN** the show list refreshes and the selected `libraryItemId` remains in the result
- **THEN** mbv SHALL restore selection to that show regardless of its new positional index

#### Scenario: Show list refresh removes the selected show
- **WHEN** the show list refreshes and the selected `libraryItemId` is no longer present
- **THEN** mbv SHALL select the nearest valid show or the library's empty state

### Requirement: Selected shows expand downloaded episodes inline
Selecting a podcast show SHALL load and display its downloaded episodes inline within the library view. Episode identity SHALL be the Audiobookshelf Service kind plus `libraryItemId` and `episodeId`.

#### Scenario: Selected show has downloaded episodes
- **WHEN** the selected show's expanded Audiobookshelf response contains downloaded episodes
- **THEN** mbv SHALL display those episodes inline beneath the selected show with title, publication information, duration, and available listening state

#### Scenario: Selected show has no downloaded episodes
- **WHEN** the selected show's expanded response contains no downloaded episodes
- **THEN** mbv SHALL show an inline empty-episodes state without treating remote undownloaded feed episodes as playable catalog entries

#### Scenario: User selects and activates an episode
- **WHEN** the user moves selection onto an inline episode and presses the ordinary activation key
- **THEN** mbv SHALL retain the episode selection without starting playback, enqueueing an item, or opening a playback session

#### Scenario: User changes shows while detail is loading
- **WHEN** an expanded-show result completes after the user has selected a different show
- **THEN** mbv SHALL NOT replace the currently displayed inline episodes with the stale selection's episodes

### Requirement: Episode progress is read-only and identity-qualified
mbv SHALL display the authenticated user's Audiobookshelf progress for downloaded podcast episodes using `libraryItemId` and `episodeId`. Catalog browsing SHALL NOT write, infer, or periodically report progress.

#### Scenario: Episode has listening progress
- **WHEN** Audiobookshelf reports current time or completion state for a downloaded episode
- **THEN** mbv SHALL display the corresponding resume position or finished state on that episode

#### Scenario: Episode has no listening progress
- **WHEN** no progress record exists for a downloaded episode
- **THEN** mbv SHALL display it as unstarted rather than borrowing progress from another show or episode

#### Scenario: Progress changes outside mbv while the tab remains open
- **WHEN** progress changes on the server without an explicit REST refresh
- **THEN** mbv MAY continue displaying the last REST-loaded value because live Socket.IO refresh is outside this capability

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

### Requirement: Personalized podcast shelves remain library-local
mbv SHALL display Audiobookshelf 2.36 personalized podcast shelves within the corresponding podcast library. Shelf entries SHALL resolve to provider-qualified shows or downloaded episodes and SHALL NOT be merged into the cross-Service Home tab.

#### Scenario: Library has personalized show and episode shelves
- **WHEN** Audiobookshelf returns supported personalized shelves for the selected podcast library
- **THEN** mbv SHALL render each supported shelf with its server-provided label and ordered entries
- **THEN** selecting a shelf entry SHALL navigate to its corresponding show or downloaded episode within that library

#### Scenario: Shelf refers to unavailable content
- **WHEN** a shelf entry cannot be resolved to an accessible show or downloaded episode
- **THEN** mbv SHALL omit or mark that entry unavailable without failing the rest of the library

#### Scenario: Home tab is rendered
- **WHEN** Audiobookshelf personalized shelves have been loaded
- **THEN** the Home tab SHALL remain unchanged and SHALL NOT contain those shelves

### Requirement: Catalog results obey the current Service lifecycle
Every asynchronous Audiobookshelf catalog, detail, progress, shelf, and artwork result SHALL be reconciled with the Service setup generation that initiated it. Replacement, removal, authentication rejection, or a newer setup generation SHALL prevent old-server data from becoming visible.

#### Scenario: Stale result arrives after replacement
- **WHEN** a result initiated for the previous Audiobookshelf server arrives after Service replacement
- **THEN** mbv SHALL ignore it without changing current tabs, selection, progress, shelves, artwork, or Service state

#### Scenario: User removes Audiobookshelf
- **WHEN** Audiobookshelf Service removal is confirmed
- **THEN** mbv SHALL remove its podcast tabs and clear its in-memory catalog, progress, shelf, loading, and artwork state
- **THEN** Emby and Feeds content SHALL remain unaffected

### Requirement: Podcast browsing remains outside playback boundaries
This capability SHALL remain read-only catalog integration. It SHALL NOT create Audiobookshelf queue items, resolve streams, open or synchronize playback sessions, write progress, connect to Socket.IO, alter ctrl, or add Audiobookshelf support to any Player owner.

#### Scenario: User browses every Audiobookshelf catalog surface
- **WHEN** the user discovers libraries, pages shows, expands episodes, views progress and artwork, or navigates personalized shelves
- **THEN** no Audiobookshelf media SHALL enter a Composed or Bound queue
- **THEN** no Audiobookshelf playback lifecycle or daemon transport request SHALL occur


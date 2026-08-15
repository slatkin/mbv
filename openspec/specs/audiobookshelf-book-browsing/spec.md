# audiobookshelf-book-browsing Specification

## Purpose

Defines read-only discovery and browsing of Audiobookshelf book libraries — library-to-tab exposure, author-surname grouping, the Music-style hero-on-left composition, chapter display, and read-only progress — distinct from the podcast tab's TV-style browsing.

## Requirements

### Requirement: Book libraries are peer tabs with provider-specific behavior
Each accessible Audiobookshelf book library SHALL appear as a peer tab alongside Home, Emby libraries, Audiobookshelf podcast libraries, and Feeds, in the server's library order. Selecting a book tab SHALL dispatch only book browsing behavior and SHALL NOT fall through to Emby or Audiobookshelf podcast actions.

#### Scenario: Book and podcast libraries interleave in server order
- **WHEN** an Audiobookshelf server exposes both book and podcast libraries
- **THEN** mbv SHALL present their tabs in the order `/api/libraries` returns them
- **THEN** mbv SHALL NOT group or reorder tabs by `media_type`

#### Scenario: User invokes a podcast- or Emby-specific action from a book tab
- **WHEN** an Audiobookshelf book library is selected
- **THEN** podcast played-state filtering, playlist, watched-state, shuffle, route, search, and Emby context-menu actions SHALL NOT operate on the book selection

### Requirement: Books load incrementally, grouped and sorted by author surname
mbv SHALL list books from the selected Audiobookshelf book library using bounded pagination, grouped and sorted by author surname only. Book identity SHALL be the Audiobookshelf Service kind plus `libraryItemId`, and refresh or page loading SHALL preserve the selected book when that identity remains present.

#### Scenario: Author surname determines sort position
- **WHEN** a book has one or more listed authors
- **THEN** mbv SHALL sort it using the first-listed author's surname, extracted from the raw author credit
- **THEN** remaining authors on a multi-author book SHALL NOT participate in the sort key

#### Scenario: Surname extraction fails
- **WHEN** author-name parsing cannot extract a surname from the raw credit
- **THEN** mbv SHALL fall back to the raw author credit string as the sort key
- **THEN** the book SHALL remain grouped and browsable rather than excluded

#### Scenario: Book list refresh retains the selected book
- **WHEN** the book list refreshes and the selected `libraryItemId` remains in the result
- **THEN** mbv SHALL restore selection to that book regardless of its new positional index or group

### Requirement: Book libraries use the Music tab composition
An Audiobookshelf book library SHALL use the same outer composition as the Music tab at the same terminal dimensions and image setting: a hero-on-left, list-on-right two-column layout above `TWO_COLUMN_THRESHOLD`, falling back to hero-on-top below it. This SHALL NOT be the always-vertical hero the TV Shows and Audiobookshelf podcast tabs use.

The following substitutions SHALL be the only domain changes to that composition:

| Music tab | Audiobookshelf book tab |
|---|---|
| Album | Book |
| Album cover | Audiobookshelf book cover |
| Track list | Chapter list |
| Artist grouping | Author-surname grouping |

All other observable layout behavior SHALL match the Music tab, including hero placement, content padding, image slot, row budgeting, selected-cell treatment, focus styling, scrolling, and narrow-terminal fallback.

#### Scenario: Terminal width crosses the two-column threshold
- **WHEN** the book tab crosses `TWO_COLUMN_THRESHOLD`
- **THEN** the layout SHALL switch between hero-on-left and hero-on-top at the same width the Music tab does

### Requirement: The selected book hero shows an inline progress percentage
The selected book hero SHALL place the selected book's Audiobookshelf cover in the same image slot as the Music hero's album cover, and SHALL show the book's listening progress as an inline `%` or `Finished` span in the hero meta, in the same style the podcast tab uses for episode progress. A resume-emphasizing hero treatment is out of scope for this capability.

#### Scenario: Selected book has listening progress
- **WHEN** the selected book has Audiobookshelf progress
- **THEN** the hero SHALL display the corresponding `%` or `Finished` span
- **THEN** the image, title, and author metadata positions SHALL remain unchanged by the presence of progress

#### Scenario: Selected book has no listening progress
- **WHEN** no progress record exists for the selected book
- **THEN** the hero SHALL display it as unstarted rather than borrowing progress from another book

### Requirement: Chapters render as first-class rows in the persistent list
The book tab's persistent list (the Music track list's analog) SHALL render one row per chapter from the selected book's Audiobookshelf `chapters[]`, using the book-relative chapter title and duration. Chapter rows SHALL use provider-native identity and SHALL NOT be converted to an Emby or podcast episode row shape.

#### Scenario: Selected book has chapters
- **WHEN** the selected book has one or more chapters
- **THEN** mbv SHALL render each chapter as a selectable row in the persistent list area

#### Scenario: Selected book has no chapter metadata
- **WHEN** the selected book has no `chapters[]` entries
- **THEN** mbv SHALL render its `audioFiles` as the persistent list rows instead, without an empty or broken list state

### Requirement: Book progress is read-only and identity-qualified
mbv SHALL display the authenticated user's Audiobookshelf progress for a book using only `libraryItemId` — books have no episode identity. Catalog browsing SHALL NOT write, infer, or periodically report progress.

#### Scenario: Progress changes outside mbv while the tab remains open
- **WHEN** progress changes on the server without an explicit REST refresh
- **THEN** mbv MAY continue displaying the last REST-loaded value because live Socket.IO refresh is outside this capability

### Requirement: Book artwork is authenticated and Service-scoped
mbv SHALL fetch Audiobookshelf book cover artwork through the configured Service credential without exposing that credential in cache keys, logs, user-visible errors, or cross-Service state. Artwork state SHALL be isolated from Emby, from Audiobookshelf podcast artwork, and from a replacement Audiobookshelf server.

#### Scenario: Artwork is absent or images are disabled
- **WHEN** a book has no cover or terminal images are disabled
- **THEN** the book browser SHALL remain fully usable with its text and placeholder presentation

### Requirement: Catalog results obey the current Service lifecycle
Every asynchronous book catalog, chapter, progress, and artwork result SHALL be reconciled with the Service setup generation that initiated it. Replacement, removal, authentication rejection, or a newer setup generation SHALL prevent old-server data from becoming visible.

#### Scenario: Stale result arrives after replacement
- **WHEN** a result initiated for the previous Audiobookshelf server arrives after Service replacement
- **THEN** mbv SHALL ignore it without changing current tabs, selection, progress, or artwork

### Requirement: Book browsing reaches playback only through explicit book actions
Catalog discovery, pagination, chapter display, progress hydration, artwork, and navigation SHALL remain read-oriented and SHALL NOT themselves create queue items, resolve streams, or open playback sessions. Only an explicit play or enqueue action on a book, or a seek action on a chapter row of the active book, SHALL cross into the `audiobookshelf-book-playback` capability.

#### Scenario: User browses the book catalog
- **WHEN** the user discovers libraries, pages books, views chapters, progress, or artwork, or moves selection
- **THEN** no Audiobookshelf book SHALL enter a Composed or Bound queue
- **THEN** no Audiobookshelf playback lifecycle request SHALL occur

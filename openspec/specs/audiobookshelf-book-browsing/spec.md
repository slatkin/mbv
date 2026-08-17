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
mbv SHALL list books from the selected Audiobookshelf book library using bounded pagination, grouped and sorted by author surname only, and further bucketed into alphabetical author-surname ranges (e.g. A-C, D-F) for pill-filtered browsing. Book identity SHALL be the Audiobookshelf Service kind plus `libraryItemId`, and refresh or page loading SHALL preserve the selected book when that identity remains present.

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
- **THEN** mbv SHALL restore selection to that book regardless of its new positional index or bucket

#### Scenario: Surname buckets omit empty ranges
- **WHEN** the sorted book list is grouped for browsing
- **THEN** mbv SHALL partition it into contiguous alphabetical author-surname ranges
- **THEN** a range with no books in the current library SHALL NOT produce an empty, selectable bucket

### Requirement: Book libraries use the hero-on-left arrangement

An Audiobookshelf book library SHALL use the hero-on-left arrangement, the same arrangement grouped
Music uses: a persistent hero pane with chapters below, beside a persistent single-column book
browser with surname-bucket pills, at or above the shared breakpoint; falling back to hero-on-top
with a single list column below it. Both panes SHALL remain visible at all times. The book tab SHALL
obtain this arrangement from the shared definition rather than by reproducing the Music tab's
implementation, and SHALL NOT evaluate the breakpoint itself.

The following substitutions SHALL be the only domain changes to that arrangement. They are DATA
the book tab supplies — the arrangement renders whatever hero content, list rows, and pills the
screen hands it — so they are not presentation declarations. The book tab's single declaration of
differences SHALL cover only the presentation fields (image shape, metadata lines and order, colour
variant, element presence, and the `image source` for the cover):

| Hero-on-left default | Audiobookshelf book tab |
|---|---|
| Album | Book |
| Album cover | Audiobookshelf book cover |
| Track list (persistent hero pane) | Chapter list (persistent hero pane) |
| Artist grouping pills and filter drill | Author-surname bucket pills and filter drill |
| Album list within artist filter | Book list within surname-bucket filter |
| Left/right arrow toggles pane focus | Left/right arrow toggles pane focus |

All other observable layout behavior SHALL be that of the hero-on-left arrangement, including hero
placement, content padding, image slot, row budgeting, selected-cell treatment, focus styling,
scrolling, and narrow fallback.

#### Scenario: Terminal width crosses the two-column threshold

- **WHEN** the book tab crosses the shared breakpoint
- **THEN** the layout SHALL switch between hero-on-left and hero-on-top at the same width every other
  hero-on-left screen does

#### Scenario: Hero follows the browser cursor

- **WHEN** the book browser cursor moves to another book
- **THEN** the hero SHALL update to that book without an Enter/open action
- **THEN** the right-pane browser SHALL remain visible

#### Scenario: A surname pill filters the browser

- **WHEN** the user selects an author-surname bucket pill
- **THEN** the right-pane book list SHALL contain only books in that bucket until another bucket is
  selected

#### Scenario: Arrow focus leaves both panes visible

- **WHEN** the user presses left or right while the book tab is focused
- **THEN** focus SHALL toggle between the chapter list and right-pane browser
- **THEN** neither pane SHALL be hidden or replaced

#### Scenario: The hero-on-left arrangement changes

- **WHEN** the hero-on-left arrangement's presentation is changed
- **THEN** the book tab renders the change identically to grouped Music, without an individual edit

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
The book tab's persistent list (the Music track list's analog) SHALL render one row per chapter from the selected book's Audiobookshelf `chapters[]`, using the book-relative chapter title and duration. Chapter rows SHALL use provider-native identity and SHALL NOT be converted to an Emby or podcast episode row shape. Chapter or audio-file detail SHALL be fetched as soon as the browser cursor moves onto a book, mirroring the Music tab's eager track fetch, rather than only after an explicit book-open action.

#### Scenario: Selected book has chapters
- **WHEN** the selected book has one or more chapters
- **THEN** mbv SHALL render each chapter as a selectable row in the persistent list area

#### Scenario: Selected book has no chapter metadata
- **WHEN** the selected book has no `chapters[]` entries
- **THEN** mbv SHALL render its `audioFiles` as the persistent list rows instead, without an empty or broken list state

#### Scenario: Cursor moves onto an uncached book
- **WHEN** the browser cursor moves onto a book whose chapter/audio-file detail is not yet cached
- **THEN** mbv SHALL fetch that detail immediately, without requiring an explicit book-open action
- **THEN** a fetch already in flight or cached for that book SHALL NOT be re-requested

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

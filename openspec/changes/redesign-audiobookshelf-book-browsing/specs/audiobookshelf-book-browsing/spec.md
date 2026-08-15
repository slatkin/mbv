## MODIFIED Requirements

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

### Requirement: Book libraries use the Music tab composition
An Audiobookshelf book library SHALL use the same outer composition as the Music tab at the same terminal dimensions and image setting: a persistent hero-on-left, persistent browser-on-right two-column layout above `TWO_COLUMN_THRESHOLD`, falling back to hero-on-top below it. Both panes SHALL remain visible and rendered at all times; the composition SHALL NOT replace one pane with the other, and SHALL NOT be the always-vertical hero the TV Shows and Audiobookshelf podcast tabs use, nor the multi-column grid-then-detail-drilldown pattern Emby library tabs use.

The following substitutions SHALL be the only domain changes to that composition:

| Music tab | Audiobookshelf book tab |
|---|---|
| Album | Book |
| Album cover | Audiobookshelf book cover |
| Track list (left pane, persistent) | Chapter list (left pane, persistent) |
| Artist grouping pills, filter drill (right pane) | Alphabetical author-surname-bucket grouping pills, filter drill (right pane) |
| Album list within artist filter (right pane) | Book list within surname-bucket filter (right pane) |
| Left/right arrow toggles pane focus | Left/right arrow toggles pane focus |

All other observable layout behavior SHALL match the Music tab, including hero placement, content padding, image slot, row budgeting, selected-cell treatment, focus styling, scrolling, and narrow-terminal fallback.

#### Scenario: Terminal width crosses the two-column threshold
- **WHEN** the book tab crosses `TWO_COLUMN_THRESHOLD`
- **THEN** the layout SHALL switch between hero-on-left and hero-on-top at the same width the Music tab does

#### Scenario: Hero follows the browser cursor
- **WHEN** the book browser's cursor moves to a different book
- **THEN** the hero SHALL update to that book's cover, title, author, and progress without requiring an Enter or open action
- **THEN** the right-pane browser SHALL remain visible and rendered beside the hero

#### Scenario: A surname pill filters the browser
- **WHEN** the user selects a different author-surname-bucket pill
- **THEN** the right-pane book list SHALL show only books whose surname falls in that bucket
- **THEN** books outside the selected bucket SHALL NOT be reachable by scrolling until a different bucket is selected

#### Scenario: Arrow focus leaves both panes visible
- **WHEN** the user presses left or right arrow while the book tab is focused
- **THEN** input focus SHALL toggle between the hero's chapter list and the right-pane book browser
- **THEN** neither pane SHALL be hidden, replaced, or resized as a result of the focus toggle

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

# audiobookshelf-book-browsing Delta

## MODIFIED Requirements

### Requirement: Chapters render as first-class rows in the persistent list
The book tab's persistent list (the Music track list's analog) SHALL render one row per chapter from the selected book's Audiobookshelf `chapters[]`, using the book-relative chapter title, with no duration time. Chapter rows SHALL use provider-native identity and SHALL NOT be converted to an Emby or podcast episode row shape. Chapter or audio-file detail SHALL be fetched as soon as the browser cursor moves onto a book, mirroring the Music tab's eager track fetch, rather than only after an explicit book-open action.

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

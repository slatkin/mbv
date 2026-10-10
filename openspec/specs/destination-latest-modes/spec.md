# destination-latest-modes Specification

## Purpose

Defines a Latest content mode at each eligible media destination so new additions are browsable there without requiring or duplicating Home sections.

## Requirements

### Requirement: Every eligible destination offers Latest beside its existing selector choices

Every visible Emby library except Music libraries, regardless of collection kind or size, every Audiobookshelf podcast library, and the Feeds tab SHALL offer a `Latest` pill alongside its existing selector choices. Audiobookshelf book libraries and Emby Music libraries SHALL NOT gain a Latest pill. On launch restoration, every destination whose selector offers a `Latest` pill SHALL select `Latest`; its other pill selections are session memory and SHALL NOT persist across a restart. A Grouped Music library's selected group and an Audiobookshelf book library's selected bucket SHALL keep persisting across restart. The selector SHALL support keyboard cycling and mouse selection over the displayed choices.

#### Scenario: Emby libraries retain their choices
- **WHEN** a user opens a Movie, Home Videos, or other visible Emby library
- **THEN** its selector contains Latest as well as its pre-existing choices
- **THEN** the existing entry selection and existing choices remain available

#### Scenario: Music library has no Latest pill
- **WHEN** a user opens a visible Emby Music library
- **THEN** its selector contains no Latest choice

#### Scenario: Podcast and Feeds retain their choices
- **WHEN** a user opens an Audiobookshelf podcast library or Feeds
- **THEN** its selector contains Latest alongside the existing podcast state/show or Feeds group/filter choices
- **THEN** a book library has no Latest choice

#### Scenario: TV keeps its established default
- **WHEN** mbv restarts with a TV library as the restored tab
- **THEN** the library resolves its established count-dependent default (`Latest` above the pill threshold, `All` at or below it) instead of the mode selected in the previous session

#### Scenario: Restart selects Latest
- **WHEN** mbv restarts after a session in which a Latest-bearing destination's letter range, state pill, show pill, or Feeds filter or group was selected
- **THEN** that destination's selector SHALL open on `Latest` when it is the restored tab
- **THEN** the non-Latest pill selection SHALL NOT reappear in any later session

#### Scenario: Exit outside the pill-bearing view
- **WHEN** a session ends while a Latest-bearing Emby library stands inside one of its folders, so its selector row is not painted and the exit snapshot records no pill scope
- **WHEN** mbv restarts
- **THEN** that library's selector SHALL still open on `Latest` when it is the restored tab

#### Scenario: Music and book pills keep persisting
- **WHEN** mbv restarts after a session in which a Grouped Music group or an Audiobookshelf book bucket was selected on the exit tab
- **THEN** the recorded group or bucket SHALL restore through the launch snapshot

### Requirement: Latest uses the destination's existing source independent of Home
Emby TV Latest SHALL present the library's newest episodes; other Emby libraries SHALL present their newest additions using the same library-scoped Latest source previously used on Home. Audiobookshelf podcast Latest SHALL use that library's Newest Episodes shelf; Feeds Latest SHALL use its loaded combined entries newest-first, independent of the current watched filter or subscription selection. The modes SHALL work without visiting Home, without requiring an unrelated Service, and without reading or changing another destination's selection. Empty sources SHALL show an empty Latest mode.

#### Scenario: Non-TV Emby library loads Latest without Home
- **WHEN** a user selects Latest on a Movie, Home Videos, or other Emby library before visiting Home
- **THEN** the library shows its own newest additions, not another library's rows

#### Scenario: Independent Audiobookshelf and Feeds sources
- **WHEN** Emby is unavailable and a podcast library or Feeds has loaded entries
- **THEN** its Latest mode shows its own entries without an Emby error
- **THEN** Feeds Latest includes played entries even when the Feeds watched filter is Unplayed

#### Scenario: No newest items
- **WHEN** an eligible destination's Latest source has no items
- **THEN** its Latest pill remains selectable and its list shows an empty state

### Requirement: Latest retains destination-independent item actions and metadata
Each Latest row SHALL retain the media identity, title and container context, provider date in the canonical right gutter when valid, and direct play/enqueue behavior of its former Home Latest row. Audiobookshelf episodes SHALL retain their parent show and description in selected detail; Feed entries SHALL retain the configured subscription display name when available. No Latest activation SHALL change an unrelated destination's cursor or filter. TV Latest SHALL continue to play playable episodes directly and retain its existing geometry-specific hero behavior.

#### Scenario: Playing from Latest
- **WHEN** a user plays or enqueues a Latest episode, movie, or Feed entry
- **THEN** the action addresses that displayed item by its own Service identity
- **THEN** other destinations' selections remain unchanged

#### Scenario: Row date and context
- **WHEN** a Latest item has a valid added/published date and container context
- **THEN** its row displays that date in the canonical gutter and its container context in the shared title-parts format
- **WHEN** date or context is unavailable
- **THEN** that absent field is omitted rather than fabricated

### Requirement: Latest markers follow their destination pills
The launch window SHALL remain the interval strictly after the previous client launch through the current client launch. Every eligible Latest pill SHALL show an Iris new-content marker when any of that destination's Home `Recently added` items has a valid provider added/published timestamp in that interval. The destination's tab in the tab bar SHALL show the same Iris marker whenever that destination's Latest pill marker would be shown, evaluated from Home's `Recently added` data even while another tab is active and before the destination's Latest list has loaded; adding the tab marker SHALL NOT change any tab's width. The first launch SHALL establish the baseline without markers. Selecting Latest, including when already selected before its items arrive, SHALL acknowledge that destination's marker, on both its pill and its tab, for the rest of the run, keyed by Service-qualified destination identity; refresh and asynchronous replacement SHALL NOT restore it. Items dated after the launch instant SHALL NOT mark during that run.

#### Scenario: New content is visible at its destination
- **WHEN** a library or Feeds has Home `Recently added` items dated inside the launch window and its Latest mode has not been visited
- **THEN** the destination's Latest pill shows the marker and the Continue destination has no corresponding pill

#### Scenario: New content is visible on the library tab
- **WHEN** a library or Feeds has Home `Recently added` items dated inside the launch window, its Latest mode has not been visited, and a different tab is active
- **THEN** that destination's tab in the tab bar shows the Iris marker
- **AND** tabs of destinations with no in-window items, and the Home tab, show no marker
- **AND** every tab keeps the same width it has without a marker

#### Scenario: Unvisited Emby library marks at launch
- **WHEN** an Emby Movies library that has not been opened this run received a movie inside the launch window
- **THEN** its tab shows the Iris marker without its Latest list having been fetched

#### Scenario: Visiting clears marker across refresh
- **WHEN** a user selects a marked Latest pill, or its selected mode receives items asynchronously
- **THEN** the marker clears on both the pill and the destination's tab and remains cleared through subsequent refresh in that run

#### Scenario: Visiting the tab alone does not clear the marker
- **WHEN** a user switches to a marked destination's tab whose entry selection is not Latest, and does not select its Latest pill
- **THEN** the tab and pill markers remain shown

#### Scenario: Window older than fourteen days
- **WHEN** the previous launch was 30 days ago and a library's only in-window item is 20 days old
- **THEN** that library shows no marker, because Home `Recently added` holds only the last 14 days

### Requirement: Sunsetting hidden_latest does not hide Latest
`hidden_latest` SHALL no longer be offered in Settings or honored when determining Latest visibility. Saving configuration SHALL no longer emit `hidden_latest`; an existing configuration value SHALL be ignored without preventing startup or hiding Latest. `hidden_libraries` SHALL retain its existing separate behavior.

#### Scenario: Legacy setting is present
- **WHEN** a user's configuration contains `hidden_latest = ["movies", "feeds"]`
- **THEN** Movies and Feeds still show Latest, and the Settings control for hidden Latest is absent
- **THEN** an ordinary configuration save omits the obsolete key

### Requirement: Destination Latest lists load lazily
Every eligible destination SHALL fetch its full Latest list only when that list is selected, and on refresh while it is selected. This includes restoring a destination at launch on its `Latest` pill. No Service SHALL fetch a destination's full Latest list at startup for a destination that is not selected. Feeds Latest makes no fetch of its own, because it is derived from the Feeds entries already loaded for the Feeds tab.

#### Scenario: Podcast library not opened
- **WHEN** mbv starts with two Audiobookshelf podcast libraries and Home as the restored tab
- **THEN** neither podcast library's Latest list is fetched until the user selects its `Latest` pill

#### Scenario: Restored on Latest
- **WHEN** mbv restarts with an Emby Movies library as the restored tab
- **THEN** that library's Latest list is fetched at launch, because restoration selects `Latest`

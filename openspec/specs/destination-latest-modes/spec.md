# destination-latest-modes Specification

## Purpose

Defines a Latest content mode at each eligible media destination so new additions are browsable there without requiring or duplicating Home sections.

## Requirements

### Requirement: Every eligible destination offers Latest beside its existing selector choices
Every visible Emby library, regardless of collection kind or size, every Audiobookshelf podcast library, and the Feeds tab SHALL offer a `Latest` pill alongside its existing selector choices. Audiobookshelf book libraries SHALL NOT gain a Latest pill. Adding Latest SHALL NOT change the initial selection or saved selection of an existing non-TV destination; TV SHALL retain its existing count-dependent default and saved-mode behavior. The selector SHALL support keyboard cycling and mouse selection over the displayed choices.

#### Scenario: Emby libraries retain their choices
- **WHEN** a user opens a Movie, Music, Home Videos, or other visible Emby library
- **THEN** its selector contains Latest as well as its pre-existing choices
- **THEN** the existing entry selection and existing choices remain available

#### Scenario: Podcast and Feeds retain their choices
- **WHEN** a user opens an Audiobookshelf podcast library or Feeds
- **THEN** its selector contains Latest alongside the existing podcast state/show or Feeds group/filter choices
- **THEN** a book library has no Latest choice

#### Scenario: TV keeps its established default
- **WHEN** a TV library opens above or at/below its pill threshold
- **THEN** its existing Latest or All default respectively remains unchanged

### Requirement: Latest uses the destination's existing source independent of Home
Emby TV Latest SHALL present the library's newest episodes; other Emby libraries SHALL present their newest additions using the same library-scoped Latest source previously used on Home. Audiobookshelf podcast Latest SHALL use that library's Newest Episodes shelf; Feeds Latest SHALL use its loaded combined entries newest-first, independent of the current watched filter or subscription selection. The modes SHALL work without visiting Home, without requiring an unrelated Service, and without reading or changing another destination's selection. Empty sources SHALL show an empty Latest mode.

#### Scenario: Non-TV Emby library loads Latest without Home
- **WHEN** a user selects Latest on a Movie, Music, Home Videos, or other Emby library before visiting Home
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
- **WHEN** a user plays or enqueues a Latest episode, movie, track, or Feed entry
- **THEN** the action addresses that displayed item by its own Service identity
- **THEN** other destinations' selections remain unchanged

#### Scenario: Row date and context
- **WHEN** a Latest item has a valid added/published date and container context
- **THEN** its row displays that date in the canonical gutter and its container context in the shared title-parts format
- **WHEN** date or context is unavailable
- **THEN** that absent field is omitted rather than fabricated

### Requirement: Latest markers follow their destination pills
The launch window SHALL remain the interval strictly after the previous client launch through the current client launch. Every eligible Latest pill SHALL show an Iris new-content marker when any item has a valid provider added/published timestamp in that interval. The first launch SHALL establish the baseline without markers. Selecting Latest, including when already selected before its items arrive, SHALL acknowledge that destination's marker for the rest of the run, keyed by Service-qualified destination identity; refresh and asynchronous replacement SHALL NOT restore it. Items dated after the launch instant SHALL NOT mark during that run.

#### Scenario: New content is visible at its destination
- **WHEN** a library or Feeds receives Latest items dated inside the launch window and its Latest mode has not been visited
- **THEN** the destination's Latest pill shows the marker and Home shows no corresponding pill

#### Scenario: Visiting clears marker across refresh
- **WHEN** a user selects a marked Latest pill, or its selected mode receives items asynchronously
- **THEN** the marker clears and remains cleared through subsequent refresh in that run

### Requirement: Sunsetting hidden_latest does not hide Latest
`hidden_latest` SHALL no longer be offered in Settings or honored when determining Latest visibility. Saving configuration SHALL no longer emit `hidden_latest`; an existing configuration value SHALL be ignored without preventing startup or hiding Latest. `hidden_libraries` SHALL retain its existing separate behavior.

#### Scenario: Legacy setting is present
- **WHEN** a user's configuration contains `hidden_latest = ["movies", "feeds"]`
- **THEN** Movies and Feeds still show Latest, and the Settings control for hidden Latest is absent
- **THEN** an ordinary configuration save omits the obsolete key

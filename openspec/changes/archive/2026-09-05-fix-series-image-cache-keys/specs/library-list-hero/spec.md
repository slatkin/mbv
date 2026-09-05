## Purpose

Extends `library-list-hero` with the Series artwork cache contract the key split
(`0cbd51b7`) requires: each declared Series image chain fetches once, prefetch
warms the painted entry, the loading placeholder shows until that entry lands,
and completion re-pushes the TV workspace.

## MODIFIED Requirements

### Requirement: Hero content is independent of placement

Hero content SHALL be independent of responsive placement. The same surface declaration SHALL supply content to hero-on-left and inline presentations, with only arrangement-specific composition changing. Wide Movies SHALL continue reusing Home's selected-media card rather than maintaining a second Movies-specific left card. No hero content implementation SHALL depend on a separate placement fallback.

#### Scenario: Placement changes
- **WHEN** terminal geometry switches between hero-on-left and inline presentation
- **THEN** selected detail preserves the content declared for that surface
- **AND** only placement and arrangement-specific composition change

#### Scenario: Home and wide Movies use one selected-media card
- **WHEN** the same Movie is selected in Home and in wide Movies
- **THEN** the hero card uses the same image selection, metadata, watch-state, overview treatment, and cache behavior

#### Scenario: Shared card changes centrally
- **WHEN** the shared Home selected-media card changes
- **THEN** wide Movies renders that change without a second Movies-card edit

#### Scenario: Hero content remains consistent
- **WHEN** selected detail switches between hero-on-left and inline presentation
- **THEN** its declared image, metadata, overview, loading state, and child detail remain consistent

#### Scenario: Wide Movies card changes centrally
- **WHEN** the shared Home selected-media card presentation changes
- **THEN** wide Movies renders that change without a Movies-specific card edit

### Requirement: Series hero artwork has consistent cache identity

Series artwork SHALL be cached per declared image-type chain under one shared key
constructor, so every fetch, loading-state lookup, and completion match for the
same series and chain resolves to the same cache entry. The TV Wide shell prefetch
SHALL request the same canonical type chain the TV Wide painter declares, so the
prefetch warms the painted entry instead of a key no painter reads. The TV
workspace completion gate SHALL match the whole Series key family, so any Series
chain completion re-pushes TV content.

#### Scenario: Series prefetch warms the painted entry
- **WHEN** a Series is selected on the wide TV workspace
- **THEN** the shell prefetch fetches the same canonical image-type chain the painter declares
- **AND** paint-time handling starts no additional worker or network request for the same series and chain

#### Scenario: Series placeholder shows until the painted entry lands
- **WHEN** a Series is selected on wide TV or narrow inline detail and its painted
  cache entry is absent
- **THEN** the loading placeholder is shown
- **AND** no blank is shown in place of the pending artwork

#### Scenario: Series completion re-pushes the TV workspace
- **WHEN** any Series image chain for the selected series completes fetching
- **THEN** the TV workspace content is re-pushed
- **AND** the completed artwork paints without waiting for the next render cadence

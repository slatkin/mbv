# Spec Delta

## Purpose

Defines the TV library top-level pill row as a content-mode selector — which
modes exist and in what order, at which library size, which one opens by
default, how each mode sources and presents its rows, and how the selected mode
persists and cycles — so a TV library can be looked at as "newest", "coming
up", an alphabet range, or the whole library instead of only an alphabet split.

## MODIFIED Requirements

### Requirement: The TV top level is a content-mode selector

A TV library's top-level browse view SHALL present a pill row whose selectable
modes are, in order, `Latest`, `Upcoming`, then either the alphabet ranges when
the library's top-level show count exceeds the pill threshold, or a single
`All` mode when it does not. The row SHALL appear only at the top browse level
of the library and only while the user is not searching. When the alphabet
ranges are present, `All` SHALL NOT be offered. Above the threshold the mode
selected on entry SHALL be `Latest`; at or below it the mode selected on entry
SHALL be `All`.

#### Scenario: A large TV library opens on Latest
- **WHEN** a TV library's top-level show count exceeds the pill threshold and the user opens the library
- **THEN** the pill row shows `Latest`, `Upcoming`, `A-I`, `J-R`, `S-Z`
- **AND** `Latest` is selected without any user action

#### Scenario: A small TV library opens on All
- **WHEN** a TV library's top-level show count is at or below the pill threshold and the user opens the library
- **THEN** the pill row shows `Latest`, `Upcoming`, `All`
- **AND** `All` is selected without any user action

#### Scenario: The first open resolves the default mode from the captured show count
- **WHEN** a TV library is opened for the first time and no show count is known yet
- **THEN** the top level loads unfiltered once to capture the show count, and the mode row is not painted until the count lands
- **AND** the default mode for the captured count (`Latest` above the pill threshold, `All` at or below it) is then selected without user action, replacing the unfiltered load's content when the default is `Latest`

#### Scenario: All is not offered alongside the alphabet ranges
- **WHEN** a TV library's top-level show count exceeds the pill threshold
- **THEN** the pill row does not contain an `All` mode

#### Scenario: The row is top-level only
- **WHEN** the user is searching, or is below the library's top browse level
- **THEN** the TV content-mode pill row is not displayed

### Requirement: Latest mode presents the library's newest episodes

The `Latest` mode SHALL present the same content as Home's `Latest` section for
that library, sourced from the same Emby request. Selecting the mode SHALL
obtain that content independently of whether Home has loaded any section, and
SHALL NOT require Home to have been visited. The rows SHALL be a flat list of
episodes; the list SHALL NOT nest.

#### Scenario: Latest shows the library's newest episodes
- **WHEN** the user selects the `Latest` mode for a TV library
- **THEN** the list shows that library's newest episodes from the same source as Home's `Latest` section
- **AND** the rows are a flat episode list with no nesting

#### Scenario: Latest loads without Home
- **WHEN** the user opens a TV library and selects `Latest` without Home having loaded that library's `Latest` section
- **THEN** the library loads and shows the newest episodes

### Requirement: Upcoming mode presents the library's upcoming episodes

The `Upcoming` mode SHALL present the library's upcoming episodes from Emby's
`GET /Shows/Upcoming` route scoped to that library. The rows SHALL be a flat
list of episodes, possibly headed by non-selectable date headings (see the
grouping requirement below); the list SHALL NOT nest.

#### Scenario: Upcoming shows upcoming episodes
- **WHEN** the user selects the `Upcoming` mode for a TV library
- **THEN** the list shows that library's upcoming episodes from the `Upcoming` route scoped to the library
- **AND** the rows are a flat episode list with no nesting

### Requirement: Latest and Upcoming rows play directly or open their series

Activating a `Latest` or `Upcoming` row that carries a playable episode id SHALL play that episode and SHALL NOT open a series detail or Workspace. Activating an `Upcoming` row that carries no episode id but names its series SHALL navigate the library to that series and open its Workspace instead of playing. Keyboard and mouse activation SHALL take the same path. A selected `Latest` or `Upcoming` episode SHALL show a hero only in mini view; in every other geometry the mode presents its flat list across the full panel with no reserved hero pane.

#### Scenario: Activating a playable episode row plays it
- **WHEN** the user selects a `Latest` or `Upcoming` episode row with an episode id and activates it
- **THEN** that episode plays
- **AND** no series detail, season selector, or episode Workspace opens

#### Scenario: Activating an id-less Upcoming placeholder opens its series
- **WHEN** the user selects an `Upcoming` row with no episode id but a series reference and activates it, by keyboard or by mouse
- **THEN** the library navigates to that series and opens its Workspace
- **AND** no episode plays
- **AND** the landed series is the activated row's series on both input paths

#### Scenario: Mini view shows the episode hero
- **WHEN** the TV library is in mini view and a `Latest` or `Upcoming` episode is selected
- **THEN** the episode's hero is shown
- **AND** the same selection in any other geometry shows no episode hero and reserves no hero pane

### Requirement: The content mode is part of the sticky library position

The selected TV content mode SHALL be saved with the library's navigation
position and restored when the library is reopened. A saved mode that the
reopened library's current show count no longer offers SHALL be replaced by
the count's default mode before the row is painted or any fetch is issued.
Keyboard cycling SHALL move
the selection across every mode in row order and SHALL wrap from the last mode
to the first and from the first to the last. Mouse selection SHALL select the
clicked mode.

#### Scenario: The mode is restored on reopen
- **WHEN** the user selects a TV content mode and later reopens that library's saved position
- **THEN** the same mode is selected and its content is loaded

#### Scenario: A saved mode the current count no longer offers is re-clamped
- **WHEN** the user reopens a TV library whose saved position selected a mode the library's current show count no longer offers, because the count crossed the pill threshold between runs
- **THEN** the count's default mode (`Latest` above the threshold, `All` at or below it) is selected instead, before the row is painted or any fetch is issued
- **AND** no pill is highlighted over content it does not select

#### Scenario: Cycling wraps across modes
- **WHEN** the user cycles forward from the last mode, or backward from the first
- **THEN** the selection wraps to the opposite end
- **AND** every mode in the row participates in the cycle

### Requirement: The Latest mode reflects the library's shared new-content marker

When the shell marks a library's `Latest` section as having new content since
the previous launch, the library's `Latest` mode SHALL show that marker.
Selecting the library's `Latest` mode SHALL acknowledge the section, and that
acknowledgement SHALL be shared with Home's `Latest` pill for the same library
so that acknowledging on either surface clears the marker on both for the
remainder of the client run. Acknowledgement SHALL be keyed by the section's
provider identity.

#### Scenario: The marker appears on both surfaces
- **WHEN** a library's `Latest` section has new content since the previous launch
- **THEN** Home's `Latest` pill for that library shows the marker
- **AND** the library's `Latest` mode shows the marker

#### Scenario: Acknowledging on either surface clears both
- **WHEN** the user selects the library's `Latest` mode while its marker is shown
- **THEN** the marker clears on the library's `Latest` mode and on Home's `Latest` pill for that library
- **WHEN** the user instead selects Home's `Latest` pill for that library
- **THEN** the marker also clears on the library's `Latest` mode

#### Scenario: Content after launch does not mark
- **WHEN** `Latest` content appears after the current client launch
- **THEN** it does not add a marker during the current run

## ADDED Requirements

### Requirement: Upcoming rows are grouped and identified by series (ADDED 2026-09-22)

`Upcoming` rows SHALL be grouped under `Heading` rows derived from `PremiereDate` with relative labels (mirroring Emby web). Each row SHALL render `SeriesName` as primary and `"Sxx:Eyy — episode title"` (from `ParentIndexNumber`/`IndexNumber`/`Name`) as subtitle. Id-less rows SHALL carry synthesized stable per-row targets keyed by series + season/episode identity; no two rows SHALL share an empty-`Id` target.

#### Scenario: Upcoming groups by date with series context
- **WHEN** the user selects the `Upcoming` mode and the feed carries `PremiereDate`s, `SeriesName`s, and season/episode numbers
- **THEN** rows appear under relative-date headings with the series name and `Sxx:Eyy — title` on each row
- **AND** two id-less rows in one list resolve independently

### Requirement: TV Latest renders identically to Home Latest (ADDED 2026-09-22)

The same items SHALL produce the same row text (primary, secondary, trailing) in the TV `Latest` mode and in Home's `Latest` section for that library.

#### Scenario: Differential rendering
- **WHEN** identical items are fed to both render paths
- **THEN** the row text is identical on both surfaces

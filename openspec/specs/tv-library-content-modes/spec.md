# tv-library-content-modes Specification

## Purpose

Defines the TV library top-level pill row as a content-mode selector — which
modes exist and in what order, at which library size, which one opens by
default, how each mode sources and presents its rows, and how the selected mode
persists and cycles — so a TV library can be looked at as "newest", "coming
up", an alphabet range, or the whole library instead of only an alphabet split.

## Requirements


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
list of episodes; the list SHALL NOT nest.

#### Scenario: Upcoming shows upcoming episodes
- **WHEN** the user selects the `Upcoming` mode for a TV library
- **THEN** the list shows that library's upcoming episodes from the `Upcoming` route scoped to the library
- **AND** the rows are a flat episode list with no nesting

### Requirement: Latest and Upcoming rows play directly and never open a series workspace

Activating a `Latest` or `Upcoming` row SHALL play that episode. The TV library
SHALL NOT open a series detail or Workspace in response to activating a
`Latest` or `Upcoming` row. A selected `Latest` or `Upcoming` episode SHALL show
a hero only in mini view; in every other geometry the mode presents its flat
list without an episode hero.

#### Scenario: Activating an episode row plays it
- **WHEN** the user selects a `Latest` or `Upcoming` episode row and activates it
- **THEN** that episode plays

#### Scenario: No series workspace opens
- **WHEN** the user selects a `Latest` or `Upcoming` episode row
- **THEN** no series detail, season selector, or episode Workspace opens

#### Scenario: Mini view shows the episode hero
- **WHEN** the TV library is in mini view and a `Latest` or `Upcoming` episode is selected
- **THEN** the episode's hero is shown
- **AND** the same selection in any other geometry shows no episode hero

### Requirement: The content mode is part of the sticky library position

The selected TV content mode SHALL be saved with the library's navigation
position and restored when the library is reopened. Keyboard cycling SHALL move
the selection across every mode in row order and SHALL wrap from the last mode
to the first and from the first to the last. Mouse selection SHALL select the
clicked mode.

#### Scenario: The mode is restored on reopen
- **WHEN** the user selects a TV content mode and later reopens that library's saved position
- **THEN** the same mode is selected and its content is loaded

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

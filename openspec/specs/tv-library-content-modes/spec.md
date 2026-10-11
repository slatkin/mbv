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
- **AND** `Latest` is selected without any user action (user rule, 2026-10-11)

#### Scenario: The first open resolves the default mode from the captured show count
- **WHEN** a TV library is opened for the first time and no show count is known yet
- **THEN** the top level loads unfiltered once to capture the show count, and the mode row is not painted until the count lands
- **AND** `Latest` is then selected without user action, whatever the count, replacing the unfiltered load's content

#### Scenario: All is not offered alongside the alphabet ranges
- **WHEN** a TV library's top-level show count exceeds the pill threshold
- **THEN** the pill row does not contain an `All` mode

#### Scenario: The row is top-level only
- **WHEN** the user is searching, or is below the library's top browse level
- **THEN** the TV content-mode pill row is not displayed

### Requirement: Latest mode presents the library's newest episodes

The `Latest` mode SHALL present the TV library's newest episodes from its library-scoped Latest source. Selecting the mode SHALL obtain that content independently of Home; Home SHALL NOT contain a matching Latest section. The rows SHALL be a flat list of episodes; the list SHALL NOT nest.

#### Scenario: Latest shows the library's newest episodes
- **WHEN** the user selects the `Latest` mode for a TV library
- **THEN** the list shows that library's newest episodes as a flat episode list with no nesting

#### Scenario: Latest loads without Home
- **WHEN** the user opens a TV library and selects `Latest` without visiting Home
- **THEN** the library loads and shows the newest episodes

### Requirement: Upcoming mode presents the library's upcoming episodes

The `Upcoming` mode SHALL present the library's upcoming premieres from Emby's
`GET /Shows/Upcoming` route scoped to that library. Each row SHALL be a
premiere notice: an announcement of a future episode, not a media item. The
rows SHALL be a flat list, possibly headed by non-selectable date headings (see
the grouping requirement below); the list SHALL NOT nest.

#### Scenario: Upcoming shows upcoming episodes
- **WHEN** the user selects the `Upcoming` mode for a TV library
- **THEN** the list shows that library's upcoming premieres from the `Upcoming` route scoped to the library
- **AND** the rows are a flat list of premiere notices with no nesting

### Requirement: Upcoming rows are grouped and identified by series (ADDED 2026-09-22)

`Upcoming` rows SHALL be grouped under `Heading` rows derived from `PremiereDate` with relative labels (mirroring Emby web). Each row SHALL render `SeriesName` as primary and `"Sxx:Eyy — episode title"` (from `ParentIndexNumber`/`IndexNumber`/`Name`) as subtitle. The notice may lack one or both numbers. Then the row renders a fallback in place of the full `Sxx:Eyy` label: a season without an episode renders `Sxx`; an episode without a season renders `Eyy`; a notice with neither renders `no number`. The subtitle stays `<label> — episode title` in every case. Each row SHALL carry a stable per-row target keyed by series + season/episode identity; no two rows SHALL share a target.

#### Scenario: Upcoming groups by date with series context
- **WHEN** the user selects the `Upcoming` mode and the feed carries `PremiereDate`s, `SeriesName`s, and season/episode numbers
- **THEN** rows appear under relative-date headings with the series name and a `Sxx:Eyy — title`-shaped subtitle on each row, where the number label falls back to `Sxx`, `Eyy`, or `no number` when the notice lacks one or both numbers
- **AND** two rows in one list resolve independently

### Requirement: Latest rows play and show the shared Wide hero
Activating a `Latest` row, by keyboard or by mouse, in any geometry SHALL play its episode and SHALL NOT open a series detail, Workspace, or Library Hero overlay. In Wide geometry the selected `Latest` episode SHALL show its hero in the shared Hero pane, as every Latest list does. Mini view SHALL NOT open a hero by itself on selection.

#### Scenario: Activating a Latest row plays it
- **WHEN** the user selects a `Latest` episode row and activates it, by keyboard or by mouse, in any geometry
- **THEN** that episode plays
- **AND** no series detail, season selector, episode Workspace, or Library Hero overlay opens

#### Scenario: Wide Latest shows the episode hero
- **WHEN** the TV library is in Wide geometry with `Latest` selected and an episode row selected
- **THEN** the shared Hero pane shows that episode's hero
- **AND** the list and Hero pane occupy the same cells as a Movies `Latest` list at the same terminal size

#### Scenario: Mini view has no automatic hero
- **WHEN** the TV library is in mini view and a `Latest` row is selected
- **THEN** no Library Hero overlay opens by itself

#### Scenario: Keyboard actions address the displayed row at every geometry
- **WHEN** a `Latest` list is shown newest-first in any geometry, including narrow, and the user selects a row whose episode is not first in alphabetical order and presses Enter, Ctrl+P or Ctrl+A
- **THEN** the action addresses the episode displayed in the selected row
- **AND** no other episode in the list plays or is enqueued

### Requirement: Upcoming rows are premiere notices that open their series
Activating an `Upcoming` row, by keyboard or by mouse, SHALL navigate the library to that row's series and open its Workspace. An `Upcoming` row SHALL NOT play, enqueue, or offer any media action. An `Upcoming` row SHALL show no hero in any geometry; in Wide geometry the list SHALL span the whole content area.

#### Scenario: Activating an Upcoming row opens its series
- **WHEN** the user selects an `Upcoming` row and activates it, by keyboard or by mouse
- **THEN** the library navigates to that row's series and opens its Workspace
- **AND** no episode plays
- **AND** the landed series is the activated row's series on both input paths

#### Scenario: Upcoming rows offer no media actions
- **WHEN** an `Upcoming` row is selected and the user presses Ctrl+P or Ctrl+A
- **THEN** nothing plays and nothing enters the queue

#### Scenario: Upcoming has no hero
- **WHEN** `Upcoming` is selected in any geometry, including mini view
- **THEN** no Hero pane or Library Hero overlay is shown
- **AND** in Wide geometry the list spans the whole content area

### Requirement: The content mode is part of the sticky library position

The selected TV content mode SHALL be saved with the library's navigation position and restored when the library is reopened within the same session. The mode SHALL NOT persist across a restart: after a restart the library opens on `Latest` before the row is painted or any fetch is issued, regardless of the mode selected in the previous session. A saved mode that the reopened library's current show count no longer offers SHALL be replaced by the count's default mode before the row is painted or any fetch is issued. Keyboard cycling SHALL move the selection across every mode in row order and SHALL wrap from the last mode to the first and from the first to the last. Mouse selection SHALL select the clicked mode.

#### Scenario: The mode is restored on reopen
- **WHEN** the user selects a TV content mode, leaves the library's tab, and later reopens that library's saved position without restarting
- **THEN** the same mode is selected and its content is loaded

#### Scenario: Restart opens on Latest
- **WHEN** mbv restarts after a session in which a TV library's `Upcoming`, `All`, or alphabet-range mode was selected
- **THEN** the library opens on `Latest` instead of the previously selected mode

#### Scenario: A saved mode the current count no longer offers is re-clamped
- **WHEN** the user reopens a TV library whose saved position selected a mode the library's current show count no longer offers, because the count crossed the pill threshold between runs
- **THEN** `Latest` is selected instead, before the row is painted or any fetch is issued
- **AND** no pill is highlighted over content it does not select

#### Scenario: Cycling wraps across modes
- **WHEN** the user cycles forward from the last mode, or backward from the first
- **THEN** the selection wraps to the opposite end
- **AND** every mode in the row participates in the cycle

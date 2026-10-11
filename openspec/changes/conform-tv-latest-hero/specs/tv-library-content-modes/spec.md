# Spec Delta

## MODIFIED Requirements

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

`Upcoming` rows SHALL be grouped under `Heading` rows derived from `PremiereDate` with relative labels (mirroring Emby web). Each row SHALL render `SeriesName` as primary and `"Sxx:Eyy — episode title"` (from `ParentIndexNumber`/`IndexNumber`/`Name`) as subtitle. Each row SHALL carry a stable per-row target keyed by series + season/episode identity; no two rows SHALL share a target.

#### Scenario: Upcoming groups by date with series context
- **WHEN** the user selects the `Upcoming` mode and the feed carries `PremiereDate`s, `SeriesName`s, and season/episode numbers
- **THEN** rows appear under relative-date headings with the series name and `Sxx:Eyy — title` on each row
- **AND** two rows in one list resolve independently


## REMOVED Requirements

### Requirement: Latest and Upcoming rows play directly or open their series
**Reason**: It allowed an `Upcoming` row with an id to play, and it limited the TV episode hero to mini view. That limit came from the scope of `rework-tv-library-pills`, not from a product decision, and made TV Latest the only Latest list without a Wide hero.
**Migration**: Replaced by "Latest rows play and show the shared Wide hero" and "Upcoming rows are premiere notices that open their series" below.

## ADDED Requirements

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

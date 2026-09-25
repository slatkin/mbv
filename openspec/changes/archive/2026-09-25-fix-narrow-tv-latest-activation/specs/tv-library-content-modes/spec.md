# Spec Delta

## MODIFIED Requirements

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

#### Scenario: Keyboard actions address the displayed row at every geometry
- **WHEN** a `Latest` list is shown newest-first in any geometry, including narrow, and the user selects a row whose episode is not first in alphabetical order and presses Enter, Ctrl+P or Ctrl+A
- **THEN** the action addresses the episode displayed in the selected row
- **AND** no other episode in the list plays or is enqueued


# Spec Delta

## ADDED Requirements

### Requirement: A list of non-media rows has no Hero pane
In Wide geometry, when a destination's current list rows are not media items, the Library panel SHALL paint no Hero pane and the list SHALL span the whole content area. Every other list SHALL keep the Hero pane, empty when its selection has no hero. The panel SHALL decide this from the destination's content, never from which destination it is.

#### Scenario: TV Upcoming spans the content area
- **WHEN** a TV library shows `Upcoming` in Wide geometry
- **THEN** no Hero pane is painted and the list fills the content area below the Selector band

#### Scenario: A media list without a selected hero keeps its pane
- **WHEN** a media list in Wide geometry has a selection that produces no hero
- **THEN** the Hero pane is still painted, empty, at its usual geometry

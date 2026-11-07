## MODIFIED Requirements

### Requirement: A completed TUI session persists one launch location

On orderly exit, a TUI SHALL persist one launch-state snapshot containing the selected tab, whether Library or Queue held Panel focus, and, for the selected tab only, the destination's persisted pill scope. The persisted pill scope is the `Latest` pill for every destination whose main Selector offers a `Latest` pill, the selected group for a Grouped Music library, the selected bucket for an Audiobookshelf book library, and Home's fixed scope. A destination whose selector presents no pills SHALL record no pill. The snapshot SHALL NOT record the selected library item or any other live pill selection; pill choices other than the persisted scope are session memory. When the GUI Client exits with two or more groups selected in a Grouped Music library, the persisted pill scope SHALL be the first selected group, and the snapshot SHALL also carry the other selected groups, which only the GUI Client restores.

The snapshot SHALL NOT contain state for unselected tabs, a selected Queue item, a nested Workspace selector such as a TV season, an overlay or Sidebar, a search query or result, multi-selection other than the GUI Client's selected Grouped Music groups, Visual mode, undo history, scroll offsets, loading or error state, or paint geometry.

#### Scenario: A session exits from a library item
- **WHEN** a TUI exits normally with a tab, a non-default main Selector pill, and a library item selected and Library holding Panel focus
- **THEN** its launch snapshot SHALL identify that tab, the tab's persisted pill scope, and Library focus
- **THEN** it SHALL NOT record the selected library item, the live pill, or state for an unselected tab

#### Scenario: Queue holds focus at exit
- **WHEN** a TUI exits normally while Queue holds Panel focus
- **THEN** its launch snapshot SHALL record Queue focus
- **THEN** it SHALL still record the selected Library tab's persisted pill scope
- **THEN** it SHALL NOT record the Queue cursor or selected Queue slot

#### Scenario: The selected tab has no main Selector pills or selectable items
- **WHEN** the selected tab presents no main Selector pill and no selectable library item at exit
- **THEN** the snapshot SHALL represent the absent pill without substituting a Queue selection or nested Workspace selection
- **THEN** a Library destination whose Selector offers a Latest pill SHALL still restore on Latest, whether or not the exit view painted its pill row

#### Scenario: The GUI Client exits with several Music groups selected
- **WHEN** the GUI Client exits normally with Music active and the Hip-Hop and Soul groups selected, in that pill order
- **THEN** the snapshot SHALL record Hip-Hop as the persisted pill scope and Soul as an extra selected group
- **THEN** a TUI that starts from this snapshot SHALL restore Music on Hip-Hop only
- **THEN** a GUI Client that starts from this snapshot SHALL restore Music with Hip-Hop and Soul selected

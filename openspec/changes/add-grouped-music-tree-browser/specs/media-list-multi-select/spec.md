## ADDED Requirements

### Requirement: Grouped Music tree selection materializes album leaves
The Grouped Music tree SHALL provide the same Visual-mode outcomes and status presentation as canonical media lists while retaining tree-local ownership of its membership and anchor. Album leaves SHALL be the only stored and emitted multi-selection identities.

Toggling an artist root through `V`, Ctrl+Click, or the equivalent tree selection operation SHALL toggle all of that root's visible in-scope album leaves. The artist root SHALL display aggregate Unmarked, Partial, or Marked state derived from those leaves. Shift+Click and keyboard range extension SHALL traverse visible album leaves in tree display order and SHALL skip artist roots. A filter SHALL limit artist-root selection and range extension to visible matching album leaves. Marks on albums hidden by the filter SHALL remain stored but SHALL be masked from aggregate state, status counts, painting, ranges, and actions while hidden; dismissing the filter SHALL reveal those surviving marks without selecting albums that were unmarked before or during filtering.

Context and playback actions over the resulting multi-selection SHALL carry selected album targets in visible tree display order and the stable identity of the originating tree. Artist identities SHALL never appear in the emitted target set. Running a multi-selection action SHALL clear only the tree's multi-selection.

#### Scenario: Toggling an unfiltered artist selects descendants
- **WHEN** an unfiltered artist root with three visible album leaves is toggled into the multi-selection
- **THEN** all three album targets enter the selection in settled display order
- **AND** the artist root displays Marked aggregate state

#### Scenario: Partial artist state is derived
- **WHEN** some but not all visible album leaves beneath an artist root are selected
- **THEN** the artist root displays Partial state
- **AND** no artist identity is stored as a selected effect target

#### Scenario: Filtered artist selection uses visible matches
- **WHEN** filtering leaves two of an artist's five albums visible and the artist root is toggled
- **THEN** only the two visible album targets enter or leave the multi-selection
- **AND** clearing the filter does not add the three formerly hidden albums

#### Scenario: Dismissing a filter reveals surviving hidden marks
- **WHEN** an album was marked before filtering, the filter hides it, and the user dismisses the filter without clearing selection
- **THEN** the hidden mark contributes to no filtered count or action while hidden
- **AND** the album is marked again after dismissal while every previously unmarked album remains unmarked

#### Scenario: Range selection skips roots
- **WHEN** the anchor and clicked target are album leaves separated by one or more artist roots
- **THEN** the range contains visible album leaves between those targets in tree display order
- **AND** artist roots are skipped

#### Scenario: Bulk action emits album identities
- **WHEN** a context or playback action runs over a tree multi-selection
- **THEN** it receives ordered stable album targets and the originating-tree identity
- **AND** completing the action clears the tree selection without clearing Queue selection

## MODIFIED Requirements

### Requirement: Grouped-view continuity
When a settled grouped snapshot is replaced, the system SHALL preserve the current artist-root or album-leaf selection by stable identity when that node remains present. It SHALL preserve surviving artist expansion state. When the selected node survives, the viewport SHALL retain its prior screen row when projection bounds permit; otherwise it SHALL scroll only enough to keep the node visible and clamp at projection bounds. Artist roots SHALL remain stable grouping and action targets across the replacement.

#### Scenario: Selected album survives a replacement
- **WHEN** a replacement snapshot contains the album leaf selected in the prior snapshot
- **THEN** that album remains selected and remains visible after the replacement is committed

#### Scenario: Selected artist survives a replacement
- **WHEN** a replacement snapshot contains the artist root selected in the prior snapshot
- **THEN** that artist remains selected with its persistent expansion state preserved

#### Scenario: Selected album is absent from a replacement
- **WHEN** the selected artist or album node is not present in a replacement
- **THEN** the system selects a valid visible node using its normal fallback selection behavior

#### Scenario: Artist grouping survives a replacement
- **WHEN** a replacement snapshot is committed
- **THEN** its settled artist roots continue to group the visible album leaves with stable identity and expansion

#### Scenario: Artist header action follows the visible grouping
- **WHEN** the user invokes an artist-root action on a settled grouped view
- **THEN** the action operates on exactly that root's in-scope albums in the settled snapshot

### Requirement: Album navigation remains visible across artist groups
Tree navigation in the grouped Music view SHALL keep the selected artist root or album leaf visible while crossing artist-group boundaries. Artist roots SHALL contribute to scroll geometry and SHALL be cursor targets; collapsed album leaves SHALL not contribute to the visible projection or hit geometry.

#### Scenario: Cursor crosses an artist boundary
- **WHEN** tree navigation moves selection from one artist group to an adjacent group
- **THEN** the destination visible node is selected
- **AND** the viewport adjusts as needed to keep it visible

#### Scenario: Artist is collapsed
- **WHEN** an expanded artist root is collapsed
- **THEN** its album leaves leave the visible projection
- **AND** viewport clamping keeps the focused root visible

### Requirement: Responsive grouped-view continuity

The non-Wide and Wide compositions SHALL consume the same settled grouped snapshot and tree owner. Changing composition SHALL NOT restart artist metadata resolution, publish a different grouping for the same snapshot, or replace the selected artist root or album leaf when it remains available.

#### Scenario: Grouped Music crosses the responsive breakpoint
- **WHEN** terminal resizing switches grouped Music between its non-Wide and Wide compositions
- **THEN** the same settled grouping, selected tree node, and expansion state remain in use
- **AND** the active tree viewport is clamped around that selection

#### Scenario: Responsive composition redraws
- **WHEN** either responsive composition redraws without a changed album snapshot
- **THEN** it reuses the existing settled grouping without starting artist metadata resolution work

## REMOVED Requirements

### Requirement: Artist headers are non-selectable grouping labels

**Reason**: Grouped Music replaces its former structural artist Heading rows with focusable artist roots in the shallow tree.

**Migration**: Grouped Music uses the new `Artist roots are focusable grouping targets` requirement. Canonical Group headings in every other list and in the artist track Workspace remain non-selectable visual labels.

## ADDED Requirements

### Requirement: Artist roots are focusable grouping targets
The grouped Music album view SHALL present focusable artist roots in the shallow tree. Artist roots SHALL receive keyboard and mouse selection, expansion, current-item scope, playback and queue actions, and context actions as specified by `grouped-music-tree-browser`. Album leaves SHALL remain independently selectable targets within each artist root. This SHALL NOT change canonical Group headings in any other list or in the artist track Workspace; those remain non-selectable visual labels.

#### Scenario: Keyboard navigation crosses an artist boundary
- **WHEN** the user moves through the grouped Music tree across an artist boundary
- **THEN** focus can land on the artist root and its visible album leaves

#### Scenario: Artist root is clicked
- **WHEN** the user clicks a painted artist root
- **THEN** that root receives focus and becomes the current tree action scope

#### Scenario: Grouped music action is invoked
- **WHEN** the user invokes a playback, queue, or context action with an artist root focused
- **THEN** the action resolves the root to its in-scope album leaves
- **AND** no artist identity crosses the effect boundary as a playable target

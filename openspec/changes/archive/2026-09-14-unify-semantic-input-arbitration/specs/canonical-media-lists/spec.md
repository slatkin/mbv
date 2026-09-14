## MODIFIED Requirements

### Requirement: Row-local input uses one delegation contract

After the mounted destination resolves overlay, chrome, and active-pane precedence, it SHALL offer every remaining eligible row-local key and normalized pointer gesture to one provider-neutral media-list delegation contract. Pointer input SHALL be resolved through current painted geometry to a stable target before the shared owner applies the corresponding target-bearing operation; keyboard and pointer delivery SHALL remain distinct before this resolution and SHALL converge on the same row-local state operations afterward.

The shared owner SHALL return one transition whose independent facts describe whether the input was unhandled or consumed, whether the selected stable target changed, whether a multi-selection presentation summary changed, and whether a provider-neutral external row intent was requested. A transition MAY report more than one fact for the same operation. A newly added purely local behavior SHALL use the consumed disposition and SHALL NOT require a new destination dispatch arm or a shell mirror of list-local state.

The mounted destination SHALL remain the sole TuiRealm event boundary and SHALL retain mouse gesture recognition. Embedded media-list presentations SHALL NOT be mounted, focused, subscribed, assigned a component identity, or made into another keyboard router. The shared owner SHALL receive no raw terminal event and SHALL NOT resolve screen coordinates supplied without the presentation's stable target result.

#### Scenario: Local movement is delegated once

- **WHEN** an eligible movement, page, edge, or row-selection operation reaches an active media-row flow
- **THEN** the shared delegation contract mutates the shared owner
- **AND** destination code does not call row cursor, scroll, membership, anchor, or point-selection mutators directly
- **AND** the transition independently reports every resulting target or summary change

#### Scenario: One operation has several consequences

- **WHEN** one row-local operation consumes input, changes the selected target or multi-selection, and requests an external intent
- **THEN** one transition reports all applicable facts
- **AND** destination code does not reproduce the operation by calling delegation more than once or sequencing private mutators

#### Scenario: External row intent stays provider-specific

- **WHEN** shared row handling resolves activation or context intent for one or more stable targets
- **THEN** the parent translates that provider-neutral intent into its typed destination intent
- **AND** Service, Player, persistence, target materialization, and effect authority do not enter the shared list

#### Scenario: Keyboard and pointer select through one state transition

- **WHEN** a keyboard Visual operation and a modifier-click express the same selection operation
- **THEN** both apply the same shared-owner transition after pointer geometry has been resolved to a stable target
- **AND** their resulting membership, anchor, selected target, and presentation summary are identical

#### Scenario: Pointer resolution remains geometry-owned

- **WHEN** a normalized pointer gesture lands on a canonical media row
- **THEN** the active presentation resolves the point from its completed current-frame geometry before delegation
- **AND** the shared owner receives the stable target rather than raw coordinates for later re-resolution

## ADDED Requirements

### Requirement: Selection summaries do not duplicate list authority

A canonical media list SHALL remain the sole owner of its multi-selection membership and anchor. When those values change, its transition MAY expose a read-only presentation summary containing only facts needed by another surface, such as selected count and originating-list identity. The summary SHALL NOT contain a writable membership copy, SHALL NOT be accepted as an ordinary content projection, and SHALL NOT be used to reconstruct or overwrite selection.

Library and Queue lists MAY retain independent multi-selections simultaneously. Panel focus SHALL determine which list's summary appears in the status presentation and which list receives keyboard input; changing panel focus SHALL NOT clear either list. A selection action SHALL clear only its originating list.

#### Scenario: Library and Queue selections survive focus changes

- **WHEN** Library and Queue each hold a multi-selection and panel focus moves between them
- **THEN** neither shared owner loses membership or anchor state
- **AND** the status presentation changes to the newly focused list's summary

#### Scenario: Destination switch remains list-local

- **WHEN** the active Library destination changes while Queue also holds a multi-selection
- **THEN** the departing Library destination follows its defined selection-retention or clearing policy
- **AND** Queue's selection is unchanged

#### Scenario: Summary cannot restore selection

- **WHEN** a list clears or prunes selected targets after content changes
- **THEN** a previously published summary cannot restore those targets
- **AND** the next summary reflects the owner's resulting state

### Requirement: Selection intents preserve list order and origin

A context intent over a multi-selection SHALL carry the selected stable targets in canonical list order and a stable identity for the originating list. The destination SHALL resolve those targets against one coherent owned content snapshot and translate them to effect-ready domain values. Generic list code SHALL NOT retain provider objects or destination lookup callbacks.

Missing targets SHALL be handled explicitly and SHALL NOT be replaced by the current cursor or silently resolved from a different snapshot. Context-menu lifetime and focus changes SHALL NOT change the origin or ordered target snapshot used by the action.

#### Scenario: Context selection resolves in list order

- **WHEN** targets were selected in an order different from their displayed order
- **THEN** the context intent carries them in canonical list order
- **AND** the destination resolves them against one content snapshot in that order

#### Scenario: Focus changes while menu is open

- **WHEN** a menu opened from Queue while Library also retains a selection
- **THEN** the menu action remains addressed to Queue and its captured ordered targets
- **AND** completing the action clears Queue's selection without clearing Library's selection

#### Scenario: Selected target disappears before resolution

- **WHEN** an emitted target is absent from the destination snapshot used to materialize the action
- **THEN** the destination reports or deliberately rejects the missing target according to the action contract
- **AND** it does not substitute the cursor target or another occurrence

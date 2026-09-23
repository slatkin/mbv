# Spec Delta

## ADDED Requirements

### Requirement: TreeBrowser supports optional structural group headings
A nested browser SHALL support optional group headings and between-group spacers among its root rows. Each structural row SHALL occupy a painted row and participate in scrolling and grouping, but SHALL carry no selectable media target and SHALL never receive cursor focus, expansion, marks, activation, context intent, or pointer selection. Movement and paging SHALL skip structural rows; their presence SHALL not make a destination without structural rows display any. A heading SHALL remain associated with its following group when the browser's content is refreshed or its geometry changes. A spacer SHALL separate consecutive groups without becoming a tree node.

#### Scenario: Grouping is visible but not interactive
- **WHEN** a destination supplies a heading followed by show nodes
- **THEN** the heading paints before those shows and occupies one row in the scrollable flow
- **AND** keyboard movement and pointer gestures can select the shows but cannot select, expand, mark, activate, or open a menu on the heading or spacer

#### Scenario: Ungrouped tree is unchanged
- **WHEN** a destination supplies no headings
- **THEN** its tree has no additional rows and retains its existing navigation, marks, painting, and hit behavior

#### Scenario: Refresh preserves stable selection across headings
- **WHEN** grouped content is refreshed or reordered while the selected media target remains present
- **THEN** that target remains selected and its viewport remains valid even if a heading appears or disappears

### Requirement: Right navigation expands, activates roots, and no-ops on expanded descendants
The shared TreeBrowser SHALL implement one `Right` operation over the selected row: on an unexpanded expandable row it SHALL expand that row; on an expanded root row it SHALL emit the row's activation intent to the destination; and on an expanded non-root row it SHALL perform no action while still consuming the key. A non-expandable selected row SHALL leave `Right` unhandled.

#### Scenario: Right on an expanded root activates the destination
- **WHEN** the selected row is an expanded root
- **THEN** the operation emits the activation intent for that row's stable target

#### Scenario: Right on an expanded descendant is a no-op
- **WHEN** the selected row is expanded and has a parent
- **THEN** the operation consumes the key without moving, collapsing, or emitting an intent

#### Scenario: Right on an unexpanded expandable row expands it
- **WHEN** the selected row is expandable but not expanded
- **THEN** the operation expands that row and reveals its children

## MODIFIED Requirements

### Requirement: Tree rows share one visual hierarchy
The shared Render Component SHALL paint nested rows with one destination-neutral visual hierarchy. An ordinary node title's resting foreground role SHALL fall back by nesting depth: the cream emphasis role at depth zero, the yellow focus-accent role at depth one, and the aqua accent role at deeper depths; semantic-state and selected-row roles SHALL still take precedence when they apply. A heading SHALL paint bold in the metadata role with its title sitting on the same baseline as a depth-zero node title, taking the shared content inset as its effective two-column inset. A heading and a spacer SHALL each paint the unstriped base; only item rows SHALL participate in zebra striping. Zebra striping SHALL reset at each heading: a row's stripe SHALL derive from its position within the group that heading introduces, not from its absolute position in the flow or viewport, and SHALL remain stable under clipping as well as scrolling, so stripes stay stable across scroll and clip and no stripe runs continuously across group boundaries. A played row's resting foreground SHALL use the existing muted/played palette role above the generic depth colours, while selected-row and other semantic-state roles still take precedence when they apply.

#### Scenario: Depth colours fall back cream, yellow, aqua
- **WHEN** an ordinary unselected node title paints at depth zero, depth one, or depth two and deeper
- **THEN** its resting foreground role is the cream emphasis, yellow focus-accent, or aqua accent role respectively
- **AND** a selected or semantically emphasised row still paints its own role instead of the depth fallback

#### Scenario: A heading aligns with the depth-zero baseline
- **WHEN** a heading and the depth-zero nodes of its following group paint
- **THEN** the heading's title starts on the same baseline as those node titles, via the effective two-column inset

#### Scenario: Stripes reset per heading and hold across scroll and clip
- **WHEN** the viewport scrolls a grouped tree or clips a group at the viewport edge
- **THEN** each row's stripe still matches its position within its heading's group
- **AND** the heading and spacer paint unstriped and no stripe claim runs continuously from one group across a heading into the next

#### Scenario: A played row reads muted before depth colours
- **WHEN** an unselected row with the played semantic state paints at any depth
- **THEN** its title uses the existing muted/played palette role rather than the cream, yellow, or aqua depth fallback
- **AND** a selected played row still paints the selected-row role

### Requirement: Nesting destinations use one complete shared TreeBrowser

Every destination that presents a nested row flow SHALL use one shared `TreeBrowser<Target>` as the complete nesting counterpart to the complete flat-list component. `TreeBrowser<Target>` SHALL implement TuiRealm `Component`; it SHALL remain embedded rather than independently mounted, focused, subscribed, or assigned a `ComponentId`. The shared Interactive Component SHALL own tree model reconciliation, expansion state, filtering state, cursor and viewport behavior, keyboard tree operations, ordered multi-selection and aggregate mark presentation, retained point geometry, marquee state, and all presentation state. Its `Component::view` SHALL be the only interactive view entry point and SHALL delegate indentation, zebra striping, selected-row treatment, optional trailing metadata, marquee, and scrollbar painting to one shared destination-neutral Render Component.

A destination SHALL supply only plain `TreeNode<Target>` values containing typed stable targets, parent-child relationships, row content, searchable text, semantic state, a declaration of whether the node can have children even while they are not yet loaded, and the closed per-node `TreeMarkPolicy`, plus optional plain structural heading and spacer entries placed among root groups. Structural entries SHALL carry no target, tree-library handle, parent-child edge, or behavior. The shared TreeBrowser SHALL permit expansion of a node declared to have children even when none are currently projected, retain that expansion across reconciliation, and reveal children when they arrive; heading-free Music nodes SHALL retain their existing child-derived expansion behavior. The destination SHALL translate emitted stable-target intents. It SHALL supply no trait implementation, callback, closure, model/query/state object, renderer, painter, navigation rule, filter matcher, aggregation algorithm, or action-order policy. It SHALL NOT define another tree browser, tree state machine, tree painter, tree navigation implementation, inherent or destination-owned alternative view entry point, or wrapper that reproduces those responsibilities. Internal tree-library identifiers and types SHALL remain private to the shared Interactive and Render Components in both production and tests.

#### Scenario: A destination adopts tree browsing through typed data
- **WHEN** a destination needs to present a nested row flow
- **THEN** it supplies only plain `TreeNode<Target>` values and optional plain structural headings/spacers, and translates emitted stable-target intents
- **AND** it supplies no behavior implementation, callback, mutable state object, renderer, painter, or policy beyond the closed per-node `TreeMarkPolicy`

#### Scenario: A tree behavior changes centrally
- **WHEN** expansion, tree navigation, filtering, selection presentation, marquee, indentation, zebra striping, or scrollbar behavior changes without changing the destination-data contract
- **THEN** the production change is confined to the shared `TreeBrowser`
- **AND** every adopting destination receives that behavior without changing destination-owned browser code

#### Scenario: Tree-library details remain private
- **WHEN** a destination or its tests select, expand, mark, activate, render, or resolve a pointer against a tree row
- **THEN** the operation addresses the row by its destination-defined stable target
- **AND** no production or test module outside the shared Interactive and Render Components imports, names, constructs, or asserts against an internal node handle, projection index, or tree-library type

#### Scenario: All mutations use one complete operation surface
- **WHEN** a destination requests movement, expansion, filtering, marking, pointer resolution, activation, or context behavior
- **THEN** it invokes `apply(TreeOperation<Target>)` and receives one `TreeTransition<Target>` containing every independently observable result
- **AND** no destination-visible mutator bypasses that operation surface

#### Scenario: Invalid hierarchy reconciliation is atomic
- **WHEN** reconciliation receives duplicate targets, a missing parent, self-parenting, or a cycle
- **THEN** it returns a typed reconciliation error
- **AND** content, selection, expansion, marks, viewport, and retained geometry remain unchanged

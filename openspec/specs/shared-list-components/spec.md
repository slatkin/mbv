# shared-list-components Specification

## Purpose

Defines the one reusable seam every mbv list shape satisfies — flat and tree alike — so that row flow, cursor, viewport, retained paint geometry, point resolution, and ordered multi-selection have exactly one implementation site rather than one per list shape.

## Requirements

### Requirement: Every list is one ordered row flow addressed by stable target

A list SHALL present its content as one ordered sequence of rows in paint order. Each row SHALL be either selectable and carrying a stable opaque target, or structural and carrying none. A list's ordering SHALL be the order its rows paint, so that row position, viewport offset, and point resolution all address the same sequence. Supporting structural rows in the abstraction SHALL NOT require a shape to synthesize structural rows its current implementation cannot produce.

A stable target's identity SHALL survive reorder and ordinary refresh. A target that is unique only within a parent SHALL include that parent identity. A nesting shape with heterogeneous row kinds SHALL use one closed stable-target type that distinguishes those kinds; its internal node handles SHALL remain private. The flow SHALL impose no fixed nesting depth and SHALL NOT require any row to carry optional decoration such as a metadata gutter.

#### Scenario: Structural rows occupy the flow without being selectable
- **WHEN** a list's content includes structural rows among its selectable rows
- **THEN** those structural rows occupy positions in the painted flow
- **AND** they carry no stable target and cannot become the selection

#### Scenario: A flow imposes no depth
- **WHEN** a list shape presents nested rows
- **THEN** the flow accepts any nesting depth without a shape-specific limit
- **AND** a list with no nesting satisfies the same flow contract

### Requirement: Shared list mechanics have exactly one implementation site

Cursor movement, viewport resolution and clamping, retained paint geometry, point resolution, and ordered multi-selection membership SHALL each have one shared algorithm or state-carrier implementation over the row flow and SHALL NOT be reimplemented per list shape. A list shape SHALL supply only primitive access to its existing state owner, how its rows and completed-paint geometry are produced, and any behavior its shape defines differently where the seam names an explicit policy. A shape adapter SHALL NOT reproduce the shared arithmetic.

Adding or correcting one of these shared mechanics without changing its primitive adapter contract SHALL be possible by changing the shared implementation alone, without editing either list shape's own code.

#### Scenario: A shared mechanic has one change site
- **WHEN** a developer changes cursor movement, viewport clamping, or retained point resolution behavior
- **THEN** the production change is confined to the shared implementation
- **AND** no list shape's own production code changes
- **AND** every list shape receives the change through the seam

#### Scenario: A shape supplies only shape-determined behavior
- **WHEN** a list shape is implemented against the seam
- **THEN** it supplies row and paint production, primitive state access, and its explicitly named policy choices
- **AND** it supplies no cursor, viewport, retained-geometry, point-resolution, or ordered-membership arithmetic of its own

### Requirement: Public list surfaces address rows by stable target

A list's public surface SHALL address rows by stable opaque target. It SHALL NOT expose an internal row index, node handle, arena identifier, or any other position- or storage-derived value by which a caller could address a row.

A request a list emits because of a row SHALL carry that stable target. Internal identifiers a list uses to store or project its rows SHALL remain private to that list's implementation.

#### Scenario: A caller cannot address a row by internal identity
- **WHEN** a destination selects a row, resolves a point to a row, or requests an effect for a row
- **THEN** it names that row by its stable target
- **AND** no internal index, node handle, or arena identifier is available to it

#### Scenario: Internal identity does not cross the boundary
- **WHEN** a list projects rows from an internal store that identifies them by its own handles
- **THEN** those handles remain private to that list
- **AND** the list's selection, point-resolution, and request surfaces expose only stable targets

### Requirement: Cursor movement addresses only selectable rows

Cursor movement SHALL move between selectable rows and SHALL NOT rest on a structural row. Movement past the first or last selectable row SHALL clamp rather than wrap. First and last selection SHALL resolve to the first and last selectable rows in the flow.

Movement SHALL be defined over the rows currently present in the flow, so that rows absent from the flow are not reachable and remain unreachable until they return to it.

#### Scenario: Movement skips structural rows
- **WHEN** the cursor moves across a run of rows containing structural rows
- **THEN** it lands only on selectable rows
- **AND** the structural rows remain painted in place

#### Scenario: Movement clamps at the ends
- **WHEN** the cursor is on the first selectable row and moves backward, or on the last and moves forward
- **THEN** the selection stays where it is
- **AND** no wrap to the opposite end occurs

#### Scenario: Rows absent from the flow are unreachable
- **WHEN** rows leave the flow
- **THEN** cursor movement cannot reach them
- **AND** the selection and viewport remain valid over the remaining flow

### Requirement: The viewport keeps the selection visible and clamps at bounds

A list SHALL keep its selection visible in its viewport. When the selection survives a content or geometry change, the list SHALL preserve its prior viewport offset where bounds permit, and otherwise apply only the minimum scroll needed to bring it into view, clamping at flow bounds.

A geometry change SHALL clamp the existing viewport in place and SHALL NOT transfer viewport or cursor state into a second control. Paging distance SHALL be an explicitly named policy a list shape selects, because shapes differ in what a page means over their flow.

#### Scenario: A surviving selection keeps its viewport row
- **WHEN** content is replaced and the selected target is still present
- **THEN** its prior viewport offset is preserved where bounds permit
- **AND** otherwise the viewport scrolls the minimum needed to keep it visible

#### Scenario: Geometry change clamps in place
- **WHEN** a list's available area changes
- **THEN** the same list retains its cursor, viewport, and selection
- **AND** the viewport is clamped to the new bounds without copying state elsewhere

#### Scenario: Paging is a named shape policy
- **WHEN** two list shapes page their viewports
- **THEN** each applies its own explicitly named paging policy
- **AND** neither inherits the other's paging behavior implicitly

### Requirement: Point resolution uses only completed-frame geometry

A list SHALL retain the geometry of its latest completed paint and SHALL resolve a point only from that retained geometry. Changing content or geometry, or beginning a paint that does not complete, SHALL invalidate the retained geometry; while invalid the list SHALL claim no point and SHALL expose no selected-row geometry.

A point-resolution call SHALL accept only the point and SHALL return the stable target of the row it resolves to, if any. A list SHALL NOT expose a mutable row map, row rectangles, or caller-supplied resolution geometry.

#### Scenario: Stale geometry claims no point
- **WHEN** a list's content or geometry changes and it has not completed a subsequent paint
- **THEN** it claims no point
- **AND** it exposes no selected-row geometry
- **AND** its parent has no fallback using prior or reconstructed geometry

#### Scenario: A resolved point names a stable target
- **WHEN** a point falls on a selectable row of a completed paint
- **THEN** resolution returns that row's stable target
- **AND** no row map or rectangle set is exposed to the caller

### Requirement: Multi-selection stores addition order and emits action order explicitly

A list that supports multi-selection SHALL retain its selected targets in the order they were added. Removing a target SHALL preserve the relative order of the rest. Multi-selection membership SHALL be expressed in stable targets. Read-only presentation state that depends on mark history MAY use this addition order.

An action or context intent SHALL emit selected targets in the current visible row-flow order, preserving the canonical list-order contract independently of addition order. Aggregating a multi-selection across nested rows SHALL belong to the list shape that has nesting, and SHALL NOT be required of a shape without it.

#### Scenario: Stored addition order survives addition and removal
- **WHEN** rows are added to and removed from a multi-selection
- **THEN** the stored membership order matches the order the remaining rows were added
- **AND** membership is stored as stable targets

#### Scenario: An action emits display order rather than click order
- **WHEN** rows are marked in an order different from their current visible row-flow order
- **AND** the list emits an action or context intent for them
- **THEN** the intent carries those stable targets in current visible row-flow order
- **AND** changing action order does not rewrite the stored addition order

#### Scenario: A flat shape needs no aggregation
- **WHEN** a list shape without nesting supports multi-selection
- **THEN** it provides membership and order without any aggregate parent state

### Requirement: Content replacement preserves selection by stable identity

Replacing a list's content SHALL preserve the selection when its stable target is still present, along with that list's row-local state for surviving rows. When the selected target is absent from the replacement, the list SHALL resolve to a deterministic selection over the new flow rather than an arbitrary one.

A list SHALL NOT translate a prior numeric position into the replacement flow to recover its selection.

#### Scenario: Reorder preserves the selection
- **WHEN** replacement content reorders rows and still contains the selected target
- **THEN** that target remains selected
- **AND** no numeric position from the prior flow is used to find it

#### Scenario: A vanished selection resolves deterministically
- **WHEN** replacement content no longer contains the selected target
- **THEN** the list resolves to a deterministic selection over the new flow
- **AND** the viewport is clamped to the new bounds

### Requirement: A nesting list shape owns expansion

A list shape whose flow nests SHALL own which rows are expanded, and its flow SHALL contain exactly the rows its current expansion makes visible. Collapsing a row SHALL remove its descendants from the flow, leaving selection, viewport, and later point resolution valid over the remaining flow.

Expansion state SHALL survive content replacement by stable identity. A list shape without nesting SHALL NOT be required to implement expansion.

#### Scenario: Collapse removes descendants from the flow
- **WHEN** an expanded row with visible descendants is collapsed
- **THEN** those descendants leave the flow
- **AND** selection, viewport, and point resolution remain valid

#### Scenario: Expansion survives replacement
- **WHEN** content is replaced and previously expanded rows are still present
- **THEN** their expansion state is still in use
- **AND** rows absent from the replacement carry no stale expansion state

#### Scenario: A flat shape implements no expansion
- **WHEN** a list shape has no nesting
- **THEN** it satisfies the seam without providing expansion behavior

### Requirement: Tree rows share one visual hierarchy
The shared Render Component SHALL paint nested rows with one destination-neutral visual hierarchy. An ordinary node title's resting foreground role SHALL fall back by nesting depth: the cream emphasis role at depth zero, the yellow focus-accent role at depth one, and the aqua accent role at deeper depths; semantic-state and selected-row roles SHALL still take precedence when they apply. A heading SHALL paint bold in the metadata role with its title sitting on the same baseline as a depth-zero node title, taking the shared content inset as its effective two-column inset. A heading itself SHALL paint unstriped. Zebra striping SHALL reset at each heading: a row's stripe SHALL derive from its position within the group that heading introduces, not from its absolute position in the flow or viewport, and SHALL remain stable under clipping as well as scrolling, so stripes stay stable across scroll and clip and no stripe runs continuously across group boundaries. A played row's resting foreground SHALL use the existing muted/played palette role above the generic depth colours, while selected-row and other semantic-state roles still take precedence when they apply.

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
- **AND** the heading itself paints unstriped and no stripe claim runs continuously from one group across a heading into the next

#### Scenario: A played row reads muted before depth colours
- **WHEN** an unselected row with the played semantic state paints at any depth
- **THEN** its title uses the existing muted/played palette role rather than the cream, yellow, or aqua depth fallback
- **AND** a selected played row still paints the selected-row role

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

### Requirement: Grouped Music is a consumer rather than an implementation of TreeBrowser

Grouped Music SHALL consume the shared `TreeBrowser` by supplying `TreeNode<MusicTreeTarget>` values and translating typed stable-target intents. `MusicContent` SHALL contain exactly one tree-control field, of concrete type `TreeBrowser<MusicTreeTarget>`. Music production modules SHALL NOT retain any tree model/query/state, expansion/filter/viewport/mark/geometry/marquee carrier, tree painter, renderer, forwarding browser API, or duplicate implementation of behavior owned by the shared component.

This adoption SHALL preserve Grouped Music's existing observable tree behavior and presentation, including Artist, Album, and Track hierarchy; expansion and parent-child navigation; filtering and restoration; multi-selection aggregation; selected-row and zebra treatment; title marquee; indentation; conditional year gutter; scrollbar; pointer resolution; viewport continuity; and destination effects.

#### Scenario: Music paints through the shared component
- **WHEN** Grouped Music renders its nested browser in Wide or non-Wide Panel mode
- **THEN** the shared `TreeBrowser` is its only browser owner and painter
- **AND** no Music-specific fallback browser or painter exists

#### Scenario: Music behavior is preserved during extraction
- **WHEN** Music adopts the shared `TreeBrowser`
- **THEN** its current hierarchy, navigation, filtering, marks, rendering, pointer behavior, responsive continuity, and typed effects remain unchanged
- **AND** the extraction introduces no new user-visible Music behavior

#### Scenario: Music-specific metadata is data, not browser logic
- **WHEN** an Album row carries a year while an Artist or yearless row does not
- **THEN** Music supplies the optional trailing metadata through the shared row-content contract
- **AND** the shared `TreeBrowser` owns its reservation, alignment, clipping, and painting

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

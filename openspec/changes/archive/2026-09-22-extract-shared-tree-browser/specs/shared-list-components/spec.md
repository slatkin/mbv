# Spec Delta

## ADDED Requirements

### Requirement: Nesting destinations use one complete shared TreeBrowser

Every destination that presents a nested row flow SHALL use one shared `TreeBrowser<Target>` as the complete nesting counterpart to the complete flat-list component. `TreeBrowser<Target>` SHALL implement TuiRealm `Component`; it SHALL remain embedded rather than independently mounted, focused, subscribed, or assigned a `ComponentId`. The shared Interactive Component SHALL own tree model reconciliation, expansion state, filtering state, cursor and viewport behavior, keyboard tree operations, ordered multi-selection and aggregate mark presentation, retained point geometry, marquee state, and all presentation state. Its `Component::view` SHALL be the only interactive view entry point and SHALL delegate indentation, zebra striping, selected-row treatment, optional trailing metadata, marquee, and scrollbar painting to one shared destination-neutral Render Component.

A destination SHALL supply only plain `TreeNode<Target>` values containing typed stable targets, parent-child relationships, row content, searchable text, semantic state, and the closed per-node `TreeMarkPolicy`, then translate emitted stable-target intents. A destination SHALL supply no trait implementation, callback, closure, model/query/state object, renderer, painter, navigation rule, filter matcher, aggregation algorithm, or action-order policy. It SHALL NOT define another tree browser, tree state machine, tree painter, tree navigation implementation, inherent or destination-owned alternative view entry point, or wrapper that reproduces those responsibilities. Internal tree-library identifiers and types SHALL remain private to the shared Interactive and Render Components in both production and tests.

#### Scenario: A destination adopts tree browsing through typed data
- **WHEN** a destination needs to present a nested row flow
- **THEN** it supplies only plain `TreeNode<Target>` values and translates emitted stable-target intents
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

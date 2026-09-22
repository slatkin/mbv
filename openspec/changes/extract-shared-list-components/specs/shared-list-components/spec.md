# Spec Delta

## Purpose

Defines the one reusable seam every mbv list shape satisfies — flat and tree alike — so that row flow, cursor, viewport, retained paint geometry, point resolution, and ordered multi-selection have exactly one implementation site rather than one per list shape.

## ADDED Requirements

### Requirement: Every list is one ordered row flow addressed by stable target

A list SHALL present its content as one ordered sequence of rows in paint order. Each row SHALL be either selectable and carrying a stable opaque target, or structural and carrying none. A list's ordering SHALL be the order its rows paint, so that row position, viewport offset, and point resolution all address the same sequence.

A stable target's identity SHALL survive reorder and ordinary refresh. A target that is unique only within a parent SHALL include that parent identity. The flow SHALL impose no fixed nesting depth and SHALL NOT require any row to carry optional decoration such as a metadata gutter.

#### Scenario: Structural rows occupy the flow without being selectable
- **WHEN** a list's content includes structural rows among its selectable rows
- **THEN** those structural rows occupy positions in the painted flow
- **AND** they carry no stable target and cannot become the selection

#### Scenario: A flow imposes no depth
- **WHEN** a list shape presents nested rows
- **THEN** the flow accepts any nesting depth without a shape-specific limit
- **AND** a list with no nesting satisfies the same flow contract

### Requirement: Shared list mechanics have exactly one implementation site

Cursor movement, viewport resolution and clamping, retained paint geometry, point resolution, and ordered multi-selection membership SHALL be implemented once over the row flow and SHALL NOT be reimplemented per list shape. A list shape SHALL supply only what its shape genuinely determines: how its rows are produced, and any behavior its shape defines differently where the seam names an explicit policy.

Adding or correcting one of these shared mechanics SHALL be possible by changing the shared implementation alone, without editing either list shape's own code.

#### Scenario: A shared mechanic has one change site
- **WHEN** a developer changes cursor movement, viewport clamping, or retained point resolution behavior
- **THEN** the production change is confined to the shared implementation
- **AND** no list shape's own production code changes
- **AND** every list shape receives the change through the seam

#### Scenario: A shape supplies only shape-determined behavior
- **WHEN** a list shape is implemented against the seam
- **THEN** it supplies its row production and its explicitly named policy choices
- **AND** it supplies no cursor, viewport, retained-geometry, or point-resolution arithmetic of its own

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

### Requirement: Multi-selection preserves the order rows were added

A list that supports multi-selection SHALL retain its selected targets in the order they were added, and SHALL report that order to callers. Removing a target SHALL preserve the relative order of the rest. Multi-selection membership SHALL be expressed in stable targets.

Aggregating a multi-selection across nested rows SHALL belong to the list shape that has nesting, and SHALL NOT be required of a shape without it.

#### Scenario: Selection order survives addition and removal
- **WHEN** rows are added to and removed from a multi-selection
- **THEN** the reported order matches the order the remaining rows were added
- **AND** membership is reported as stable targets

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

# Interactive Component Framework Delta

## MODIFIED Requirements

### Requirement: Mounted parents recognize mouse gestures and embedded controls resolve targets

A mounted destination `AppComponent` SHALL own its TuiRealm mouse subscription and its `MouseGestureState`. An embedded media-list control SHALL resolve a point within the list rectangle its parent painted to a stable target, using the same internal row flow its component `view` painted. After the parent recognizes a mouse gesture, it SHALL delegate point resolution to the active embedded control and translate the returned stable target into the destination request.

An embedded control SHALL NOT subscribe independently, own a second gesture recognizer, store a per-row rectangle list duplicating its internal row flow, export that flow to a parent-owned list painter, or publish row rectangles into a parent-owned hit map. Parent-owned controls outside the list rectangle, such as pills or Queue scope buttons, MAY retain separate parent hit regions, populated where those rectangles are painted. When a recognized point falls within the embedded list rectangle, the embedded control's explicit list targets SHALL be resolved before any parent workspace target. Canonical destinations SHALL NOT retain per-surface canonical row-hit `*HitRegion` enums; `QueueHitRegion` and equivalent row registries SHALL remain absent once row hits resolve through the embedded control.

#### Scenario: A pointer gesture targets a list row

- **WHEN** the mounted parent recognizes a click, double click, context click, or scroll gesture over its embedded list rectangle
- **THEN** the parent passes the list rectangle it allocated and the point to the active embedded control, which resolves it from the same internal row flow its `view` painted
- **AND** it returns the stable target or list-local scroll result to the parent
- **AND** neither the parent nor shell recomputes the row from coordinates

#### Scenario: A pointer gesture targets a parent control

- **WHEN** the mounted parent recognizes a gesture over a pill, Queue scope button, or another region outside the embedded list rectangle
- **THEN** the parent resolves that separately owned region
- **AND** the embedded control's hit regions remain limited to its own painted rectangle

#### Scenario: Queue migrates mouse hit ownership

- **WHEN** Queue composes the canonical fixed-row control
- **THEN** Queue's parent keeps the subscription, gesture state, and scope-button geometry
- **AND** the embedded control resolves a point in its painted row area to a `QueueSlotId`
- **AND** Queue has no `QueueHitRegion` or equivalent parent-owned row registry

### Requirement: Destination components may embed reusable interaction controls

A destination `AppComponent` MAY own a reusable plain TuiRealm `Component` as an embedded interaction control when the control is not an independently mounted surface. The embedded control SHALL implement the framework's component contract and SHALL be persistent for the parent's lifetime, sharing the parent's mount, activation, focus, and subscription. It SHALL NOT be constructed during rendering, receive a `ComponentId`, register independently with the application, recognize raw events, or create another event-precedence boundary.

For each frame, the parent SHALL configure the control's closed semantic paint policy before directly invoking its single `Component::view(frame, outer_paint_rect)` call. The policy SHALL name the content inset and selected-row treatment but SHALL contain no provider client, raw style, callback, image, or effect. Because `view` returns no value, the control SHALL retain a per-frame paint result for the parent to consume after that call: canonical row geometry and selected rectangle, plus an Inline control's admitted detail rectangle. The parent SHALL NOT invoke a second list painter or reconstruct list geometry/content area.

The embedded control SHALL own any delegated live cursor, scroll, viewport, movement, list painting entry point, and render-derived row geometry for its region. The destination parent SHALL remain the application-level `Event` to `Msg` boundary, own provider-specific chrome and workspace state, delegate list-local commands only to the active embedded control, and translate that control's resolved opaque target into the destination's typed request.

#### Scenario: A destination owns a canonical media list

- **WHEN** a destination component is mounted for a media browser
- **THEN** each embedded media-list control is created and destroyed with that destination and implements the plain component view contract
- **AND** the application registry contains only the destination's existing identity
- **AND** focus and subscriptions continue to target the destination component

#### Scenario: A list-local key is received

- **WHEN** the destination parent receives a key that belongs to its currently painted media list
- **THEN** it delegates the corresponding local command to that embedded control only
- **AND** the control resolves and applies movement against its own geometry
- **AND** the parent emits a typed request only when work crosses the component boundary

#### Scenario: A global key is received

- **WHEN** a key is owned by the central keyboard policy rather than the destination list
- **THEN** the existing router resolves it before list-local delegation
- **AND** embedding a reusable control creates no second global resolution site

#### Scenario: A parent recognizes a pointer gesture

- **WHEN** the destination parent recognizes a pointer gesture inside its painted list rectangle
- **THEN** it delegates point resolution to the active embedded control
- **AND** the control returns a stable target from its own painted row geometry without receiving the raw event or owning gesture state

#### Scenario: One child view supplies one paint result

- **WHEN** a destination paints an embedded canonical list
- **THEN** it configures the child once, calls the child view once, and consumes that frame's retained result afterward
- **AND** the parent does not separately calculate an inner list rectangle, paint ordinary rows, or call another list renderer

### Requirement: Embedded controls have one state owner and one painter

For each rectangle and reachable presentation, exactly one persistent embedded control or parent-owned workspace SHALL own interaction state and painting for that region. A parent that delegates a list to an embedded control SHALL NOT retain a second live list cursor or scroll, repaint the control's ordinary rows, rebuild or copy its row geometry, repeatedly seed its selection during rendering, or copy the control's paint-resolved state back into parent state. A convenience field that mirrors the embedded control's current cursor is forbidden even when the parent, rather than the shell, owns it.

Canonical content projection types SHALL exclude cursor and scroll; carrying those values but ignoring them is not sufficient. Position input retained for an explicit non-canonical carve-out SHALL use a separate type and code path that canonical presentations cannot read. A discrete navigation or breakpoint transition MAY explicitly re-anchor a control using the selected stable target and the selected ordinary row's zero-based offset from the top of the list viewport.

The non-hero two-column Browser path is the sole accepted position-and-geometry carve-out in this change. Its `BrowserGridState` and `BrowserGridGeometry`, including grid row maps, SHALL be inaccessible to canonical Browser presentations. TV episode and Audiobookshelf episode/chapter panes are explicit parent-owned provider workspace exemptions, retaining their typed activation and seek targets; they SHALL NOT be used to exempt a destination rail from this ownership rule.

#### Scenario: Content refreshes in place

- **WHEN** a destination pushes refreshed rows while the visible browse identity is unchanged
- **THEN** each embedded control preserves its live selection and scroll by stable target where possible
- **AND** the parent does not push a duplicate cursor or scroll value

#### Scenario: A breakpoint changes the presentation

- **WHEN** a destination changes between its Wide and Inline controls
- **THEN** the parent obtains one explicit anchor from the outgoing control and applies it once to the incoming control
- **AND** only the incoming active control receives subsequent movement
- **AND** ordinary render passes do not synchronize the two controls

#### Scenario: The parent renders a destination

- **WHEN** the parent delegates its list rectangle to the embedded control
- **THEN** the control's component view is the sole ordinary-row painter and row-geometry owner for that rectangle
- **AND** the parent paints only its arrangement-adjacent pills, hero/detail payload, workspace, or other separately owned regions

#### Scenario: A canonical destination keeps no parent mirror

- **WHEN** the embedded control changes its cursor or paint-resolved scroll
- **THEN** the parent resolves selected content through the control's stable target and does not copy the numeric state into parallel fields
- **AND** shell-owned persistence receives a resolved target or resting position only at the event that requires it

#### Scenario: A non-canonical grid remains separate

- **WHEN** a destination uses the accepted two-column non-hero grid presentation
- **THEN** grid cursor, columns, scroll, and row geometry live in `BrowserGridState` and `BrowserGridGeometry`
- **AND** that state is not shared with or projected into a canonical embedded control

#### Scenario: Ordinary content has no position fields

- **WHEN** Music, TV, or Audiobookshelf content is pushed without a discrete navigation or restore event
- **THEN** its projection contains no cursor or scroll
- **AND** its control retains local state or locally clamps to refreshed content
- **AND** a separately named resting-position/re-anchor input is the only permitted position source

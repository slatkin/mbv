## ADDED Requirements

### Requirement: Destination components may embed reusable interaction controls

A destination `AppComponent` MAY own a reusable plain TuiRealm `Component` as an embedded interaction control when the control is not an independently mounted surface. The embedded control SHALL share the parent's mount, activation, focus, subscription, and lifetime; it SHALL NOT receive a `ComponentId`, register independently with the application, or create another event-precedence boundary.

The embedded control MAY own the delegated live cursor, scroll, viewport, movement, painting entry point, and render-derived hit geometry for its region. The destination parent SHALL remain the application-level `Event` to `Msg` boundary, own provider-specific presentation and workspace state, and translate the embedded control's resolved opaque target into the destination's typed request.

#### Scenario: A destination owns a canonical media list

- **WHEN** a destination component is mounted for a media browser
- **THEN** its embedded media-list control is created and destroyed with that destination
- **AND** the application registry contains only the destination's existing identity
- **AND** focus and subscriptions continue to target the destination component

#### Scenario: A list-local key is received

- **WHEN** the destination parent receives a key that belongs to its active media list
- **THEN** it delegates the corresponding local command to the embedded control
- **AND** the control resolves and applies the movement against its own geometry
- **AND** the parent emits a typed request only when work crosses the component boundary

#### Scenario: A global key is received

- **WHEN** a key is owned by the central keyboard policy rather than the destination list
- **THEN** the existing router resolves it before list-local delegation
- **AND** embedding a reusable control creates no second global resolution site

### Requirement: Embedded controls have one state owner and one painter

For each rectangle and reachable presentation, exactly one embedded control or parent-owned workspace SHALL own the interaction state and painting for that region. The parent SHALL NOT mirror an embedded control's live cursor or scroll, repaint the control's body, or overwrite the control's state during an ordinary content push. A discrete navigation or breakpoint transition MAY explicitly re-anchor the control using the selected stable target and the selected ordinary row's zero-based offset from the top of the list viewport.

#### Scenario: Content refreshes in place

- **WHEN** a destination pushes refreshed rows while the visible browse level is unchanged
- **THEN** the embedded control preserves its live selection and scroll by stable target where possible
- **AND** the parent does not push a duplicate cursor or scroll value

#### Scenario: A breakpoint changes the presentation

- **WHEN** a destination changes between its Wide and Inline controls
- **THEN** the parent performs one explicit handoff of the selected target and defined viewport-row offset
- **AND** ordinary render passes do not continually synchronize the two controls

#### Scenario: The parent renders a destination

- **WHEN** the parent delegates its list rectangle to the embedded control
- **THEN** the embedded control is the sole painter and hit-geometry owner for that rectangle
- **AND** the parent paints only its arrangement-adjacent pills, hero, workspace, or other separately owned regions

### Requirement: Mounted parents recognize mouse gestures and embedded controls resolve targets

A mounted destination `AppComponent` SHALL own its TuiRealm mouse subscription and its `MouseGestureState`. An embedded media-list control SHALL own the `HitRegions<Target>` populated from the geometry of its own most recent view. After the parent recognizes a mouse gesture, it SHALL delegate point resolution to the embedded control and translate the returned stable target into the destination request. Parent-produced mouse messages cross the boundary in the runtime-only typed origin envelope defined by `restore-mouse-support`; the shell arbitrates that source tag before unwrapping the winning semantic message.

An embedded control SHALL NOT subscribe independently, own a second gesture recognizer, or publish row rectangles into a parent-owned duplicate hit map. Parent-owned controls outside the list rectangle, such as pills or Queue scope buttons, MAY retain separate parent hit regions. When a recognized point falls within the embedded list rectangle, the embedded control's explicit list targets SHALL be resolved before any parent workspace target. The per-surface canonical row-hit enums SHALL migrate to the embedded control's `HitRegions<Target>` within the owning canonical slice, not within `restore-mouse-support`.

#### Scenario: A pointer gesture targets a list row

- **WHEN** the mounted parent recognizes a click, double click, context click, or scroll gesture over its embedded list rectangle
- **THEN** the embedded control resolves the point against hit regions populated by its own view
- **AND** it returns the stable target or list-local scroll result to the parent
- **AND** neither the parent nor shell recomputes the row from coordinates

#### Scenario: A pointer gesture targets a parent control

- **WHEN** the mounted parent recognizes a gesture over a pill, Queue scope button, or another region outside the embedded list rectangle
- **THEN** the parent resolves that separately owned region
- **AND** the embedded control's hit regions remain limited to its own painted rectangle

#### Scenario: Queue migrates mouse hit ownership

- **WHEN** Queue composes the canonical fixed-row control
- **THEN** Queue's parent keeps the subscription, gesture state, and scope-button geometry
- **AND** the embedded control owns row hit regions and resolves `QueueSlotId`
- **AND** no parallel Queue row-hit migration remains in `restore-mouse-support`

### Requirement: Embedded control requests carry resolved values

When embedded-control interaction requires shell-owned persistence, pagination, activation, navigation, or another effect, the destination parent SHALL emit a typed request carrying the resolved stable target or resolved position. Neither the parent nor shell SHALL recompute the original movement delta or read a mirrored cursor to determine the effect target.

#### Scenario: Movement requires persistence

- **WHEN** an embedded list movement also updates a resting position or pagination state
- **THEN** the emitted request carries the resolved target or position once
- **AND** the shell applies that value directly

#### Scenario: Pointer input selects a row

- **WHEN** supported pointer input resolves against geometry populated by an embedded control's most recent view
- **THEN** the resolved stable target is returned to the destination parent
- **AND** no parent, arrangement, or shell coordinate path recomputes the row

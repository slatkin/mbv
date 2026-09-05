## ADDED Requirements

### Requirement: Destination components may embed reusable interaction controls

A destination `AppComponent` MAY own a reusable plain TuiRealm `Component` as an embedded interaction control when the control is not an independently mounted surface. The embedded control SHALL be persistent for the parent's lifetime and share its mount, activation, focus, and subscription; it SHALL NOT be constructed during each render pass, receive a `ComponentId`, register independently with the application, or create another event-precedence boundary.

The embedded control MAY own the delegated live cursor, scroll, viewport, movement, painting entry point, and the render-derived row geometry its painting and scrolling need for its region. The destination parent SHALL remain the application-level `Event` to `Msg` boundary, own provider-specific presentation and workspace state, and translate the embedded control's resolved opaque target into the destination's typed request.

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

For each rectangle and reachable presentation, exactly one persistent embedded control or parent-owned workspace SHALL own the interaction state and painting for that region. The parent SHALL NOT mirror an embedded control's live cursor or scroll, repaint the control's body, rebuild its row or replacement geometry, or overwrite the control's state during an ordinary content push. Canonical content projection types SHALL exclude cursor and scroll; carrying those values but ignoring them is not sufficient. Position input retained for an explicit non-canonical carve-out SHALL be isolated from canonical paths. A discrete navigation or breakpoint transition MAY explicitly re-anchor the control using the selected stable target and the selected ordinary row's zero-based offset from the top of the list viewport.

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
- **THEN** the embedded control is the sole painter and row-geometry owner for that rectangle
- **AND** the parent paints only its arrangement-adjacent pills, hero, workspace, or other separately owned regions

### Requirement: Embedded control requests carry resolved values

When embedded-control interaction requires shell-owned persistence, pagination, activation, navigation, or another effect, the destination parent SHALL emit a typed request carrying the resolved stable target or resolved position. Neither the parent nor shell SHALL recompute the original movement delta or read a mirrored cursor to determine the effect target.

#### Scenario: Movement requires persistence

- **WHEN** an embedded list movement also updates a resting position or pagination state
- **THEN** the emitted request carries the resolved target or position once
- **AND** the shell applies that value directly

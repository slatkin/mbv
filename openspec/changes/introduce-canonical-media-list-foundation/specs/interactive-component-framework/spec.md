# interactive-component-framework Specification Delta

## MODIFIED Requirements

### Requirement: Embedded canonical controls remain under the mounted parent
A canonical media-list control SHALL be a persistent plain embedded TuiRealm `Component`, not an independently mounted component or a value constructed during each render pass. It SHALL have no registry identity, subscription, focus-stack entry, second router, callback/provider framework, or effect execution. The mounted destination parent remains the application event boundary. Mouse is out of scope for this slice: no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation is added, and existing bespoke `*HitRegion` paths stay wired and untouched; `restore-mouse-support` (#638) lands after every canonical slice and owns all mouse work.

#### Scenario: Parent translates a child intent
- **WHEN** a canonical control resolves activation or movement
- **THEN** the parent translates the stable target/resolved value to its existing typed message
- **AND** Service, Player, persistence, and effects remain shell-owned.

### Requirement: Component-local values are not mirrored
The embedded control SHALL own live cursor, scroll, selection, viewport, and internal row geometry for painting and scrolling. The parent SHALL delegate list-local movement to the active control and translate its resolved stable target without recomputing the movement. Shell projections SHALL carry content only; the canonical Browser projection type SHALL exclude cursor and scroll rather than carry ignored mirror fields, and the write-back leg that stores the component's painted scroll into the App navigation level SHALL be removed with them. Any retained cursor/scroll input for the non-hero two-column carve-out SHALL be isolated to that legacy path and unreachable from canonical paths. Responsive transitions use one explicit selected-target plus selected-row-offset handoff, and persisted resting position is written only at navigation events.

#### Scenario: A refresh does not overwrite local position
- **WHEN** the shell pushes ordinary refreshed content
- **THEN** the embedded control keeps its live cursor and scroll, clamping only when its target no longer exists
- **AND** it does not adopt a shell mirror or write paint-derived values back every frame.

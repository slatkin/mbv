## MODIFIED Requirements

### Requirement: Wheel scrolling has a uniform interaction policy

Every scrollable interactive surface SHALL interpret an accepted wheel gesture as exactly one forward or backward logical step: ScrollUp moves toward the preceding row or viewport line, and ScrollDown moves toward the following row or viewport line. A surface SHALL NOT use a per-surface wheel multiplier or page-sized wheel movement.

The component that owns the pointed list, selection, or text viewport SHALL apply the step locally. It SHALL send a message beyond that component only when the resolved result requires an existing shell-owned effect or persistence operation; the shell SHALL NOT recompute the wheel step or re-resolve the pointed surface.

A migrated canonical media-list surface SHALL accept a wheel gesture only when its
embedded list control claims the pointed list region from its completed current-frame
retained result. An unmigrated destination MAY retain its existing compatibility
claim path until its own migration. A text viewport or irregular-row surface that competes with another eligible surface SHALL accept a wheel gesture only when the point is inside geometry published by its own painter. A sole eligible focused overlay SHALL accept its wheel gesture independently of pointer position; its local boundary clamp SHALL retain valid state.

A canonical media list SHALL apply an accepted wheel step to its viewport rather than to its selection: the visible window moves one display row and the selection stays where it is, unless the step would put the selection outside the window, in which case the selection is dragged to the leading edge of the step direction — the first selectable row of the new window for a step toward the preceding row, the last selectable row of the new window for a step toward the following row — and never left on the edge it was dragged off. The list SHALL apply the step to its viewport even when the pointed list does not hold keyboard focus.

#### Scenario: Wheel moves a canonical list by one row

- **WHEN** the user turns the wheel down once over a painted canonical media list with a following row
- **THEN** the list's owning component moves its local viewport down by one row
- **AND** the selection moves only when it would otherwise leave the viewport
- **AND** a dragged selection lands on the leading-edge selectable row of the new viewport, never on the edge it left
- **AND** no shell-side wheel movement is calculated for that list

#### Scenario: Wheel moves a text viewport by one line

- **WHEN** the user turns the wheel up once over a painted text viewport with a preceding line
- **THEN** the viewport's owning component moves its local offset back by one line
- **AND** its cursor or selection remains unchanged unless that surface's ordinary one-step navigation also changes it

#### Scenario: A wheel step reaches the first display row of a grouped list

- **WHEN** the user turns the wheel up repeatedly over a grouped canonical list whose first selectable row follows a group `Heading`
- **THEN** the viewport reaches the first display row
- **AND** the first group's `Heading` is painted

#### Scenario: A competing surface ignores an outside wheel gesture

- **WHEN** the user turns the wheel over painted chrome or blank space outside a competing surface's relevant list or viewport region
- **THEN** that competing surface does not change its selection, cursor, or viewport offset

#### Scenario: A focused sole overlay accepts wheel independently of pointer

- **WHEN** a focused sidebar is the sole eligible overlay and the user turns the wheel with the pointer outside its painted content
- **THEN** the sidebar moves its local selection or viewport by one step
- **AND** focus does not follow the pointer

#### Scenario: A boundary clamps wheel movement

- **WHEN** the user turns the wheel toward the start or end of a scrollable surface that is already at that boundary
- **THEN** the component retains its valid boundary selection and viewport offset
- **AND** no shell-side movement is requested

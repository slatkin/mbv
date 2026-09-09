# Mouse Input Delta

## MODIFIED Requirements

### Requirement: Every visible interactive surface receives mouse events

An interactive component that is mounted **and painted in the most recent frame**
SHALL receive every terminal mouse event, not only while it holds keyboard focus.
A component that is mounted but was not painted in the most recent frame SHALL
NOT receive mouse events at all — not merely have its resulting message
discarded — because the component framework mutates a component before it
returns a message, so a discarded message does not undo a mutation.

Mouse-event delivery SHALL use the component framework's subscription mechanism;
the shell SHALL NOT introduce a separate mouse event loop, a global
completed-frame hit map, or a global coordinate router.

The mounted destination parent owns gesture recognition for its surface. The
parent SHALL decide whether an event is its own by testing the event coordinates
against the non-list chrome geometry it painted on its most recent render —
pills, scope buttons, the seek bar and transport, overlay and popup regions. For a canonical media-list row whose embedded control has a completed current-frame
retained result, the parent SHALL delegate point resolution by passing only the
point to that result, and SHALL NOT re-derive row coordinates itself. An
unmigrated destination MAY continue to pass its painted list rectangle through its
existing compatibility path until its own retained-result migration. The parent SHALL emit a `Msg` for a
mouse event only when it resolves to a region it painted or to a row the embedded
control claims; otherwise it SHALL ignore the event.

A component that resolves a mouse event SHALL resolve it against geometry it
painted itself. Filtering by mouse event kind SHALL happen inside the component;
no behaviour SHALL depend on the subscription clause filtering by event kind.

#### Scenario: A click lands on a panel that does not hold focus

- **WHEN** keyboard focus is on one panel and the user clicks inside a different
  visible panel
- **THEN** the clicked panel's mounted parent receives the event, resolves the
  target from the non-list chrome geometry it painted or by delegating a list
  point to its embedded control, and acts on it
- **AND** the focused panel's mounted parent, receiving the same event with
  coordinates outside its geometry, produces no message

#### Scenario: Mouse events reach a subscribed component through a live tick

- **WHEN** a mouse event is injected into a mounted `Application` through its
  event listener and `tick()` is called
- **THEN** every eligible mounted parent is given the event, and each resolves
  it against the geometry it painted or its embedded control's rows
- **AND** no parent's message for that event is produced twice

#### Scenario: A destination is mounted but not painted

- **WHEN** several destinations are mounted at once and only one is painted,
  and a mouse event arrives over the painted one
- **THEN** the unpainted destinations' mouse handlers are not invoked at all
- **AND** their cursor, scroll offset, and selection are unchanged

#### Scenario: Chrome that never holds focus is still clickable

- **WHEN** the user clicks a transport control or the seek bar in the playback
  chrome, which never receives keyboard focus
- **THEN** the playback component resolves the click against its painted control
  geometry and emits the corresponding transport intent, or a seek intent
  carrying a resolved position fraction
- **AND** the shell handler for that intent does not read the seek bar's
  rectangle

### Requirement: Pointer gestures are recognized by the mounted parent

Each mounted destination parent SHALL recognize click, double-click, right-click,
wheel, and drag gestures from the raw mouse events it receives, using a private
`MouseGestureState`. The double-click interval and wheel throttle SHALL NOT be
held as shell-global state keyed by screen position. An embedded canonical
media-list control SHALL NOT recognize gestures — it only resolves a point from
its completed current-frame retained geometry to a stable target, and the parent
delegates list-point resolution to it. An unmigrated destination MAY retain its
existing painted-rectangle compatibility path until its own migration.

A drag SHALL be recognized as a left-button press that arms a drag anchor at the
press position, followed by pointer motion while the button is held, and ended
by the button release. The recognizer SHALL report the anchor position and the
current pointer position with each motion, and SHALL report the end of the drag
so the parent can release any state it holds for it. Recognizing a drag SHALL
NOT suppress the click the press already produced: a press remains a click, and
a drag is an additional gesture that follows it. A press that is released
without intervening motion SHALL produce no drag gesture at all.

Drag anchor state SHALL be private to the recognizing parent, in the same way
the double-click interval and wheel throttle are. A parent that does not
interpret drag SHALL be unaffected by the gesture's existence.

Hit geometry for a migrated uniform row flow SHALL be resolved from the completed
current-frame result the control retains, not from a separately stored per-row
rectangle list. An unmigrated destination MAY retain its existing compatibility
flow until its own migration. A stored rectangle registry is for irregular painted controls — pills,
scope buttons, transport controls, group selectors, overlay rows — whose owner
populates it in the same code that paints those rectangles.

A mounted parent SHALL translate a recognized gesture into a semantic typed `Msg`
carrying the resolved target (a child-returned row identity, a control, a pill
index, a seek fraction), never raw coordinates for the shell to re-resolve. The
shell handler for that `Msg` SHALL accept the resolved target as an argument, and
SHALL NOT read the painted geometry of the component that emitted it. A drag
gesture SHALL be translated the same way: the parent resolves both the anchor
and the current position to stable targets before emitting, and SHALL NOT emit
positions for the shell to resolve.

The gesture vocabulary SHALL remain open to hover (`enter`, `leave`) gestures
without changing the delivery or arbitration mechanism; those gestures are out
of scope for this capability but SHALL NOT be precluded by its design.

#### Scenario: A double-click activates the pointed row

- **WHEN** the user clicks the same row twice within the double-click interval
- **THEN** the mounted parent recognizes a double-click, delegates row resolution
  to the embedded list control, and emits the activation intent for that row's
  child-returned identity
- **AND** a single click on the same row emits only a focus/selection intent

#### Scenario: A wheel event scrolls the pointed list

- **WHEN** the user turns the wheel over a scrollable canonical list in any panel
- **THEN** the mounted parent recognizes the scroll gesture, subject to its own
  throttle, and the embedded list control scrolls its own viewport and keeps its
  own row identity, whether or not the list holds keyboard focus

#### Scenario: A right-click opens the context menu at the pointer

- **WHEN** the user right-clicks a selectable row on any migrated interactive
  surface that paints selectable rows
- **THEN** the row is focused and the context menu opens anchored at the click
  position

#### Scenario: A press and drag is recognized as a drag

- **WHEN** the user presses the left button over a surface and moves the pointer
  while holding it
- **THEN** the parent recognizes a click at the press position, and then a drag
  gesture for each motion, carrying both the press position and the current
  position
- **AND** releasing the button ends the drag

#### Scenario: A press without motion is only a click

- **WHEN** the user presses and releases the left button without moving the
  pointer
- **THEN** the parent recognizes a click and no drag gesture

#### Scenario: A parent that does not interpret drag is unaffected

- **WHEN** the user drags over a surface whose parent has no drag behaviour
- **THEN** the surface behaves exactly as it did before drag recognition existed

### Requirement: Wheel scrolling has a uniform interaction policy

Every scrollable interactive surface SHALL interpret an accepted wheel gesture as exactly one forward or backward logical step: ScrollUp moves toward the preceding row or viewport line, and ScrollDown moves toward the following row or viewport line. A surface SHALL NOT use a per-surface wheel multiplier or page-sized wheel movement.

The component that owns the pointed list, selection, or text viewport SHALL apply the step locally. It SHALL send a message beyond that component only when the resolved result requires an existing shell-owned effect or persistence operation; the shell SHALL NOT recompute the wheel step or re-resolve the pointed surface.

A migrated canonical media-list surface SHALL accept a wheel gesture only when its
embedded list control claims the pointed list region from its completed current-frame
retained result. An unmigrated destination MAY retain its existing compatibility
claim path until its own migration. A text viewport or irregular-row surface that competes with another eligible surface SHALL accept a wheel gesture only when the point is inside geometry published by its own painter. A sole eligible focused overlay SHALL accept its wheel gesture independently of pointer position; its local boundary clamp SHALL retain valid state.

#### Scenario: Wheel moves a canonical list by one row

- **WHEN** the user turns the wheel down once over a painted canonical media list with a following row
- **THEN** the list's owning component advances its local selection and viewport by one row
- **AND** no shell-side wheel movement is calculated for that list

#### Scenario: Wheel moves a text viewport by one line

- **WHEN** the user turns the wheel up once over a painted text viewport with a preceding line
- **THEN** the viewport's owning component moves its local offset back by one line
- **AND** its cursor or selection remains unchanged unless that surface's ordinary one-step navigation also changes it

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

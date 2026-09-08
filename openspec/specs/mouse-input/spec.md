# mouse-input Specification

## Purpose
Defines how raw terminal mouse events reach interactive components, how
overlapping hit claims between stacked surfaces are arbitrated, how pointer
gestures are recognized, and the mouse-parity contract every migrated interactive
surface must satisfy — so that mouse is a first-class interaction surface that new
gestures extend additively rather than a per-surface bolt-on.

## Requirements

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
pills, scope buttons, the seek bar and transport, overlay and popup regions. For
a canonical media-list row the parent SHALL delegate point resolution to the
embedded list control, passing the list rectangle the parent itself painted, and
SHALL NOT re-derive row coordinates itself. The parent SHALL emit a `Msg` for a
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

### Requirement: Overlapping hit claims are arbitrated before delivery

The shell SHALL determine which components are mouse-eligible for the current
frame and SHALL deliver mouse events only to those, in this order of precedence:

1. a mounted blocking overlay or modal — exclusively;
2. otherwise, the topmost mounted overlay or popup that paints over panel
   content — exclusively;
3. otherwise, the components painted in the current frame.

Arbitration SHALL take effect **before** a component's event handler runs, so a
losing component is never mutated. The shell SHALL NOT rely on discarding a
component's returned mouse message to prevent that component from acting.

At most one component's mouse message SHALL be applied for a single event. The
eligible components' painted regions do not overlap, so two claims for one event
indicate a geometry defect; the shell SHALL surface that as a failure in debug
builds rather than silently ranking them.

A component that holds keyboard focus receives events outside this eligibility
set as a framework property. A mounted blocking overlay SHALL therefore hold
keyboard focus, so that no surface beneath it can be both focused and obscured.

A popup that is not blocking SHALL still receive mouse events outside its own
geometry, so its dismissal policy applies; surfaces beneath it SHALL NOT act on
those events.

#### Scenario: A click falls where an overlay covers a panel

- **WHEN** an overlay is mounted over a panel and the user clicks a point inside
  both the overlay's and the panel's painted geometry
- **THEN** only the overlay's mouse handler runs and only its message is applied
- **AND** the panel's handler is not invoked, so the panel's state is unchanged

#### Scenario: A click outside a blocking modal

- **WHEN** a blocking modal is mounted and the user clicks outside it
- **THEN** the underlying surfaces' mouse handlers are not invoked and their
  state is unchanged
- **AND** the modal's own dismissal policy, if any, still applies

#### Scenario: A blocking overlay is mounted without keyboard focus

- **WHEN** a blocking overlay is mounted and the shell's synchronisation pass
  completes
- **THEN** the overlay holds keyboard focus
- **AND** a test asserts this, so a future change that mounts a blocking overlay
  without focusing it fails rather than leaking clicks to obscured surfaces

#### Scenario: Simultaneous Queue and Library are both pointable

- **WHEN** both the Queue and a Library destination are visible with no overlay
  mounted, and the user clicks first one then the other
- **THEN** each click is resolved and applied by the component that painted the
  region under it, independently, with focus following the click

### Requirement: Pointer gestures are recognized by the mounted parent

Each mounted destination parent SHALL recognize click, double-click, right-click,
wheel, and drag gestures from the raw mouse events it receives, using a private
`MouseGestureState`. The double-click interval and wheel throttle SHALL NOT be
held as shell-global state keyed by screen position. An embedded canonical
media-list control SHALL NOT recognize gestures — it only resolves a point
within the list rectangle its parent painted to a stable target, and the parent
delegates list-point resolution to it.

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

Hit geometry for a uniform row flow SHALL be resolved from the flow the control
already exports to its painter, not from a separately stored per-row rectangle
list. A stored rectangle registry is for irregular painted controls — pills,
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

### Requirement: Every migrated interactive surface has verified mouse parity

Every row in `docs/architecture/interactive-surface-ledger.md` SHALL record its
mouse ownership and the verification behind it, in the same way keyboard, state,
rendering, and geometry ownership are recorded. A row SHALL NOT be considered
complete while its mouse gestures are unverified.

For each surface the ledger SHALL state which component owns mouse hit-testing,
which gestures that surface supports, and the test or explicit manual validation
that confirms them. Panels SHALL support click-to-focus, click-to-select,
double-click-to-activate, wheel-scroll, and right-click-to-menu where the surface
has a corresponding keyboard action; overlays and popups SHALL support
click-to-select and click-to-dismiss where they have a corresponding keyboard
action. A surface with no meaningful pointer gesture SHALL say so explicitly.

A surface that renders at more than one breakpoint SHALL have its mouse
behaviour verified at each breakpoint it renders at, since its hit geometry
differs between them. A surface that exists at only one breakpoint SHALL record
which.

#### Scenario: A surface renders at both breakpoints

- **WHEN** a surface paints pointable regions in both the wide and narrow
  arrangements
- **THEN** its ledger row records mouse verification for both
- **AND** a verification at one breakpoint alone does not satisfy the row

#### Scenario: The ledger is checked for mouse completeness

- **WHEN** the change that restores mouse support is complete
- **THEN** every ledger row has a filled mouse ownership/verification cell
- **AND** no row defers mouse verification to a later pass

#### Scenario: A surface gains a keyboard action after mouse restoration

- **WHEN** a new keyboard-driven action is added to a migrated interactive
  surface
- **THEN** the equivalent pointer gesture is added in the same change, or the
  ledger row records why the action has no pointer equivalent

### Requirement: Mouse gesture recognition is verifiable without a terminal

The seams that make cross-surface mouse behaviour testable SHALL match those
already required for keyboard: the event-listener configuration substitutable at
`Model` construction, and the run loop's synchronisation sequence a single
callable unit. Cross-surface mouse properties — delivery set, arbitration
outcome, and blocking-overlay suppression — SHALL be verified by exercising
`Application::tick()` against the shell's own synchronisation order, not by
calling a component handler directly or hand-building the message list.

#### Scenario: The three deferred precedence proofs are executed

- **WHEN** the mouse-restoration change is complete
- **THEN** tests exercise, through `tick()`: a click routed to the correct one of
  two simultaneously visible panels, a blocking overlay suppressing a click on
  obscured content, and a component resolving a click from the same geometry it
  painted
- **AND** each test drives the real synchronisation order rather than a
  reconstructed one

### Requirement: The Queue panel boundary supports precise column resizing

When both panels are visible, the single full-height terminal column at the outer right edge of the Queue-side column SHALL be a mouse resize target. This is the root Queue column's trailing edge next to the Library panel, not the inset queue frame or list edge. Pressing the left button on that column and dragging horizontally SHALL resize the queue column live at one-column precision. The resulting width SHALL place the grabbed edge at the pointer column, subject to the same minimum and maximum bounds as keyboard resizing.

The resize target SHALL use the existing visual edge without adding a divider, gutter, hover treatment, or wider invisible hit region. A left press and release without drag motion SHALL leave the width unchanged. The final width SHALL be persisted when the drag ends, not once per drag event.

A dedicated Interactive Component SHALL be the sole painter and gesture owner of the boundary column. Root chrome and Queue destination hit geometry SHALL exclude that column. The boundary owner SHALL receive mouse events through normal component subscriptions, recognize the gesture locally, and emit resolved widths. The shell SHALL NOT re-resolve raw pointer coordinates. Queue row dragging and other panel mouse gestures SHALL remain independently owned and SHALL NOT activate from the boundary column.

#### Scenario: Drag resizes by one column

- **WHEN** both panels are visible and the user presses the Queue panel's right-edge column and drags it by one terminal column within the allowed range
- **THEN** the queue column becomes exactly one column wider or narrower during the drag
- **AND** the existing visual boundary follows the pointer without adding new chrome

#### Scenario: Drag is clamped to the existing bounds

- **WHEN** an active boundary drag moves beyond the minimum or maximum queue-column width
- **THEN** the live width remains at the existing bound nearest the pointer
- **AND** further motion beyond that bound does not exceed it

#### Scenario: Click without motion does not resize

- **WHEN** the user presses and releases the Queue panel boundary without drag motion
- **THEN** the queue-column width and persisted preference remain unchanged

#### Scenario: Final width is persisted once

- **WHEN** a boundary drag produces one or more live width changes and then ends
- **THEN** the final clamped width is persisted
- **AND** intermediate drag positions are not persisted individually

#### Scenario: Press outside the exact boundary does not arm resize

- **WHEN** the user presses in either panel outside the Queue panel's single-column right edge and then drags
- **THEN** queue-column resizing is not armed
- **AND** the component that owns that panel remains free to interpret the gesture normally

#### Scenario: Overlay suppresses boundary resizing

- **WHEN** an overlay or popup has exclusive mouse eligibility over the panels
- **THEN** the boundary owner does not receive the press or drag
- **AND** the queue-column width is unchanged

#### Scenario: Live tick delivers the complete resize gesture

- **WHEN** boundary press, drag, and release events are injected through the event listener and processed by the application's real tick and synchronization sequence
- **THEN** the boundary owner applies the resolved live width and persists the final width on release
- **AND** no Queue or Library destination handles the same gesture

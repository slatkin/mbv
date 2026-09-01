## Purpose

Defines how raw terminal mouse events reach interactive components, how
overlapping hit claims between stacked surfaces are arbitrated, how pointer
gestures are recognized, and the mouse-parity contract every migrated interactive
surface must satisfy — so that mouse is a first-class interaction surface that new
gestures extend additively rather than a per-surface bolt-on.

## ADDED Requirements

### Requirement: Every visible interactive surface receives mouse events

A mounted interactive component that paints a region the user can point at SHALL
receive every terminal mouse event while it is mounted, not only while it holds
keyboard focus. Mouse-event delivery SHALL use the component framework's
subscription mechanism; the shell SHALL NOT introduce a separate mouse event
loop, a global completed-frame hit map, or a global coordinate router.

A component SHALL decide whether an event is its own by testing the event
coordinates against the geometry it painted on its most recent render. A
component SHALL emit a `Msg` for a mouse event only when the coordinates fall
within a region it painted; otherwise it SHALL ignore the event.

#### Scenario: A click lands on a panel that does not hold focus

- **WHEN** keyboard focus is on one panel and the user clicks inside a different
  visible panel
- **THEN** the clicked panel's component receives the event, resolves the target
  from its own painted geometry, and acts on it
- **AND** the focused panel's component, receiving the same event with
  coordinates outside its geometry, produces no message

#### Scenario: Mouse events reach a subscribed component through a live tick

- **WHEN** a mouse event is injected into a mounted `Application` through its
  event listener and `tick()` is called
- **THEN** every subscribed component whose painted geometry contains the event
  coordinates is given the event
- **AND** no component's message for that event is produced twice

#### Scenario: Chrome that never holds focus is still clickable

- **WHEN** the user clicks a transport control or the seek bar in the playback
  chrome, which never receives keyboard focus
- **THEN** the playback component resolves the click against its painted control
  geometry and emits the corresponding transport or seek intent

### Requirement: Overlapping hit claims are arbitrated by a fixed surface priority

When more than one mounted component's painted geometry contains a mouse event's
coordinates, the shell SHALL resolve the conflict by a fixed priority order:
topmost overlay or modal, then the active panel, then any other visible panel,
then chrome. At most one component's mouse message SHALL be applied for a single
event.

While a blocking overlay or modal is mounted, mouse messages from components
beneath it SHALL be discarded, so a click on obscured content cannot mutate or
navigate it. A non-blocking popup SHALL NOT suppress mouse events outside its own
geometry.

#### Scenario: A click falls where an overlay covers a panel

- **WHEN** an overlay is mounted over a panel and the user clicks a point inside
  both the overlay's and the panel's painted geometry
- **THEN** only the overlay's mouse message is applied and the panel's is
  discarded

#### Scenario: A click outside a blocking modal

- **WHEN** a blocking modal is mounted and the user clicks outside it
- **THEN** the underlying surfaces receive no mouse mutation
- **AND** the modal's own dismissal policy, if any, still applies

#### Scenario: Simultaneous Queue and Library are both pointable

- **WHEN** both the Queue and a Library destination are visible with no overlay
  mounted, and the user clicks first one then the other
- **THEN** each click is resolved and applied by the component that painted the
  region under it, independently, with focus following the click

### Requirement: Pointer gestures are recognized per component

Each interactive component SHALL recognize click, double-click, right-click, and
wheel gestures from the raw mouse events it receives, using gesture state it owns
privately. The double-click interval and wheel throttle SHALL NOT be held as
shell-global state keyed by screen position.

A component SHALL translate a recognized gesture into a semantic typed `Msg`
carrying the resolved target (a row identity, a control, a pill index), never raw
coordinates for the shell to re-resolve. The shell handler for that `Msg` SHALL
accept the resolved target as an argument.

The gesture vocabulary SHALL be open to drag (`start`, `move`, `end`) and hover
(`enter`, `leave`) gestures without changing the delivery or arbitration
mechanism; those gestures are out of scope for this capability but SHALL NOT be
precluded by its design.

#### Scenario: A double-click activates the pointed row

- **WHEN** the user clicks the same row twice within the double-click interval
- **THEN** the component recognizes a double-click and emits the activation
  intent for that row's resolved identity
- **AND** a single click on the same row emits only a focus/selection intent

#### Scenario: A wheel event scrolls the pointed list

- **WHEN** the user turns the wheel over a scrollable list in any panel
- **THEN** that list's component scrolls its own viewport, subject to its own
  throttle, whether or not the list holds keyboard focus

#### Scenario: A right-click opens the context menu at the pointer

- **WHEN** the user right-clicks a selectable row on any migrated interactive
  surface that paints selectable rows
- **THEN** the row is focused and the context menu opens anchored at the click
  position

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

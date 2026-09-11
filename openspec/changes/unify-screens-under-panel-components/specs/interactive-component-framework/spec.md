## MODIFIED Requirements

### Requirement: Complete conversion with no mixed-framework endpoint

The migration MAY use internal checkpoints, behaviour-preserving commits, and
temporary adapters, but a mixed TuiRealm/legacy framework SHALL NOT be a
completed or mergeable endpoint. Completion requires that every row in
`docs/architecture/interactive-surface-ledger.md` is `migrated`; every
independently interactive surface is a TuiRealm `AppComponent`; component-local
state, handlers, and render adapters are removed from `App` rather than mirrored;
`CONTEXT_STACK` interaction dispatch, the global mouse router and hit map, and
duplicated mouse paths are removed; all temporary interaction adapters and
state mirrors are removed; no parallel legacy interaction framework remains;
and composition is owned by components: the root composes every visible
surface from panel components (see "The root composes every visible surface
from panel components"), no legacy base frame paints beneath them, and no
shell-wide struct carries painted geometry from paint to input. A ledger row
that records only where interaction state lives SHALL NOT be read as
completion; the ledger SHALL also record each surface's composing panel.

A `migrated` surface SHALL have exactly one painter for each frame at its
active layout breakpoint. The shell SHALL NOT run a legacy surface painter for
a surface body that a mounted component paints in the same frame. Verification
is execution ownership — the legacy painter is demonstrably not reached for
that surface at that breakpoint — not final-buffer similarity.

The per-frame placement computation the root reads (which panel occupies which
rect in the current Panel mode) MAY be shared shell code; it is paint-free and
carries no painted geometry back to input.

A legacy renderer SHALL NOT be the painter for any surface at any breakpoint at
completion. During the migration a legacy renderer MAY remain the sole painter
for a not-yet-migrated surface; the ledger row SHALL state this explicitly so
it is not mistaken for an underpaint, and completion requires none remain.

A `migrated` destination with a stable identity SHALL retain its private
interaction state (cursor, scroll, local focus, drafts) across destination
switches and layout-breakpoint changes. Leaving a destination and returning to
it SHALL restore the state it had on exit. A destination's content owner SHALL
be discarded only when its backing Service library is no longer in the live
catalog (Service disconnect, catalog refresh, library hidden or removed).

Whether a destination is the active, focused, and rendered content is a
per-frame decision driven by the current tab, panel focus, and layout
breakpoint; it SHALL be independent of whether its content owner exists.

The ledger, ADR 0022, and the source SHALL NOT contradict one another. A
`ComponentId` variant, ledger row, or documented owner that names a component
module which does not exist is such a contradiction and SHALL be reconciled by
deleting the phantom reference or implementing the component — whichever
matches the mechanism the code actually uses.

A keyboard-policy owner tag SHALL name a component that is really mounted at
the time the chord can fire, or `UiRoot` for a global chord. The single
keyboard router classifies a binding as global-versus-focused by its owner
tag; an owner that names an unmounted or non-existent component misclassifies
the binding and can leak a global chord into a focused text-entry surface.

Four framework behaviours — which component holds focus after the shell's
synchronisation pass, which components a terminal event is delivered to and in
what order, whether a blocking overlay withholds input from the surfaces
beneath it, and whether an injected `UserEvent` reaches its mounted target —
are properties of the composition, not of any one component. They SHALL be
verified by exercising `Application::tick()` against the shell's own
synchronisation order. A test that calls `Component::on` directly, hand-builds
the message list `tick()` would have returned, or re-lists the `sync_*` calls
in an order of its own choosing does not satisfy this requirement: each of
those substitutes the wiring under test for the test's own assumption about
it.

Consequently the shell SHALL expose the seams that make this verification
possible without a terminal: the event-listener configuration SHALL be
substitutable at `Model` construction, and the run loop's synchronisation
sequence SHALL be a single callable unit rather than a statement list inlined
in the loop body.

#### Scenario: A converted surface does not regain App-owned state

- **WHEN** a surface has been marked `migrated`
- **THEN** it does not reintroduce `App`-owned local state, input handling, or
  rendering
- **AND** the old fields and handlers are deleted, not synchronised with a mirror

#### Scenario: A mid-migration surface is tracked as `component`, not `migrated`

- **WHEN** a surface's Interactive Component has landed and paints the surface,
  but the shell still mirrors `App` state into it or legacy input still
  forwards to an `impl App` handler
- **THEN** its ledger row reads `component`, which is a permitted internal
  checkpoint and not a completed conversion
- **AND** the row becomes `migrated` only once the `App` state and handlers are
  deleted and the mirror is removed
- **AND** the completion gate requires no `legacy` and no `component` row to
  remain

#### Scenario: Migration preserves existing contracts

- **WHEN** any surface is converted
- **THEN** existing keyboard precedence, responsive behaviour, images-disabled
  behaviour, and render characterization coverage remain satisfied
- **AND** full mouse parity for the surface is required as defined by the
  `mouse-input` capability, not deferred to a later pass

#### Scenario: A terminal event is delivered through a live tick

- **WHEN** a key event is injected into a mounted `Application` through its
  event listener and `tick()` is called
- **THEN** the focused component receives the event and its message appears
  first
- **AND** the permanently subscribed `UiRoot` observer's message appears second
- **AND** neither message is produced twice for a single injected event

#### Scenario: A mouse event is delivered to subscribed non-focused components

- **WHEN** a mouse event is injected into a mounted `Application` through its
  event listener and `tick()` is called
- **THEN** every mounted component subscribed to mouse events is given the event,
  regardless of which component holds focus
- **AND** a mounted component that the shell has not made mouse-eligible for the
  current frame is not given the event at all, so its handler cannot mutate it
- **AND** the shell applies at most one component's resulting message
- **AND** no component's message for that event is produced twice

#### Scenario: Focus after the synchronisation pass is asserted in its real order

- **WHEN** the shell's full synchronisation sequence runs as one unit and the
  Queue panel holds focus
- **THEN** `Application::focus()` is the Queue component when the sequence
  completes
- **AND** the assertion is made after the whole sequence, so a later
  synchronisation step that reactivates a different component fails the test

#### Scenario: A blocking overlay withholds input from the surfaces beneath it

- **WHEN** a blocking overlay is mounted and the synchronisation sequence runs
- **THEN** the overlay still holds focus when the sequence completes
- **AND** a key injected through the listener is delivered to the overlay, not
  to Queue or the active destination
- **AND** a global chord resolves to a swallow rather than reaching a surface
  beneath the overlay
- **AND** a mouse event on obscured content produces no message from a surface
  beneath the overlay

#### Scenario: An injected user event reaches its mounted component

- **WHEN** a `UserEvent` is published through an event-listener port and
  `tick()` is called
- **THEN** the mounted component subscribed to that event observes it
- **AND** the shell-side path that ships in production for the same effect is
  covered by its own assertion, so replacing one with the other is a visible
  change

#### Scenario: A migrated surface body is painted once per frame

- **WHEN** a mounted component is the active painter for a surface at the
  current layout breakpoint
- **THEN** the legacy renderer for that surface body is not reached that frame
- **AND** a debug assertion or test counter that fires when the legacy painter
  runs while the component is active stays silent across the surface's render
  characterization tests

#### Scenario: Geometry is computed without painting owned surfaces

- **WHEN** the shell computes the per-frame panel placement the root reads
- **THEN** that computation produces only paint-free placement facts
- **AND** it paints nothing and records no painted geometry for input

#### Scenario: Startup and steady-state frames paint identically

- **WHEN** the first full frame is drawn at startup and any later frame is drawn
- **THEN** both go through the same shell draw entry point
- **AND** the first frame includes the component views, showing loading
  affordances rather than a chrome-only frame followed by a component pop-in

#### Scenario: A breakpoint with no component keeps a sole legacy painter

- **WHEN** during the migration a surface is shown at a layout breakpoint for
  which it is not yet composed by a panel component
- **THEN** the legacy renderer is the only painter for that surface at that
  breakpoint and the ledger row records it
- **AND** the migration is not complete while any such row remains

#### Scenario: Destination state survives a switch away and back

- **WHEN** the user scrolls or moves the cursor in a destination surface, then
  switches to another library or destination, then returns
- **THEN** the returned surface shows the same cursor position and scroll
  offset it had when the user left it
- **AND** no legacy base-frame painter is relied on to reconstruct that state

#### Scenario: Destination state survives a layout-breakpoint change

- **WHEN** a wide destination workspace is showing and the terminal is resized
  below its wide breakpoint and then back above it
- **THEN** the wide workspace shows the interaction state it had before the
  resize

#### Scenario: A destination component is torn down when its library is gone

- **WHEN** a Service disconnects or its library catalog is refreshed such that
  a library backing a destination content owner is no longer present
- **THEN** the Library panel discards that content owner
- **AND** content owners for libraries still in the catalog are retained

#### Scenario: A mounted but inactive destination is inert

- **WHEN** a destination content owner is retained but is not the active
  destination for the current tab and layout
- **THEN** it receives no input events and supplies no content to paint
- **AND** it cannot take focus away from the Library panel, Queue, or an
  overlay

#### Scenario: A ledger row and ComponentId agree with the code

- **WHEN** a ledger row or `ComponentId` variant names a component module
- **THEN** that module exists and is mounted by the shell
- **AND** if the surface's ownership is instead pure derivation over shell
  state with no component, the ledger row describes that mechanism and no
  `ComponentId` variant is reserved for the absent component

#### Scenario: A global chord does not fire while a text field is focused

- **WHEN** a global chrome chord (for example the panel-mode cycle) is pressed
  while a text-entry surface owns focus
- **THEN** the router treats the chord as global input, suppresses the command,
  and the character reaches the focused field
- **AND** this holds because the chord's keyboard-policy owner is `UiRoot`, not
  a component that is never mounted


## ADDED Requirements

### Requirement: The root composes every visible surface from panel components

Every visible cell of a frame SHALL be painted by a mounted component composed by the root: the Tab
panel, the Library panel, the Library playback panel, the Queue panel, the Queue playback panel, the
Status bar panel, the pane boundaries, and the overlay stack. The root SHALL own the frame's split
into panels. There SHALL be no legacy base frame painted beneath the components, no shell-painted
backdrop or chrome, and no hand-ordered list of per-destination render calls in the draw path.

A panel that has no content to paint in a Panel mode SHALL be unmounted in that mode, never mounted
with an empty rect: the Library playback panel exists only while the queue column is hidden; the
Queue panel, Queue playback panel and Queue boundary only while the queue column is visible (the Queue
boundary only in the two-panel layout); the Tab, Library and Status bar panels only while the library
column is visible.

#### Scenario: A frame is drawn
- **WHEN** any frame is drawn in any Panel mode
- **THEN** every cell belongs to exactly one mounted panel component's placement or to an overlay,
  and each panel paints its own surface fill across its whole placement
- **AND** no cell is first painted by a shell base frame and then overpainted by a component

#### Scenario: A panel with no content in a Panel mode
- **WHEN** the layout is queue-only
- **THEN** the Tab, Library, Library playback and Status bar panels are not mounted
- **AND** no mounted panel has an empty placement

#### Scenario: A destination changes
- **WHEN** the selected library tab changes
- **THEN** the same Library panel component paints the frame's library area with the new
  destination's content
- **AND** the draw path does not select a different per-destination render call

### Requirement: Panels own layout and destinations supply only typed slot content

A panel component SHALL own the geometry of everything inside it. A destination component SHALL
contribute to a panel only by producing the typed content the panel's slots define, and SHALL keep its
interaction state (cursor, scroll, focus, drafts) and its embedded media lists as today. A component's
`view()` SHALL NOT construct shell-wide geometry, SHALL NOT call a per-destination free painter that
lays out a pane, and SHALL NOT paint outside the rect its parent gives it. Hit geometry SHALL be
retained by the component that painted it; no shell-wide struct SHALL carry painted rects from paint to
input.

#### Scenario: A destination needs a new visual element
- **WHEN** a destination's content has no slot in its panel's content type
- **THEN** the destination cannot render it
- **AND** adding it requires adding the slot to the panel for every destination

#### Scenario: A click lands on a painted element
- **WHEN** the user clicks a pill, row, tab, control or boundary
- **THEN** the component that painted that element in the latest frame resolves the click from its own
  retained geometry

## MODIFIED Requirements

### Requirement: Complete conversion with no mixed-framework endpoint

The migration MAY use internal checkpoints, behaviour-preserving commits, and
temporary adapters, but a mixed TuiRealm/legacy framework SHALL NOT be a
completed or mergeable endpoint. Completion requires that every row in
`docs/architecture/interactive-surface-ledger.md` is `migrated`; every
independently interactive surface is a TuiRealm `AppComponent`; component-local
state, handlers, and render adapters are removed from `App` rather than mirrored;
`CONTEXT_STACK` interaction dispatch, the global mouse router and hit map, and
duplicated mouse paths are removed; render-only layout state MAY remain; all
temporary interaction adapters and state mirrors are removed; and no parallel
legacy interaction framework remains.

A `migrated` destination surface with a stable identity SHALL retain its
component-private interaction state (cursor, scroll, local focus, drafts)
across destination switches and layout-breakpoint changes. Leaving a
destination and returning to it SHALL restore the state it had on exit. A
destination component SHALL be torn down only when its backing Service library
is no longer in the live catalog (Service disconnect, catalog refresh, library
hidden or removed).

Whether a destination component is the active, focused, and rendered target is
a per-frame decision driven by the current tab, panel focus, and layout
breakpoint; it SHALL be independent of whether the component is mounted.

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
- **AND** mouse parity is required only for the alpha-supported mouse paths named
  by this capability

#### Scenario: A terminal event is delivered through a live tick

- **WHEN** a key event is injected into a mounted `Application` through its
  event listener and `tick()` is called
- **THEN** the focused component receives the event and its message appears
  first
- **AND** the permanently subscribed `UiRoot` observer's message appears second
- **AND** neither message is produced twice for a single injected event

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

#### Scenario: An injected user event reaches its mounted component

- **WHEN** a `UserEvent` is published through an event-listener port and
  `tick()` is called
- **THEN** the mounted component subscribed to that event observes it
- **AND** the shell-side path that ships in production for the same effect is
  covered by its own assertion, so replacing one with the other is a visible
  change

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
  a library backing a mounted destination component is no longer present
- **THEN** that destination component is unmounted
- **AND** components for libraries still in the catalog remain mounted

#### Scenario: A mounted but inactive destination is inert

- **WHEN** a destination component is mounted but is not the active target for
  the current tab and layout
- **THEN** it receives no input events and paints nothing
- **AND** it does not take focus away from the active destination, Queue, or an
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

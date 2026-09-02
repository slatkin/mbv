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

A `migrated` surface SHALL have exactly one painter for each frame at its
active layout breakpoint. The shell SHALL NOT run a legacy surface painter for
a surface body that a mounted component paints in the same frame. Verification
is execution ownership — the legacy painter is demonstrably not reached for
that surface at that breakpoint — not final-buffer similarity.

The per-frame geometry computation that components read (the `AppLayout` and
equivalent facts) MAY be shared shell code and is not a "parallel legacy
framework"; it paints nothing that a component owns.

Where a surface has a component variant at one breakpoint and only a legacy
renderer at another (for example a wide workspace component and a narrow
legacy body), the legacy renderer at the breakpoint with no component is the
**sole** painter for that breakpoint. The ledger row SHALL state this
explicitly so it is not mistaken for an underpaint.

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
- **AND** the shell applies at most one component's resulting message, chosen by
  the `mouse-input` arbitration priority
- **AND** no component's message for that event is produced twice
- **AND** each parent-produced mouse message carries a runtime-only originating
  mounted surface/source tag and semantic message envelope that the shell fold
  arbitrates before unwrapping the winner

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

- **WHEN** the shell computes the per-frame layout that components read
- **THEN** that computation produces the `AppLayout` and equivalent facts
- **AND** it paints no surface body that a component owns this frame

#### Scenario: Startup and steady-state frames paint identically

- **WHEN** the first full frame is drawn at startup and any later frame is drawn
- **THEN** both go through the same shell draw entry point
- **AND** the first frame includes the component views, showing loading
  affordances rather than a chrome-only frame followed by a component pop-in

#### Scenario: A breakpoint with no component keeps a sole legacy painter

- **WHEN** a surface is shown at a layout breakpoint for which no component
  variant exists (for example narrow TV or narrow Music)
- **THEN** the legacy renderer is the only painter for that surface at that
  breakpoint
- **AND** the ledger row records "wide: component; narrow: sole legacy
  renderer" so the endpoint is unambiguous

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

### Requirement: Every component request has a handler or a documented no-op, enforced by exhaustive matching

Every typed request a component emits across its authority boundary — each
`ShellRequest` variant and each intent sub-enum variant reaching the shell —
SHALL resolve in the shell's dispatch to either a real handler or an explicit
arm whose comment names why it is deliberately inert (the component owns the
effect, consumed synchronously elsewhere, or the issue that owns the missing
wiring). A request reaching the shell from a mouse gesture is not a standing
reason to be inert: it SHALL have a real handler unless one of the other reasons
applies.

The shell's top-level request dispatch (`Model::handle_terminal_message`,
destructuring `Msg::Shell(request)`) SHALL be an exhaustive `match` over
`ShellRequest` with no wildcard arm, so that a request variant with no arm is a
compile error rather than a silent fall-through. A wildcard arm that catches
"unhandled request variant" and a documented no-op arm are not equivalent: the
first is an accident that repeats, the second is a recorded decision.

A wildcard (`_`) arm is permitted only in an inner sub-dispatcher that the
exhaustive top-level match has already narrowed to a fixed OR-group of variants,
and only with a comment stating the closed set it matches and why the wildcard
is unreachable. Such an arm SHALL NOT be the enforcement mechanism for handler
coverage — the top-level exhaustive match is.

#### Scenario: A new request variant without a dispatch arm fails compilation

- **WHEN** a `ShellRequest` variant is added and no arm is added to
  `Model::handle_terminal_message`
- **THEN** `cargo check -p mbv` fails, naming the unhandled variant
- **AND** the build cannot be made to pass by relying on a wildcard arm, because
  the top-level match has none

#### Scenario: A deliberately inert request is an explicit arm

- **WHEN** a request variant is fully handled by the emitting component, or is
  consumed by a synchronous handler before `handle_terminal_message`
- **THEN** its arm in the dispatch is an explicit no-op whose comment states
  that reason and the issue or precedent that owns it
- **AND** it is not folded into a catch-all arm

#### Scenario: A mouse-emitted request is handled, not inert by default

- **WHEN** a `ShellRequest` variant is emitted from a mouse gesture and crosses
  the shell boundary for a navigation, playback, persistence, focus, or Service
  effect
- **THEN** its dispatch arm runs a real handler
- **AND** an inert arm for it is permitted only when the emitting component fully
  owns the effect

#### Scenario: An inner sub-dispatcher wildcard matches a proven closed set

- **WHEN** a shell sub-dispatcher (for example `handle_browser_request`) is
  reached only for a fixed OR-group of `ShellRequest` variants routed by the
  exhaustive top-level match
- **THEN** any `_` arm it carries has a comment naming that closed set and why
  the arm is unreachable
- **AND** removing or reordering the top-level OR-group that feeds it is what
  would change its reachability, not an unnoticed new variant

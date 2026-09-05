# interactive-component-framework Specification

## Purpose
Defines how every independently interactive terminal surface in mbv is owned:
as a TuiRealm `AppComponent` with private presentation state and a typed message
boundary, so that interaction ownership is uniform, the shell keeps runtime and
playback authority, and input precedence, mouse behaviour, and hit geometry stay
correct after the migration off the legacy `App`/`CONTEXT_STACK`/`AppLayout`
framework.

## Requirements

### Requirement: Interactive surfaces are TuiRealm components

Every independently interactive surface — one that owns a cursor, scroll,
selection, focus, form draft, or its own key/mouse interpretation — SHALL be a
TuiRealm `AppComponent` mounted in the application's
`Application<ComponentId, Msg, UserEvent>`. TuiRealm SHALL own mounting,
unmounting, focus, the focus stack, subscriptions, terminal-event delivery, and
the entry point that renders each component. mbv SHALL NOT define a parallel
component trait, component registry, event dispatcher, focus framework, generic
effect scheduler, or Flux store alongside TuiRealm.

Render Components (rows, cards, heroes, pills, modal frames, scrollbars, and
other painters under `src/app/render/components/`) are NOT interactive surfaces
and remain plain painting functions invoked from a component's rendering.

#### Scenario: An overlay is opened and dismissed

- **WHEN** the user opens an overlay (Search, Settings, Sessions, Playlists,
  Help, context menu, or a modal)
- **THEN** its component is mounted and receives focus
- **AND** when it is dismissed the component is unmounted and focus returns
  deterministically to the surface that had it before, independent of mount order

#### Scenario: A destination retains its state while inactive

- **WHEN** the user navigates away from a Library destination and later returns
- **THEN** its component MAY remain mounted so its private cursor and scroll are
  preserved
- **AND** a component for a Service library that has been removed is unmounted

### Requirement: Interactive components own only presentation authority

An Interactive Component SHALL own its cursor, scroll, local focus, selection, filters, form drafts, local loading/error/result presentation state, local event interpretation and update rules, rendering, viewport, and render-derived hit geometry. It SHALL emit a `Msg` only for work that crosses its authority boundary — navigation, playback, Service access, persistence, mounting or dismissing another component, or changing focus.

An Interactive Component SHALL NOT receive or hold `App`, a Service client, credentials, `Config`, `PlayerProxy`, `RemotePlayer`, a protocol object, an `mpsc` channel, an `Arc<Mutex<_>>` integration lock, source URLs or headers, or arbitrary Ratatui `Color`/`Style`. It MAY receive owned presentation models containing text, durations, badges, semantic focus/selection/disabled state, image cache keys, semantic variant/policy values, and opaque action keys (for example a `QueueSlotId` the shell resolves).

A component's own cursor, scroll, or selection value MUST NOT be written into a shell (`App`) field for the sole purpose of being read back by a shell-side handler invoked immediately afterward. When a component-owned value determines which shell-owned effect runs, the component SHALL pass that value as a parameter of the typed `Msg` it emits, and the shell-side handler SHALL accept it as an argument rather than re-reading it off `App`. A shell field that exists only to close this loop is a forbidden mirror, not a sanctioned content push: pushing validated shell-owned content (list rows, setting values, setup drafts, and similar presentation content the shell computed) into a component remains sanctioned and is unaffected by this rule.

When a component-local movement also has to drive shell-owned persisted or
effectful state (pagination, position persistence, navigation-idle timers),
the component SHALL resolve the movement itself and carry the resolved value
in the `Msg`; the shell SHALL apply that value directly rather than
independently recomputing the same movement. The shell SHALL NOT write a
value the component painted (a scroll offset, a resolved cursor) back into
component-local state on every render pass; it MAY push shell-owned
navigation state (a browse level's resting cursor/scroll) into the component
only at the discrete event where the visible level changes.

A movement stride (page size, column count, or equivalent) used to resolve a
component-local movement SHALL have exactly one source. Where the component
resolves the movement, that source is the component's own painted geometry;
the shell SHALL NOT apply a second stride to the same movement.

Where a projection replaces a component's state wholesale, the component's own
interaction values SHALL take precedence over the incoming snapshot's
unconditionally. When the projected content no longer contains the item a
component-owned value referred to, the component SHALL reset that value to its
own default or clamp it against the new content; it SHALL NOT fall through to
the value carried in the shell's snapshot.

A component SHALL NOT hold echo-detection state: a field whose purpose is to
distinguish the component's own writes from values arriving in a shell
projection (for example a stored copy of the last pushed cursor, compared
against the current one to decide whether to adopt an incoming value). Such a
field is evidence of two owners. Where the shell must move a component-owned
cursor, it SHALL do so through an explicit re-anchor at the navigation event
that requires it, not by an equality test evaluated on every content push.

A type projected from the shell into an Interactive Component SHALL NOT carry
a field the component owns. Content the shell computes and interaction state
the component owns SHALL be separate types, so that a projection cannot
overwrite an interaction value and no component needs to save and restore its
own fields around one.

Where a cursor or scroll value is both interacted with and persisted, the live
value and the persisted resting position SHALL be distinct, separately named
state. The component owns the live value; the shell owns the resting position
and writes it only at a navigation event.

#### Scenario: Local interaction does not become a global message

- **WHEN** the user moves a cursor, scrolls, cycles a filter chip, or edits a local form field
- **THEN** the owning component updates its private state directly
- **AND** it emits no `Msg`

#### Scenario: Cross-boundary work is a typed request

- **WHEN** a component needs playback, navigation, Service access, or persistence
- **THEN** it emits a typed `Msg` describing the request
- **AND** the shell Model performs the effect; the component neither calls the Player/Service nor mutates the canonical queue

#### Scenario: A component-owned cursor drives a shell-owned effect without a round trip

- **WHEN** the Settings/Services component's local cursor determines which setting or service entry the user activated
- **THEN** the component emits `SettingsIntent::Activate` or `ServiceRequest::ActivateService` carrying that cursor as a value
- **AND** the shell resolves the target and calls the shell-side handler with that resolved value directly
- **AND** no `App` field stores the cursor for the handler to read back

#### Scenario: A local movement that also persists carries its resolved value once

- **WHEN** the user moves the cursor in a component whose movement also
  drives shell-owned pagination or position persistence (for example the
  Emby generic/Movies/HomeVideos browser)
- **THEN** the component updates its own cursor locally and emits a `Msg`
  carrying the resolved index it landed on
- **AND** the shell applies that index directly to its persisted state and
  runs the associated effects, without independently recomputing the
  movement

#### Scenario: Audiobookshelf show and book movement carries a resolved value

- **WHEN** the user moves the show cursor, the book cursor, the surname-bucket
  pill, or the chapter focus in an Audiobookshelf browser
- **THEN** the component resolves the movement against its own content and
  geometry and emits a `Msg` carrying the resolved index, bucket position, or
  chapter selection
- **AND** the shell applies that value through its existing index-taking entry
  point, running the position-save and detail-fetch effects unchanged
- **AND** no `App` helper recomputes the same movement from a delta

#### Scenario: Paging uses one stride

- **WHEN** the user pages a component-owned list whose movement also drives a
  shell-owned effect
- **THEN** the page stride comes from the component's painted geometry alone
- **AND** the shell does not re-page the same movement with a stride of its own

#### Scenario: A projection never reinstates a component value the shell happens to hold

- **WHEN** the shell pushes content in which the item a component-owned
  selection referred to is no longer present
- **THEN** the component resets that selection, and any scroll, filter, or
  sub-selection derived from it, to its own defaults
- **AND** the values carried in the shell's snapshot for those fields are
  discarded rather than adopted

#### Scenario: A shell re-anchor lands regardless of prior local movement

- **WHEN** the shell re-anchors a component-owned cursor at a navigation event
  (a group switch, a recursive activation, or a saved-position restore)
- **THEN** the component adopts the re-anchored value
- **AND** the outcome does not depend on whether the user moved that cursor
  since the previous projection

#### Scenario: Ordinary content pushes leave a component cursor alone

- **WHEN** the shell pushes refreshed content without a navigation event
- **THEN** the component's cursor, scroll, and local focus are unchanged
- **AND** the component holds no stored copy of a previously pushed value in
  order to reach that outcome

#### Scenario: A projected content type carries no component-owned field

- **WHEN** the shell projects browse content into a component
- **THEN** the projected type contains only content the shell computed
- **AND** the component's cursor, scroll, selection, and local filters are
  absent from it, so the component neither saves nor restores its own state
  around the projection

#### Scenario: Live cursor and resting position are distinct state

- **WHEN** the user moves the cursor on a visible browse level, and the shell
  later persists that level's position or restores it on re-entry
- **THEN** the live cursor is read from the component that owns it
- **AND** the persisted resting position is separate state the shell writes at
  the navigation event, not the same field serving both purposes

#### Scenario: Painted output is not written back into shell state every frame

- **WHEN** a component renders and produces a final scroll offset or other
  paint-derived value
- **THEN** the shell does not copy that value into its own persisted state
  as part of the render pass
- **AND** any persistence of that value happens only at the navigation event
  that actually changes which content is visible, not on every paint

### Requirement: The shell Model retains runtime authority

The application Model SHALL retain terminal setup/teardown, redraw cadence, and
fatal I/O handling; Remote Service clients, credentials, startup, and worker
lifecycle; Player ownership, canonical queue mutation, and reconciliation;
daemon/Local-daemon/`mbvd`, ctrl, and shared-data protocols; persistence and
external effects; and conversion of runtime state into owned presentation models.
The Model SHALL hold the TuiRealm `Application` but SHALL NOT become a second
global UI state store holding component-local interaction state.

This migration SHALL NOT change any daemon, Local-daemon, `mbvd`, ctrl,
shared-data, provider, playback, or canonical-queue behaviour.

#### Scenario: Runtime completion reaches a component without a lock

- **WHEN** an asynchronous runtime completion (startup, library, Search, session,
  cast, shared-data, feed, image, websocket, or ABS socket) arrives
- **THEN** it is delivered to the subscribed component as a TuiRealm `UserEvent`
  or via a minimal shell adapter carrying an owned presentation model
- **AND** the component receives no channel, client, or lock

#### Scenario: Stale completion guards are preserved

- **WHEN** a completion arrives whose Service setup generation, queue revision,
  session generation, or image key no longer matches current intent
- **THEN** it is discarded exactly as before the migration
- **AND** no component-owned state is updated from the stale completion

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

### Requirement: Interactive surfaces have one owner and one painter

Every interactive surface SHALL have exactly one component owning its
interaction state, and exactly one painter, at every reachable breakpoint.

#### Scenario: An interactive surface has one cursor owner at every breakpoint

- **WHEN** a surface is reachable at a layout breakpoint
- **THEN** exactly one mounted component owns its cursor, scroll, and keyboard
  handling at that breakpoint
- **AND** a breakpoint gate on a mount decision leaves no reachable width at
  which the surface has no owner

#### Scenario: A painted surface has exactly one painter

- **WHEN** a component and a legacy renderer are both capable of painting the
  same rect
- **THEN** exactly one of them runs for that rect at that breakpoint
- **AND** the surface ledger records the owner and the painter, per breakpoint

#### Scenario: A mounted component is placed in a non-empty rect

- **WHEN** a component is mounted and owns a surface at a breakpoint
- **THEN** the shell places its view in the rect that surface actually occupies
  at that breakpoint
- **AND** a placement rect that is empty at some breakpoint is a violation, not
  a way to disable a component

#### Scenario: Components issue no effects while painting

- **WHEN** a surface's composition moves into its owning component
- **THEN** no image fetch, content fetch, or other effect executes inside the
  component
- **AND** effects the legacy painter performed inline are relocated to the
  shell, driven by the component's projected state

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

### Requirement: Mounted parents recognize mouse gestures and embedded controls resolve targets

A mounted destination `AppComponent` SHALL own its TuiRealm mouse subscription
and its `MouseGestureState`. An embedded media-list control SHALL resolve a point
within the list rectangle its parent painted to a stable target, using the same
row flow it exports to that parent's painter. After the parent recognizes a
mouse gesture, it SHALL delegate point resolution to the embedded control and
translate the returned stable target into the destination request.

An embedded control SHALL NOT subscribe independently, own a second gesture
recognizer, store a per-row rectangle list duplicating its exported row flow, or
publish row rectangles into a parent-owned hit map. Parent-owned controls outside
the list rectangle, such as pills or Queue scope buttons, MAY retain separate
parent hit regions, populated where those rectangles are painted. When a
recognized point falls within the embedded list rectangle, the embedded control's
explicit list targets SHALL be resolved before any parent workspace target. This
change owns adding point resolution to the already-landed `WideMediaList` and
`InlineMediaBrowser`, migrating every per-surface canonical row-hit `*HitRegion`
enum onto it, and deleting those enums; no `compose-canonical-media-lists` slice
performs any part of that migration.

#### Scenario: A pointer gesture targets a list row

- **WHEN** the mounted parent recognizes a click, double click, context click, or
  scroll gesture over its embedded list rectangle
- **THEN** the parent passes the list rectangle it painted and the point to the
  embedded control, which resolves it from the row flow it exported to that same
  paint
- **AND** it returns the stable target or list-local scroll result to the parent
- **AND** neither the parent nor shell recomputes the row from coordinates

#### Scenario: A pointer gesture targets a parent control

- **WHEN** the mounted parent recognizes a gesture over a pill, Queue scope
  button, or another region outside the embedded list rectangle
- **THEN** the parent resolves that separately owned region
- **AND** the embedded control's hit regions remain limited to its own painted
  rectangle

#### Scenario: Queue migrates mouse hit ownership

- **WHEN** Queue composes the canonical fixed-row control
- **THEN** Queue's parent keeps the subscription, gesture state, and scope-button
  geometry
- **AND** the embedded control resolves a point in the painted row area to a
  `QueueSlotId`
- **AND** Queue's `QueueHitRegion` enum is deleted once its row hits resolve
  through the embedded control

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

### Requirement: Mounted-component focus has one authority

Whether a mounted Interactive Component is focused SHALL be determined by the application's component-focus lifecycle. A component whose input handling or presentation depends on focus SHALL observe that lifecycle directly. The shell SHALL NOT also carry the same focused state in content projections, refresh content solely to change focused presentation, or maintain a second component-focus mirror.

Component-private pane focus, cursor, scroll, and selection SHALL remain owned by the component while mounted. Losing component focus SHALL suppress focused presentation without erasing those local values; regaining component focus SHALL immediately restore keyboard delivery and derive the focused pane from the component's retained local state.

A plain embedded Component SHALL share the mounted parent's focus boundary rather than becoming an independently focused application surface.

#### Scenario: Panel focus moves from Music to Queue and back

- **WHEN** the Music destination is mounted and Panel focus moves from Library to Queue and then back to Library
- **THEN** Music loses focused presentation while Queue holds focus
- **AND** Music receives keyboard navigation immediately when Library regains focus, without requiring a click or content refresh
- **AND** its component-private cursor, scroll, and pane focus remain as they were before the round trip

#### Scenario: Wide TV loses focused row treatment

- **WHEN** Wide TV is mounted with a selected series row and Panel focus moves from Library to Queue
- **THEN** the TV library rail loses its focused background, marker, and selected-row treatment on the next frame
- **AND** its selected series identity remains available for when Library focus returns

#### Scenario: Content refresh cannot overwrite focus

- **WHEN** shell-owned content is pushed to a mounted component while another component holds focus
- **THEN** the content refresh does not make the receiving component appear or behave focused
- **AND** a later focus transition does not require that content to be pushed again

#### Scenario: Overlay focus restores the underlying component

- **WHEN** a blocking overlay takes focus from a mounted destination and is then dismissed
- **THEN** the destination loses focused presentation while the overlay is active
- **AND** the application's focus restoration makes the destination focused again without a focus-only content projection

#### Scenario: Embedded controls share parent focus

- **WHEN** a mounted destination composes a plain embedded list, browser, or text-entry control
- **THEN** the embedded control is treated as focused only when its mounted parent and the applicable component-private pane are focused
- **AND** the embedded control is not mounted or focused independently

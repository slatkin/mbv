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

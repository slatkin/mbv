## Purpose

Defines how every independently interactive terminal surface in mbv is owned:
as a TuiRealm `AppComponent` with private presentation state and a typed message
boundary, so that interaction ownership is uniform, the shell keeps runtime and
playback authority, and input precedence, mouse behaviour, and hit geometry stay
correct after the migration off the legacy `App`/`CONTEXT_STACK`/`AppLayout`
framework.

## ADDED Requirements

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

An Interactive Component SHALL own its cursor, scroll, local focus, selection,
filters, form drafts, local loading/error/result presentation state, local event
interpretation and update rules, rendering, viewport, and render-derived hit
geometry. It SHALL emit a `Msg` only for work that crosses its authority
boundary — navigation, playback, Service access, persistence, mounting or
dismissing another component, or changing focus.

An Interactive Component SHALL NOT receive or hold `App`, a Service client,
credentials, `Config`, `PlayerProxy`, `RemotePlayer`, a protocol object, an
`mpsc` channel, an `Arc<Mutex<_>>` integration lock, source URLs or headers, or
arbitrary Ratatui `Color`/`Style`. It MAY receive owned presentation models
containing text, durations, badges, semantic focus/selection/disabled state,
image cache keys, semantic variant/policy values, and opaque action keys (for
example a `QueueSlotId` the shell resolves).

#### Scenario: Local interaction does not become a global message

- **WHEN** the user moves a cursor, scrolls, cycles a filter chip, or edits a
  local form field
- **THEN** the owning component updates its private state directly
- **AND** it emits no `Msg`

#### Scenario: Cross-boundary work is a typed request

- **WHEN** a component needs playback, navigation, Service access, or persistence
- **THEN** it emits a typed `Msg` describing the request
- **AND** the shell Model performs the effect; the component neither calls the
  Player/Service nor mutates the canonical queue

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

### Requirement: Input precedence preserved through focus and subscriptions

The input-resolution model of ADR 0002 SHALL be preserved: a priority-ordered
stack of active contexts in which each context resolves a key to `Command`,
`Swallow`, or `FallThrough`, and the first context returning `Command` or
`Swallow` claims the key. Only `FallThrough` SHALL allow a lower-priority context
to receive the key. The active interactive leaf is selected by TuiRealm focus;
blocking overlays are active components that `Swallow` bound and unbound keys;
parent and global bindings are delivered through TuiRealm subscriptions plus mbv
key-policy code, without broadcasting state-changing events to every component.
The `CONTEXT_STACK` loop SHALL NOT be retained as a parallel routing endpoint, but
the precedence order and the Command/Swallow/FallThrough semantics it encodes
SHALL be preserved and remain locked by the existing input characterization tests.

#### Scenario: A blocking overlay swallows input

- **WHEN** a blocking overlay (context menu or a modal) is active and any key is
  pressed
- **THEN** the overlay interprets or swallows the key
- **AND** the underlying surface receives no key and does not quit the app

#### Scenario: An unhandled key does not leak past a swallowing context

- **WHEN** the active context has no command bound for a key and the precedence
  table marks that context as blocking
- **THEN** the key is resolved as `Swallow` and no lower-priority context or global
  subscription receives it
- **AND** a key reaches a lower context only when the higher context resolves it as
  `FallThrough`

#### Scenario: Global and parent bindings keep their precedence

- **WHEN** a key bound at the global or parent level is pressed while a leaf is
  focused
- **THEN** the binding resolves with the same precedence it has today
- **AND** the focused leaf does not shadow a higher-precedence global binding

### Requirement: Component-owned hit geometry and overlay mouse blocking

Mouse hit targets SHALL be computed by the component that painted the region,
from the same geometry it used to paint, so painting and hit testing cannot
drift. Two surfaces that are simultaneously visible SHALL both be able to receive
mouse interaction. An active overlay SHALL prevent mouse mutation of the surface
beneath it. The global `AppLayout` completed-frame hit map and duplicated
mouse-coordinate paths SHALL be removed on completion.

#### Scenario: Queue and Library both take mouse while visible

- **WHEN** both Queue and a Library destination are visible and the user clicks
  within each region
- **THEN** each click is handled by the component that owns that region
- **AND** neither click is gated by the other's destination tag

#### Scenario: An overlay blocks underlying mouse mutation

- **WHEN** an overlay is active and the user clicks within the underlying surface
- **THEN** the click does not mutate the underlying surface

#### Scenario: Hit geometry cannot drift from painting

- **WHEN** a component's layout changes (for example variable-height playlist
  rows)
- **THEN** its hit targets change with its painting from the same computation
- **AND** no second, separately maintained geometry calculation exists for it

### Requirement: Complete conversion with no mixed-framework endpoint

The migration MAY use internal checkpoints, behaviour-preserving commits, and
temporary adapters, but a mixed TuiRealm/legacy framework SHALL NOT be a
completed or mergeable endpoint. Completion requires that every row in
`docs/architecture/interactive-surface-ledger.md` is `migrated`; every
independently interactive surface is a TuiRealm `AppComponent`; component-local
state, handlers, and render adapters are removed from `App` rather than mirrored;
`CONTEXT_STACK` interaction dispatch, `AppLayout`, and duplicated mouse paths are
removed; all temporary adapters and state mirrors are removed; and no parallel
legacy interaction framework remains.

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
- **THEN** existing input precedence, responsive behaviour, images-disabled
  behaviour, and render characterization coverage remain satisfied

### Requirement: Interactive ownership is mechanically enforced

The repository SHALL include path-scoped `ast-grep` rules under
`rules/interactive-component-boundary/` (registered in `sgconfig.yml`, matching
`src/app/components/**/*.rs`) that reject, in Interactive Component modules,
`impl App`, importing or using `App` as a type, direct Service-client or
`PlayerProxy` dependencies, and direct `mpsc` channel ownership. A CI job named
`interactive-component-boundary` SHALL run these rules on push and pull request.
Each rule SHALL ship one accepted and one rejected fixture. Static checks are a
ratchet, not proof; state-authority and duplicated-geometry questions remain the
reviewer's responsibility.

#### Scenario: A component module reaches for the shell

- **WHEN** a file under `src/app/components/` adds `impl App`, uses `App` as a
  type, holds an `mpsc` channel, or depends on a Service client or `PlayerProxy`
- **THEN** the `interactive-component-boundary` check fails

#### Scenario: The boundary check runs in CI

- **WHEN** a pull request or push is opened
- **THEN** the `interactive-component-boundary` job installs the pinned `ast-grep`
  version and runs the boundary rules

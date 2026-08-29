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

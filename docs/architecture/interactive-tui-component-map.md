# TuiRealm Migration Architecture Map

This document maps mbv's current interactive architecture, the accepted TuiRealm
target, the constraints inherited from existing decisions, and the integration
questions that must be resolved before implementation. Issue #603 is the umbrella
tracker. This map is architecture discovery, not an implementation plan.

## Status

- Current architecture: verified from `main` by static inspection on
  2026-08-23.
- Target architecture and migration policy: accepted in ADR 0022.
- Framework target: TuiRealm 4.1 using Ratatui 0.30 and Crossterm 0.29.
- Authoritative interactive inventory:
  `docs/architecture/interactive-surface-ledger.md`.
- Phase 0 architecture map and ledger: reviewed 2026-08-23.
- No implementation is authorized until the TuiRealm input, runtime-event,
  hierarchy, and geometry integration contracts are accepted.
- Existing visual design-system rules remain authoritative unless a later
  accepted spec or ADR explicitly supersedes them.

## External Reference

TuiRealm is the selected application framework:

- TuiRealm: <https://github.com/veeso/tui-realm>

A TuiRealm `AppComponent` co-locates component state, event handling, updates, and
rendering. mbv calls that unit an `Interactive Component` rooted at
`src/app/components/`; the narrower painter is a `Render Component` rooted at
`src/app/render/components/`. Bare `component` is not canonical
(`CONTEXT.md:379-392`).

## Current Architecture

### Launch and process ownership

`main` selects the launch arrangement before constructing `App`:

```text
main
|-- explicit or configured daemon endpoint -> RemotePlayer -> App::run
|-- detected Local daemon                  -> RemotePlayer -> App::run
|-- stay-alive without a daemon            -> spawn Local daemon -> attach
`-- bare mode                              -> local Player -> App::run
```

See `src/main.rs:270-374` and `src/app/construct.rs:274-519`.

The selected Player owner remains authoritative for playback and its Bound
queue. A Client may hold a Composed queue, presentation cursor, and projections,
but may not become playback authority. See ADR 0001, ADR 0017, ADR 0019, and
`CONTEXT.md:95-137,200-246`.

### The global App shell

`App` currently combines process integration, domain projections, interaction
state, rendering resources, and timing (`src/app/app_struct.rs:36-414`).

| Cluster | Current responsibilities |
| --- | --- |
| Service/runtime | Config, Emby and Audiobookshelf runtimes, setup forms, startup and catalog receivers |
| Player/transport | `PlayerProxy`, player/websocket/socket receivers, MPRIS, local/remote endpoint state |
| Browse/navigation | Home, Emby libraries, Audiobookshelf browse state, active destination, Panel focus and Panel mode |
| Queue/reconciliation | Local and remote projections, undo, queue-scope decisions, active-slot prediction, remote reconciliation |
| Interaction | Cursors, scroll, searches, overlays, sidebars, forms, context menu, modal state |
| Render/input bridge | Terminal size, image resources, last completed `AppLayout`, mouse and click timing |
| Timed/persistent work | Toast expiry, search/settings/library debounce, polling, keepalive, queue persistence |

The accepted architecture does not move every `App` field into a UI component.
It separates presentation-owned state from shell, Service, and playback
authority.

### Run loop and event flow

`App::run` deliberately draws the first frame before starting configured Remote
Services (`src/app/mod.rs:319-355`; ADR 0018).

```text
first frame
  -> start Service and feed workers
  -> loop
       -> drain worker and owner completions
       -> run periodic work
       -> poll one terminal event
       -> mutate App through handlers
       -> flush deferred persistence
       -> render on event, force, or cadence
  -> bounded teardown
```

The loop drains startup, player, library, Search, session, cast, shared-data,
feed, image, websocket, and Audiobookshelf-socket work
(`src/app/mod.rs:412-517`). Terminal events are handled at
`src/app/mod.rs:581-623`. Rendering is scheduled by
`src/app/render_cadence.rs:28-59` and `src/app/mod.rs:632-657`.

Async work generally follows this shape:

```text
input or action
  -> spawn thread with an owned client/snapshot and Sender
  -> run loop drains typed receiver
  -> completion handler validates identity/generation where available
  -> direct App reconciliation
  -> next render
```

Service setup generations, queue revisions, session generations, and image keys
are existing stale-completion guards. A component architecture must not weaken
them.

### Input precedence

ADR 0002 establishes one first-match context-priority authority. `App::handle_key`
iterates `CONTEXT_STACK`; the first handler returning `Some` claims the key
(`src/app/input.rs:76-83`, `src/app/input_resolver.rs:151-248`).

The effective order starts with blocking overlays, then sidebars and global
overlays, then Search and playback contexts, then destination dispatch:

```text
context menu
-> selection/daemon/confirm/re-anchor/save modals
-> settings/help/sessions/playlists
-> global overlay opening and panel controls
-> Search sidebar and inline library Search
-> playback/global controls
-> focused Queue or Library destination
```

`Command` is only a partial semantic seam. Playback, help, queue activation,
album-track actions, and Panel mode use it; most browse and modal handlers still
mutate `App` directly (`src/app/action.rs:23-102,385-649`). Mouse handling mixes
shared `Command` dispatch with direct state mutation
(`src/app/input_mouse_dispatch.rs`).

The accepted hierarchy preserves central precedence. A parent routes the raw
event; the selected child interprets it as a local message. Components do not
compete through an event broadcast.

### Rendering

The completed design-system migration is intentionally render-only. Its durable
boundary is:

```text
screens -> arrangements -> render components -> Ratatui
```

- Screens derive semantic content and approved variants from app state.
- Arrangements own placement, rectangle splitting, and breakpoints.
- Render components own painting and styling.
- Theme exposes semantic roles; raw primitives remain private.

See `openspec/specs/ui-design-system/spec.md:14-28`, `CONTEXT.md:379-407`,
and the archived change at
`openspec/changes/archive/2026-08-23-enforce-mbv-ui-design-system/`.

Many modules physically under `render/components` remain `impl App` methods.
The design-system migration did not claim interactive state, input, update, or
effect ownership. That is the gap tracked by #603.

`App::render` is not pure. It updates terminal and image state, expires toasts,
normalizes presentation state, builds a fresh `AppLayout`, renders overlays, and
atomically installs the completed layout (`src/app/render/screens/root.rs:25-184`).

### Geometry and hit targets

Mouse input reads the last completed `AppLayout`. The render pass builds a fresh
layout and installs it only after a complete frame (`src/app/layout.rs:1-19`).
Browse targets are protected by a destination tag; Queue targets intentionally
bypass that browse gate (`src/app/input_mouse.rs:15-48,134-186`).

The durable `ui-design-system` spec explicitly leaves hit-target ownership with
the existing app-layout/input system and prohibits partial migration to
component-published hit maps (`openspec/specs/ui-design-system/spec.md:21-38`).
The archived design records thirteen heterogeneous mechanisms and rejected a
generic partial migration
(`openspec/changes/archive/2026-08-23-enforce-mbv-ui-design-system/design.md:84-205`).

This is a current constraint, not an implicitly permanent outcome. Moving hit
geometry into interactive components would require a separate evidence-based
decision that supersedes the existing contract. Search can prove component
ownership without settling generic mouse geometry; the second proof cannot begin
until this question is resolved.

Known geometry debt includes Playlist row wrapping calculated separately by its
renderer and mouse handler (`src/app/input_mouse_panels.rs:138-256`).

## Authority Boundaries

### Must remain outside interactive components

| Authority | Owner |
| --- | --- |
| Provider APIs, config, queue identity/mutation, player runtime, ctrl/shared protocols | `mbv-core` |
| Audio device, Bound queue, source resolution, provider playback lifecycle | Player owner |
| Launch arrangement, Service startup, worker/channel lifecycle, stale completion checks | Application shell |
| Persistence and shared-data reconciliation | Application shell and `mbv-core` |
| Terminal installation, polling, render cadence, teardown | Application shell |

Interactive components must not receive `App`, Service clients, credentials,
`Config`, `PlayerProxy`, `RemotePlayer`, channels, protocol objects,
`Arc<Mutex<_>>`, source URLs/headers, or arbitrary Ratatui `Color`/`Style`.

No daemon, Local-daemon, `mbvd`, ctrl, shared-data, Service-startup, or playback
change belongs in this migration. The repository rule against speculative daemon
fixes remains absolute.

### Component-safe inputs

Components may receive owned presentation models containing text, durations,
badges, semantic focus/selection/disabled state, image cache keys, semantic
variant/policy values, and stable opaque action keys.

Queue display models may carry `QueueSlotId` as an opaque key, but the shell or
Player owner resolves and mutates it. A reduced or cloned playback-status
projection is acceptable; its lock and `PlayerProxy` are not.

### Accepted crossings

```text
terminal event
  -> TuiRealm active AppComponent
  -> private state update
  -> optional Msg
  -> TuiRealm Model update
  -> focus/mount change or shell policy
  -> existing Service/player/persistence effect
  -> UserEvent completion
  -> subscribed AppComponent
```

TuiRealm supplies the registry, focus, subscriptions, event delivery, messages, and
render entry point. A second component trait, dispatcher, focus framework, generic
effect scheduler, or Flux store is outside the target.

## Accepted Interactive Hierarchy

```text
Application shell
`-- UiRoot
    |-- Queue
    |-- Library
    |   |-- Home
    |   |-- Emby browser instances
    |   |   |-- generic/Movies/home-video
    |   |   |-- TV workspace
    |   |   `-- grouped Music workspace
    |   |-- Audiobookshelf podcast browser instances
    |   |-- Audiobookshelf book browser instances
    |   `-- Feeds
    `-- Overlay stack
        |-- Search
        |-- Settings and setup forms
        |-- Sessions
        |-- Playlists and save dialog
        |-- Help
        |-- Context menu
        |-- Selection modal
        `-- remaining blocking popups/modals
```

The boundary is an independently routed state machine, not every visual unit.
Rows, pills, heroes, modal frames, and cards remain render components. Library is
a parent with destination children rather than one new monolith.

Parents own child presence and focus. Children do not mutate parents or siblings;
they return typed outputs. A parent may handle a local output such as dismissal or
convert a cross-boundary output into shell work.

## Interactive Surface Inventory

All rows are `legacy` until an implementation change proves otherwise.

| Interactive surface | Current ownership and notable constraints | Relative migration risk |
| --- | --- | --- |
| Root UI and overlay routing | `App`, `render/screens/root.rs`, `CONTEXT_STACK`; owns focus and stacking | High |
| Queue | Cursor/scroll/scope and input on `App`; canonical queue remains outside | High |
| Library parent | Active destination, Panel focus/mode, child routing | High |
| Home | Cross-Service rows and hero presentation | Medium |
| Emby generic/Movies/home-video | Shared list and hero paths | Medium |
| TV | Two focusable panes, season/episode targets | High |
| Grouped Music | Album/track focus coupling and track targets | High |
| Audiobookshelf podcasts | Show/episode workspace and selector targets | High |
| Audiobookshelf books | Browser/chapter workspace and replacement geometry | High |
| Feeds | Grouping, selector, list, and inline hero | Medium |
| Global Search sidebar | Keyboard, debounce, async result, viewport; ordinary conversion row | Medium |
| Inline library Search | `LibSearch` inside one Emby browser; distinct conversion row | Medium |
| Settings | Destinations, forms, Service setup, nested popups | High |
| Sessions | Merged Emby/Cast targets and fixed-stride mouse geometry | Medium |
| Playlists | Variable row geometry duplicated in mouse path | High |
| Help | Local scroll and destination-derived content | Low |
| Context menu | Exclusive priority and anchor placement | Medium |
| Selection modal | Filters, source-specific behavior, explicit row targets | Medium |
| Remaining modals/popups | Mostly local cursor/form state over shared modal frame | Low to medium |

The complete interactive inventory, including each popup and modal, is
`docs/architecture/interactive-surface-ledger.md`. Existing `TestBackend`
characterization covers the render surfaces listed in the archived visual ledger.
That coverage is a regression asset, not proof of interactive ownership. Many tests
still construct `App`.

## Search: Rejected First Proof

The global Search sidebar is not the first proof and no Search component migration is
authorized. Inline library Search remains a separate legacy child of the Emby
browser.

### Current ownership map

| Concern | Current location |
| --- | --- |
| Query, results, cursor, scroll, loading, filter, error, renderer-written height | `src/app/search_sidebar.rs` |
| Presence | `App.search_sidebar` in `src/app/app_struct.rs:259` |
| Key interpretation and local mutation | `src/app/input_search_sidebar_keys.rs` |
| Open/dismiss | `src/app/library_search_actions.rs:9-19` |
| Debounce pending/deadline | `App` fields at `src/app/app_struct.rs:257-258` |
| Debounce expiry and request start | `src/app/run_loop_drains.rs:207-226` |
| Worker and result channels | `src/app/mod.rs:672-686` and construction fields |
| Rendering and viewport mutation | `src/app/render/components/search_sidebar.rs:28-175` |
| Priority | Search entry in `src/app/input_resolver.rs:201-203` |
| Activation/navigation | `src/app/input_search_sidebar_keys.rs:120-137` and `LibEvent::NavigateTo` |

### Current behavior contract

- Both terminal encodings of Ctrl+/ open global Search from Home or any Library
  destination without changing the underlying destination or Panel focus. Pressing
  it while Search is open leaves the query unchanged.
- Search occupies the standard side-panel slot, uses its frame/title/hints, never
  dims the backdrop, and falls back to the fixed-width left-edge panel when the
  normal slot has zero width.
- Query, type-filter chips, and results render in that order. Results are exactly
  one truncated row with a type badge; no result hero, metadata row, overview,
  image, wrapping, or image fetch is allowed.
- Search has a fixed position in central input precedence. It swallows bound and
  unbound keys, never quits the app, and leaves all underlying navigation and
  selection state unchanged.
- Query changes reset local result state. Requests are non-blocking, debounced at
  300 ms, and only sent at two or more characters; loading/error and out-of-order
  response semantics remain those in `global-search-sidebar`.
- Chips contain All plus exactly the types in the current result set, cycle in both
  directions with wraparound, filter results, and reset filter/cursor when changed
  or when new results arrive.
- Unsupported or unresolvable item types are excluded. Empty/all-excluded results
  show the existing empty state. Activation with no result is a no-op that leaves
  Search open.
- Activation resolves the item's Library, navigates through the existing path, and
  closes Search. Dismissal and backspace on an empty query close without navigating;
  reopening starts with empty query/results/filter.
- Normal, narrow, one-row, zero-width-slot, and images-disabled rendering remain
  supported.

The full normative contract is
`openspec/specs/global-search-sidebar/spec.md:6-190`; the list above is a handoff
index, not a replacement spec.

Existing behavior risks discovered during mapping:

- Dismissal does not clear an armed debounce; the request may still run and its
  result is dropped because Search is absent.
- Query text is the only stale-response identity, so repeated equal queries cannot
  distinguish request generations.
- A sub-two-character query sets `loading` but schedules no request, in tension
  with `openspec/specs/global-search-sidebar/spec.md:51-62`.
- Late navigation completion has no Search intent generation and may apply after
  user intent changes.

These are existing risks, not authorized behavior changes in an architecture-only
migration. Each needs an explicit decision or separate bug scope.

### Rejected proof boundary

This boundary was explored but is not an implementation authorization. Defining it
before the common component contract would let Search establish another bespoke
interface that later surfaces might not share.

Inside `src/app/components/search.rs`, the local `Message` enum is private and the
shell-facing `Output` enum is public. `Output` contains only requests that cross
Search's authority boundary; there is no global UI message/effect enum and names are
not redundantly prefixed with `Search` inside the Search module.

Search owns the 300 ms debounce policy and its pending deadline. The shell owns the
clock: it supplies `Instant`, consults `next_deadline()`, and calls `on_time(now)`.
Only Search decides whether the deadline is current and emits `Output::Search` when
due; no generic scheduling output or shell-owned Search deadline is introduced.

The App-free and behavior requirements below remain useful evidence for any later
Search migration, but they do not define the common standard.

Search-specific timer, request identity, viewport, and output questions are deferred.
Do not create a Search OpenSpec until the common component contract is accepted.

### Required render seam

The current Search renderer cannot satisfy the App-free proof by moving files alone.
It is an `impl App` and calls App-owned panel primitives in
`src/app/render/components/chrome.rs`: `render_panel_shell`,
`render_panel_shell_at`, `render_sidebar_scrollbar`, `panel_row_text_width`, and
`render_panel_row`.

The Search change must first expose the required existing panel/row/scrollbar
painting through ordinary typed render-component functions, preserving output. It
must reuse those functions rather than duplicate panel geometry or pass `App` as a
context. This is a render-boundary extraction, not a visual redesign and not a
generic interactive-component framework.

The existing App-based Search buffer characterization must protect the extraction.
The final Search tests must render the interactive component directly without App.

### Verification contract

The Search change must use these test locations:

- `src/app/search_sidebar_tests.rs` for local message, update, output, debounce,
  stale-response, filter, and cursor behavior;
- `src/app/render/tests_search_sidebar.rs` for direct App-free `TestBackend`
  rendering, replacing the current App-based helper;
- `src/app/input_search_sidebar_keys_tests.rs` for shell priority, terminal-key
  encodings, underlying-state preservation, and output routing.

Test names in those files must contain `search_sidebar`, so the narrow verification
command is `rtk cargo nextest run -p mbv search_sidebar`. Together they must cover:

- both Ctrl+/ encodings, opening from Home/Library, repeat-open, precedence,
  swallowing, and unchanged underlying state;
- panel-slot, zero-width fallback, no dimming, normal/narrow/one-row and
  images-disabled buffers;
- single-row badges, truncation, selected state, no hero/image fetch;
- debounce threshold and timing, loading/error, current and out-of-order results;
- chip inventory, forward/backward wrap, filtering and reset;
- unsupported/unresolvable results, empty activation, navigation and dismissal;
- direct component construction/update/render without App;
- shell routing of request, activation and dismissal outputs.

Prefer extending durable existing tests; do not create one test per message branch.

## Complete Conversion Gate

The ledger at `docs/architecture/interactive-surface-ledger.md` is stable and
non-archived. It has one row per independently interactive surface, not per painter
or file. Row states may expose internal conversion progress, but a mixed
TuiRealm/legacy framework is not a completed or mergeable endpoint.

- New interactive surfaces conform from creation.
- Existing surfaces migrate within one complete-conversion OpenSpec using explicit,
  behavior-preserving checkpoints.
- Narrow fixes and shared visual changes may touch legacy surfaces without forcing
  a full migration.
- A legacy change may not add a new surface-specific `App` state cluster, new
  `impl App` interaction subsystem, or duplicated geometry without an explicit
  exception.
- A converted surface may not regain `App`-owned local state, input, or rendering.
- A migration removes old state; it does not synchronize mirrors.
- Each migration preserves existing input precedence, responsive behavior, images-
  disabled behavior, and characterization coverage.

Completion additionally requires every interactive surface to implement TuiRealm's
`AppComponent`, with `CONTEXT_STACK`, `AppLayout`, temporary state mirrors, and the
legacy interaction framework removed.

## Enforcement

Compiler enforcement should use TuiRealm's `AppComponent` contract, private state,
and narrow typed APIs. Interactive Component paths must not accept `&mut App`.

Path-scoped ast-grep rules should reject, for Interactive Component modules:

- `impl App`;
- importing or using `App` as a type;
- direct Service-client or `PlayerProxy` dependencies;
- direct `mpsc` channel ownership.

Provider lifecycle, protocol, persistence, and semantic-theme authority are review
and compiler checks until a precise, low-false-positive static predicate exists.
Do not add broad identifier or import bans to claim coverage for them.

The existing frontend ast-grep rules only guard `render/screens` and do not enforce
interactive ownership. They are not sufficient for this migration. Static checks
are a ratchet, not proof; review still owns state-authority and duplicated-geometry
questions.

Rule files have the fixed location `rules/interactive-component-boundary/*.yml` and
that directory must be added to `sgconfig.yml`. Their `files` matcher is
`src/app/components/**/*.rs`. The required local command is `rtk ast-grep scan`;
rule fixtures must demonstrate one accepted and one rejected case per rule.

The complete-conversion change must add `.github/workflows/architecture-boundaries.yml`,
with a PR/push job named `interactive-component-boundary` that installs the pinned
ast-grep CLI version `0.44.1` and runs `ast-grep scan`.

Any exception to the no-new-debt or dependency rules requires maintainer approval
recorded in issue #603 and linked from the affected ledger row. Widening an ignore
glob or adding an inline suppression is not an exception process.

Tests should prefer the component boundary: local update/output tests, direct
`TestBackend` render tests, and a small shell-routing integration test. They should
not duplicate every message branch or replace durable behavior tests.

## Existing Decisions That Remain Binding

- ADR 0001: canonical queue authority remains in `mbv-core`/Player owner.
- ADR 0002: central input context precedence remains authoritative.
- ADR 0013: one Power View; images-disabled rendering is first-class.
- ADR 0014 and ADR 0015: multi-client and Local-daemon lifecycle semantics remain.
- ADR 0017 and ADR 0019: Composed/Bound queue and Player-owner resolution remain.
- ADR 0018: the first TUI frame precedes Remote Service startup; no universal
  provider abstraction is introduced.
- ADR 0021: hero placement and interaction invariants remain.
- `ui-design-system`: render ownership and current hit-target contract remain until
  explicitly superseded.

## Decision Inventory

### Verified current facts

- `App` is the global mutable integration and interaction shell.
- The current design-system architecture is render-only.
- Input priority is centralized, while local meanings and mutations remain mixed.
- `AppLayout` is the current completed-frame geometry authority.
- Search crosses state, input, update, async, navigation, and rendering boundaries.

### Accepted by ADR 0022

- TuiRealm as the application framework for every interactive surface.
- TuiRealm `AppComponent` as the Interactive Component contract.
- Parent-owned presence/focus and child-local messages.
- TuiRealm `Msg` outputs for parent/shell work and `UserEvent` completions.
- Vertical surface modules, split only when size or cohesion requires it.
- Shell, Service, playback, queue, worker, and persistence authority remain outside.
- One complete conversion with no mirrored-state or mixed-framework endpoint.
- No parallel custom component, dispatcher, focus, effect, or Flux framework.

### Unresolved before implementation

1. Exact `ComponentId`, `Msg`, and `UserEvent` shapes.
2. Parent/child visibility, mount, render-order, and focus calculations.
3. ADR 0002 key precedence using TuiRealm focus and subscriptions.
4. Simultaneously visible mouse targets and overlay blocking.
5. Component-owned geometry replacing `AppLayout`.
6. Runtime receiver mapping to TuiRealm ports or minimal shell adapters.
7. Rust 1.88/MSRV adoption for TuiRealm 4.1.
8. Enforcement and tests that reject a parallel legacy framework.

No item in this unresolved list may be silently decided by an implementation agent.

## Known Repository Inconsistencies

1. `audit-hero-on-left-arrangement` declares deleted
   `unify-selected-row-background` as a prerequisite, while #601 is closed as not
   planned. That active change must be revised before it can proceed; it does not
   block architecture mapping or Search.
2. ADR 0021 still says complete component work belongs to #563, but #563 delivered
   the render-only design system. ADR 0021 is corrected alongside this map to point
   at #603.
3. `AGENTS.md` and the frontend skill previously referred to an active visual
   migration ledger that is now archived; they now point to the stable interactive
   ledger while preserving the existing render boundary.
4. `CONTEXT.md` and ADR 0018 describe packaged `mbvd` as Emby-gated, while current
   source appears to construct optional owner contexts. This documentation/source
   drift must be investigated separately and is not a TUI migration task.
5. Current frontend static scanning reports existing violations concentrated in
   root, Queue, and pills screen modules. Do not present a clean scan as current
   fact or widen ignores to hide them.

## Work After Architecture Review

1. Resolve the TuiRealm integration questions above.
2. Create one complete-conversion OpenSpec with internal checkpoints.
3. Convert the shell, root hierarchy, every ledger surface, overlays, and geometry.
4. Remove temporary adapters, `CONTEXT_STACK`, `AppLayout`, and component-local
   `App` state.
5. Verify the full behavior and architecture gate before declaring completion.

The umbrella issue coordinates this work. Internal checkpoints may remain
reviewable, but they do not authorize a permanently mixed framework.

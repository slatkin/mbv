## Context

See `proposal.md` (Why) for motivation and ADR 0022 for the accepted decision.
The current architecture, the 29-row interactive inventory, and the eight
unresolved integration questions are recorded in
`docs/architecture/interactive-tui-component-map.md` and
`docs/architecture/interactive-surface-ledger.md`. This document resolves those
eight questions so implementation can begin; it does not restate the map.

Load-bearing facts verified for this design:

- `tuirealm 4.1.0` (latest stable, May 2026) requires `ratatui ^0.30` and
  `crossterm ^0.29`; mbv already pins `ratatui 0.30.2` and `crossterm 0.29.0`.
  The existing render substrate (screens → arrangements → render components →
  Ratatui) needs no version churn.
- tuirealm's default features already include `crossterm` and `derive` (the
  `tuirealm_derive` macros), so `tuirealm = "4.1"` needs no extra feature flags.
- tuirealm's MSRV is `1.88`; mbv currently declares no `rust-version`.
- Core API, read from source (4.0 renamed the traits): `Component`
  (`view`/`query`/`attr`/`state`/`perform` — the old `MockComponent`) and
  `AppComponent<Msg, UserEvent>: Component` with
  `on(&mut self, &Event<UserEvent>) -> Option<Msg>` (the old `Component`).
  `Application<ComponentId, Msg, UserEvent>` bounds `ComponentId: Eq+Hash+Clone`,
  `Msg: PartialEq`, `UserEvent: Eq+PartialEq+Clone+Send+'static`; built via
  `Application::init(EventListenerCfg)`; methods `mount`/`umount`/`active`/`blur`/
  `focus`/`attr`/`query`/`state`/`get_component`/`get_component_mut`/`subscribe`/
  `unsubscribe`/`restart_listener`/`tick`.
  `tick(strategy) -> Vec<Msg>`, folded through `Update::update`. There is **no**
  API to deliver an event to a chosen component. `Event<UserEvent>` =
  `Keyboard`/`Mouse(MouseEvent{column,row,kind,..})`/`WindowResize`/`FocusGained`/
  `FocusLost`/`Paste`/`Tick`/`None`/`User`. Subscriptions: `Sub(EventClause,
  SubClause)`; `EventClause` = `Any`/`Keyboard(KeyEvent)`/`WindowResize`/`Tick`/
  `User`/`Discriminant`/`Mouse` — but the `Mouse` clause only range-checks
  `column`/`row` and **ignores `kind`/`modifiers`**, so it is not a usable wildcard
  mouse subscription; `SubClause` = `Always`/`IsMounted`/
  `HasState`/`HasAttrValue`/`Not`/`And`/`Or`/`AndMany`/`OrMany`. Custom events via
  `EventListenerCfg::port(Box<dyn Poll<UserEvent>>, interval)`; `PollStrategy` =
  `Once(Duration)`/`TryFor`/`UpTo`/`BlockCollectUpTo` (behaviour-bearing: batched
  strategies collect all active-component messages before subscription messages,
  changing observable input ordering).

  Delivery/bridge facts (read from 4.1.0 source this pass): `tick` sends each event
  to the **active** component first, then to matching **non-active** subscribers
  (the focused component is skipped in that traversal, so no double delivery). A
  component may hold only **one subscription per `EventClause`**, and 4.1.0's
  `unsubscribe` removes more broadly than its signature implies — prefer static
  composite guards plus mount/unmount over subscription churn. `get_component_mut`
  returns `&mut dyn AppComponent`, downcastable via `.as_any_mut().downcast_mut::<T>()`
  (the shell→component update bridge); `restart_listener(EventListenerCfg)` is the
  **only** runtime port mechanism (whole-listener replacement, no per-port
  add/remove). Native focus is a **LIFO stack**: `active` pushes the previous focus,
  `blur`/`umount`-of-focused pops and restores it. `Update::update(Option<Msg>) ->
  Option<Msg>` runs in a while-`Some` cascade, so a component may chain follow-on
  messages without a scheduler.

## Governing Principle

**There is no target design to invent. The target is the current app.** This
migration is behaviour- and appearance-preserving to cursor-grain: same layout,
same pills, same hero behaviour, same focus movement, same keys, same mouse
targets. Nothing the user sees or presses changes. The completion gate (ADR
0022) and the characterization-buffer tests that land before each surface
converts enforce exactly this.

The interactive hierarchy is **internal code ownership**, invisible to the user.
Whether a Browser's list, its hero child, and its pill bar are "one component
with three cursors" or "three components" changes zero pixels and zero
behaviours — it only changes which Rust struct owns which selected field.
Refining to cursor-grain is therefore **not a design change**: it names the
cursors that already exist today and assigns each an owner.

Consequence for the implementer: every question about a surface's cursors,
pills, panes, hero behaviour, or focus targets is a **fact about the current
source**, not a design decision. At each surface's conversion, read the existing
behaviour from the code that owns it today (the ledger row's "Primary current
ownership" column names the files) and reproduce it exactly in the component.
Do not invent, redesign, improve, or simplify a surface's interaction model.
If a behaviour seems wrong, it is either an existing bug tracked separately
(Search's correctness issues are explicitly out of scope) or a fact to preserve
— never a licence to change it inside this migration.

This is what makes the migration mechanical rather than inventive: the shapes
below (D3–D5) name the *containers*; the *contents* are copied from the current
app one surface at a time.

## Goals / Non-Goals

**Goals:**
- Resolve the eight integration questions with concrete, reviewable contracts:
  `ComponentId`/`Msg`/`UserEvent` shapes; hierarchy/mount/focus derivation; ADR
  0002 precedence via focus + subscriptions; simultaneous mouse targets + overlay
  blocking; component-owned geometry replacing `AppLayout`; runtime-receiver
  mapping to `Port`s/adapters; MSRV adoption; enforcement and test layering.
- Define a strangler-style checkpoint order in which the app runs on TuiRealm's
  loop from the first checkpoint and every intermediate commit is
  behaviour-preserving and reviewable.

**Non-Goals (design-level):**
- No *wholesale* `tui-realm-stdlib` painter rewrite; mbv's existing render
  substrate stays the default. The monorepo's sibling crates (`tui-realm-stdlib`
  `Input`, `tui-realm-textarea`, `tui-realm-treeview`) MAY be adopted per-surface
  where they genuinely cut code — most plausibly for text-entry surfaces (search
  boxes, save-name dialog, form fields) — decided per surface, not as a blanket.
  `treeview` is a poor fit for mbv's image/hero-rich hierarchies.
- No new provider abstraction, no async runtime for playback, no Flux layer.
- No fix to Search's known correctness bugs (tracked separately by the map).
- No line-by-line component-by-component API listing; per-surface shapes are
  derived from the four canonical shapes below during each checkpoint.

## Decisions

### D1 — Dependency and MSRV (resolves Q7)

Add `tuirealm = "4.1"` (its default features already include `crossterm` and
`derive`/`tuirealm_derive`) and declare `rust-version = "1.88"` in
`[workspace.package]`.
There is no alternative version: 4.1 is the only line that matches mbv's
ratatui/crossterm pins, and its MSRV is `1.88` — i.e. it needs Rust **≥ 1.88**, a
minimum floor, not an exact pin and with no upper bound. Declaring mbv's
`rust-version = "1.88"` records that floor; effective MSRV composes as the maximum
across all dependencies, so this may already be at or near mbv's existing floor.
It is a maintainer-visible toolchain commitment recorded as **BREAKING (build)**
in the proposal; the proposal deliberately adds no `rust-toolchain.toml` pin. Alternative considered: pin an older
tuirealm to avoid the MSRV — rejected, older lines require ratatui ≤0.29 and
would force a downgrade of the entire render substrate.

### D2 — Shell Model vs. TuiRealm Application

The application splits into a **shell Model** and the **TuiRealm Application**.
The Model owns everything in the `interactive-component-framework` "shell retains
runtime authority" requirement and holds the `Application` as a field. `App::run`
becomes the Model's loop: install terminal → draw first frame (ADR 0018 ordering
preserved) → start Service/feed workers → loop over `application.tick(...)`,
runtime `Port` drains, periodic work, and `application.view(...)`. The Model is
**not** a second global UI store: no component-local cursor/scroll/form state
lives on it.

Alternative considered: keep `App` as the Model verbatim — rejected, it would
preserve the global interaction store the migration exists to remove.

### D3 — `ComponentId` shape (resolves Q1, part 1)

`ComponentId` is a typed enum; flat registry addressing is derived from it. It
encodes stable dynamic identity so a component survives re-render and an inactive
destination keeps its private state:

```rust
enum ComponentId {
    UiRoot,
    Playback,
    Queue,
    Library,                       // parent, not a monolith
    Home,
    Browser(BrowserKey),           // { service: ServiceKey, library: LibraryId, kind: BrowserKind }
    Feeds,
    InlineSearch(BrowserKey),      // inline library Search, child of one Emby browser
    Overlay(OverlayId),            // Search, Settings, Sessions, Playlists, Help, ContextMenu, SelectionModal
    Modal(ModalId),                // Confirm, DaemonLost, RemoteReanchor, SavePlaylist
    Popup(PopupId),                // Multiselect, LibraryRoutes, FeedManage — Settings children
}
```

`BrowserKey` carries the Service-library key plus surface kind, so `Browser` and
`InlineSearch` instances are stable across renders and per library. Rows, heroes,
pills, and modal frames are Render Components and get **no** `ComponentId`.
Alternative considered: opaque `usize`/`String` ids — rejected, they lose the
compile-time distinctness the `coding-practices` newtype rule requires and make
parent/visibility functions stringly-typed.

### D4 — `Msg` shape (resolves Q1, part 2)

`Msg` is the single TuiRealm outbound type, grouping surface output enums. It
carries **only** cross-authority requests; local state changes never become a
`Msg` (they mutate the component in `on`/`update` and return `None`):

```rust
enum Msg {
    Navigate(NavTarget),
    Playback(PlaybackRequest),   // opaque QueueSlotId keys; shell/Player resolves
    Queue(QueueRequest),
    Service(ServiceRequest),     // browse fetch, search request, session/cast ops
    Persist(PersistRequest),
    Shell(ShellRequest),         // mount/dismiss overlay, change focus, toast
}
```

Names inside a surface's module are not redundantly prefixed with the surface
name. This is one TuiRealm message type, not a second dispatcher. Alternative
considered: a global effect enum with per-effect scheduler — rejected by ADR 0022
(recreates a framework).

### D5 — `UserEvent` shape and runtime-receiver mapping (resolves Q1 part 3, Q6)

TuiRealm requires `UserEvent: Eq + PartialEq + Clone + Send + 'static`. That is
satisfiable by data, but forcing `Eq` and cheap `Clone` onto mbv's completion
payloads (decoded images, large result sets, float-bearing status) is awkward or
impossible, and subscriptions match user events by value (`EventClause::User`) or
by variant (`EventClause::Discriminant`). Decision: `UserEvent` carries a **small
`Eq`/`Clone` token** identifying a completion (which also gives clean
`Discriminant` subscriptions); the owned presentation model is pushed **directly
into the mounted target** by the shell via
`get_component_mut(id)?.as_any_mut().downcast_mut::<T>()` after the shell validates
the completion. No shell-owned slot, channel, or lock is needed: the token
identifies *which* completion fired (for `Discriminant` routing and
stale-generation validation); the validated payload is delivered by direct
downcast. This is the "minimal shell adapter" the map anticipates.

```rust
enum UserEvent {
    Startup(StartupTick),
    LibraryReady(BrowserKey, Generation),
    SearchReady(SearchGen),
    Session(SessionGen),
    Cast(CastGen),
    SharedData(SharedRev),
    Feed(FeedKey, Generation),
    Image(ImageKey),
    Websocket(WsTick),
    AbsSocket(AbsTick),
    Clock(Instant),              // drives component-owned debounce (e.g. Search 300 ms)
}
```

Each existing run-loop receiver (`src/app/mod.rs:412-517`) is drained by a
**shell-owned adapter (the default)** that injects the matching `UserEvent`; a
receiver MAY instead be a `Poll<UserEvent>` port on `EventListenerCfg` only when its
lifetime is stable. Receivers mbv replaces at runtime (player, websocket, ABS
socket, setup) stay shell-owned, since `restart_listener` is the only runtime port
mechanism and it restarts the whole listener. The
**existing stale-completion guards** (Service setup generations,
queue revisions, session generations, image keys) remain in the shell: the token
carries the generation, the shell validates it, then writes the validated model
into the target component via `get_component_mut`+downcast. The component only
ever holds validated data. No component receives a channel, client, or lock. Clock delivery replaces the manual debounce-deadline
drains: the shell supplies `Instant` and the owning component decides whether its
deadline is due (per the Search debounce contract in the map).

Alternative considered: widen `UserEvent` to hold owned models — rejected, it
forces `Eq`/cheap-`Clone` onto image and result payloads and entangles them with
`EventClause::User` value-matching; the token keeps event identity cheap.

### D6 — Hierarchy: visibility, mount, render order, focus (resolves Q2)

The flat registry is projected onto the accepted hierarchy by **pure functions
over typed data**, not a second tree structure:

- **Mount policy:** `UiRoot`, `Playback`, `Queue`, `Library`, and `Home` are
  mounted for the session. `Browser`/`InlineSearch`/`Feeds` destination
  components stay mounted while their Service library exists so their cursor and
  scroll persist; a component for a removed Service library is unmounted.
  Overlays, modals, and popups mount on open and unmount on dismiss.
- **Visibility/render order:** the shell builds a per-frame `render_plan` by
  **querying the components** (TuiRealm `query`/`get_component`) for the parent's
  selected child and the overlay z-order — never by mirroring that UI state on the
  Model (the boundary forbids it). The plan lists the active destination under
  `Library`, then the overlay stack bottom to top; `view()` paints in that order and
  the top blocking overlay paints last.
- **Focus:** use TuiRealm's **native LIFO focus stack** (matching the
  `interactive-component-framework` spec, which already assigns the focus stack to
  TuiRealm). `active(id)` pushes the previous focus; opening an overlay calls
  `active`, and dismissing it `umount`s the overlay, which auto-`blur`s and pops the
  stack — deterministic restoration with **no shell-owned focus stack**. mbv keeps
  only overlay/parent **z-order** (a render-ordering concern, separate from focus)
  in the owning component. Parents own child presence and the selected child; children return
  typed `Msg`s and never mutate a parent or sibling.

Alternative considered: TuiRealm's render-only `Container` as the hierarchy —
rejected, `Container` broadcasts commands to `dyn Component` children and is not
an independently-routed interactive tree.

### D7 — Input precedence via focus + subscriptions (resolves Q3)

ADR 0002 is current and its resolver model is the correct target to preserve, not
legacy to route around. That model — a **context-priority stack** in which each
active context resolves a key to one of `Command` / `Swallow` / `FallThrough`, and
the first context returning `Command` or `Swallow` claims it — is preserved as the
*semantics*. TuiRealm replaces only the `CONTEXT_STACK` **loop mechanism**, not the
three-outcome model.

1. A single small `key_policy` module holds the priority-ordered context table
   and, per context, its resolution of a key to `Command` (→ `Msg`) / `Swallow` /
   `FallThrough`. It is the one source of precedence — the direct successor to
   `CONTEXT_STACK`'s ordering, minus the per-surface handler bodies.
2. **TuiRealm has no native precedence, consume, or first-match.** `tick()`
   forwards each event to the *active* component **and** to every subscriber whose
   `EventClause`+`SubClause` match, returning all resulting `Msg`s in a `Vec`
   (active's first, then subscriptions). ADR 0002's ordering, `Swallow`, and
   `FallThrough` are therefore **not** framework primitives — they must be
   *engineered* entirely from (a) which component is active and (b) how
   subscriptions are gated.
3. Global/parent bindings are `Sub::new(EventClause::Keyboard(chord), guard)` on
   the component that owns each binding, with `guard` generated from the
   `key_policy` table so the eligible set is **mutually exclusive**:
   - `Command`: exactly one recipient's clauses match → one `Msg`.
   - `Swallow`: the active blocking overlay is the only recipient, and every global
     binding is guarded `Not(IsMounted(overlay))` so no subscription fires — the
     key dies with the overlay even when the overlay emits no `Msg`.
   - `FallThrough`: the focused leaf returns `None` from `on()` **and** a global
     binding's guard is satisfied, so that binding is the lone eligible subscriber
     and handles it in the same tick.
   Because the active component is always a recipient, a leaf must return `None`
   for any key it should not claim; correctness rests on the guards being exclusive,
   which is why they are all generated from the one table and proven by tests.

The executable contract is the phase-2 (#131) input characterization suite, which
locks the exact order and the Swallow/FallThrough outcomes; the six map proofs
(see D10) extend it: blocking overlays swallow input; parent/global precedence
preserved; Queue and Library both take mouse while visible; overlay blocks
underlying mouse mutation; focus restoration deterministic; hit geometry cannot
drift.

Alternative considered: keep the `CONTEXT_STACK` loop as a pre-TuiRealm router —
rejected, the loop is the mechanism ADR 0022 removes; the model it encodes is
exactly what we keep.

### D8 — Mouse: simultaneous targets, overlay blocking, component geometry (resolves Q4, Q5)

Constraint from source: `Application` exposes **no** way to forward an event to a
chosen component. `EventClause::Mouse` *does* exist in 4.1.0 but only range-checks
`column`/`row` and ignores `kind`/`modifiers`, so it is not a usable wildcard mouse
subscription — the prudent choice remains `EventClause::Any`. `tick()` sends
`Event::Mouse(column,row,..)` to the active component plus `Any`-clause
subscribers. A shell-side hit-router that dispatches a mouse event to the region
under the cursor is therefore **not wireable**. The realization is instead
subscription-based:

- Each currently visible top-level region (Queue, the active Library destination,
  an overlay) subscribes with `Sub::new(EventClause::Any, guard)`. On a mouse event
  every such subscriber's `on()` runs, but each returns a `Msg` **only if
  `(column,row)` falls in the geometry it painted**; all others return `None`. This
  yields simultaneous Queue+Library mouse targets with **no destination-tag gate**.
- Geometry is component-owned: a component records the `Rect`s/rows it paints
  **during `view()`** in its private state and hit-tests against exactly that, so
  painting and hit-testing cannot drift (fixing the Playlist row-wrapping
  duplication).
- Overlay blocking: while a blocking overlay is up, the region subscriptions carry
  `Not(IsMounted(overlay))` guards, so underlying regions receive no mouse event
  and cannot mutate; the active overlay handles it.

The global mouse router, completed-frame hit map, and duplicated mouse-coordinate
paths are removed for alpha. Supported paths read component-owned geometry;
Music, blocking-modal, and playback-prompt mouse interaction is deferred by D16.
Render-only layout state may remain and is not mouse-routing authority. This
supersedes the `ui-design-system` hit-target clause (see the modified spec).

Accepted trade-off: `EventClause::Any` means each visible region sees *every* event
(keyboard, tick, mouse), not just mouse. This is a **bounded, guarded broadcast** —
only a handful of visible regions, each cheaply returning `None` for events it does
not own, so no state-changing event reaches a non-owner. It is the closest
tui-realm-native fit to the map's "do not broadcast state-changing events to every
component," and its exact wiring is proven in the foundation phase and first
conversions before flood-fill.

Alternative considered: a shell hit-router forwarding to the chosen component —
rejected, `Application` has no per-component event delivery. Alternative
considered: a per-frame component→global hit map — rejected, it recreates
`AppLayout`.

### D9 — Rendering seam

A component's `view(frame, area)` calls the **existing** render substrate: the
parent arrangement supplies the child's outer `Rect`; the child owns internal
arrangement, responsive behaviour, viewport, and painting via existing render
components. TuiRealm adoption does not replace painters. The documented Search
render-seam extraction (expose `render_panel_shell*`, `render_sidebar_scrollbar`,
`panel_row_text_width`, `render_panel_row` as typed functions rather than `impl
App`) is a prerequisite for the Search row and is a render-boundary extraction,
not a redesign.

### D10 — Enforcement and tests (resolves Q8)

- `ast-grep` rules in `rules/interactive-component-boundary/*.yml` (added to
  `sgconfig.yml`, `files: src/app/components/**/*.rs`) reject `impl App`, `App`
  as a type, Service-client/`PlayerProxy` deps, and `mpsc` ownership. Each rule
  ships one accepted + one rejected fixture; local gate is `rtk ast-grep scan`.
- `.github/workflows/architecture-boundaries.yml` adds job
  `interactive-component-boundary` installing pinned `ast-grep` `0.44.1` and
  running `ast-grep scan`.
- Compiler enforcement: Interactive Component APIs never accept `&mut App`.
- Test layering per component: local update/output tests; **App-free**
  `TestBackend` render tests (replacing App-based helpers); one small
  shell-routing integration test. Prefer extending durable characterization
  tests; do not write one test per message branch. Provider-lifecycle, protocol,
  persistence, and semantic-theme authority stay review + compiler checks (no
  broad import bans).

### D11 — Checkpoint strategy (strangler, behaviour-preserving)

The migration runs on TuiRealm from checkpoint 1: the Model owns `App` and draws
the legacy UI and runs its handlers directly, while a temporary message-only
`LegacyInput` component (owns no `App`) forwards terminal events. Surfaces are
peeled off that legacy path lowest-risk first. Every checkpoint is a
behaviour-preserving, reviewable commit; none is a completion. Order and per-row
detail are in `tasks.md`. A mixed framework is never a mergeable endpoint (the
completion gate is one final checkpoint).

### D12 — Redraw contract (no framework dirty signal)

TuiRealm provides **no** redraw/dirty signal. The documented loop keeps a
`redraw: bool` on the Model, sets it inside `Update::update` when a `Msg` is
handled, calls `view()` only when it is set, then clears it. This **collides** with
D4/D5's rule that local cursor/scroll/form edits return `None` (no `Msg`): a
purely-local mutation would set no `redraw` flag and the screen would go stale.

Decision: mbv marks the frame dirty whenever `tick` reports that an event was
processed, not only when a `Msg` is produced — the minimal way to honour "local
changes emit no `Msg`" while still repainting them. A permanent `UiRoot`/shell
observer subscribed to terminal events is the native place to observe "an event
occurred" without a domain message. Reconciling this per-event redraw with the
current independent 16/150/1000 ms render cadence (D2) is a CP1 detail; the
contract is only that a local mutation cannot leave the frame stale. mbv already
has this signal: `App::run` accumulates `had_events` across every drain and the
input poll and feeds it to `wants_terminal_render` — the migration reuses that path
rather than inventing a dirty flag.

### D13 — Temporary legacy bridge is `LegacyInput`, not `LegacyShell`

During the mixed phase the Model owns `App` and calls the existing legacy rendering
and handlers **directly**. The temporary TuiRealm component is `LegacyInput`:
message-only, owns no `App`, and does one job — translate a terminal event into a
typed legacy message the Model consumes. A stateful `LegacyShell` that rendered
through `App` was rejected because a component mounted inside the `Application`
cannot own or borrow the `App` its enclosing Model holds (Rust aliasing), and the
target boundary forbids a component receiving `App` at all. Each converted surface
deletes its slice of the legacy path; `LegacyInput` is removed at the completion
gate when the last surface leaves it.

### D14 — Conversion is two-stage: mirror first, delete at teardown

A surface converts in **two** steps, in different phases. Conflating them is
what stalled three implementation sessions, so the split is normative here.

**Stage 1 (groups 2–4) — component + mirror.** The component owns its
rendering and its own local interaction state. The shell keeps the `App`
field and the `handle_key_*` handler, and pushes `App` state into the
component every tick through a `sync_<surface>()` method:

```rust
pub(super) fn sync_<surface>(&mut self) {
    let snapshot = /* read + clone off self.app */;
    if let Some(comp) = self.application.get_component_mut(&id) {
        if let Some(c) = comp.as_any_mut().downcast_mut::<XComponent>() {
            c.set_content(snapshot);
        }
    }
}
```

Async completions arrive the same way, after the shell's existing
generation/revision/session/image-key validation (D5) — `apply_drain` in
`sync_search_sidebar` is the reference. This is the "`get_component_mut`
bridge" the tables below refer to, and it already exists: every surface
landed so far (`shell_overlays.rs`, `shell_home.rs`) uses exactly this shape.
There is nothing further to design or build for it.

**Stage 2 (group 5) — teardown.** Only now is the `App` field deleted, the
legacy handler removed, and the mirror dropped.

The split exists because ownership does not transfer one surface at a time.
An `App` field is typically read by several *unrelated* authorities — the
surface's own actions, a different destination's data build, a management
flow's reset, and two or three input-routing files. Deleting it demands
migrating all of them in one diff; mirroring demands none of them. So stage 1
stays a small, genuinely independent diff per surface, and stage 2 is
scheduled by **authority cluster** rather than by surface, since a cluster's
fields are read only within the cluster plus the shell.

Two consequences follow, and both are deliberate:

- **A mirror is the correct stage-1 outcome, not technical debt smuggled in.**
  The spec's no-mirror requirement is a property of the *completion gate*, not
  of each task ("The migration MAY use internal checkpoints, behaviour-
  preserving commits, and temporary adapters"). A stage-1 task that tries to
  delete `App` state has mis-read its bar.
- **Input-precedence questions defer to 5.2.** Because a stage-1 surface keeps
  forwarding to legacy input, it needs no subscription guard yet. The gates
  that cannot be expressed as a static `SubClause` (`playback`, `lib_search`,
  `album_track_mode`) and the per-instance guard for one-component-per-tab
  surfaces are therefore answered once, when the precedence table moves as a
  unit — which is the only point they can be answered coherently.

The ledger records the two stages as two states: `component` (stage 1 landed)
and `migrated` (stage 2 landed). Only `migrated` satisfies the completion gate.

### **D15 — `Cmd` in, `Msg` out. Decide at 5.4; redraw gating explicitly deferred.**

`Component::perform(&mut self, cmd: Cmd) -> CmdResult` is stubbed
`CmdResult::NoChange` on all 28 components, alongside stub `query`/`attr`/
`state`. Those stubs are honest today — mbv declined TuiRealm's state model
(props in via `attr`, state out via `state`/`CmdResult`) in favour of its own:
domain data in via `set_content()`, typed intent out via `Msg`. That remains
the right call for the data path and is not reopened here.

The **input** path is different, and 5.4 forces the question.

**The problem 5.4 inherits.** `KEY_POLICY` and `KeyPolicyGate::sub_clause()`
are referenced nowhere outside `key_policy.rs`'s own ordering test; the file
still carries `#![allow(dead_code)]`. 5.2 turned the gate descriptions into
real `SubClause` values, but nothing executes them. A precedence table with no
execution path drifts silently from the behaviour it claims to describe — the
failure mode that already produced stale entry-13/23 comments (fixed in
`a3c7502`). 5.4's six precedence proofs must either give the table an
execution path or assert against the table rather than runtime behaviour.

**`perform` is that execution path.** `Application::get_component_mut` returns
`Option<&mut dyn AppComponent<Msg, UserEvent>>` (`application.rs:237`) — a
trait object. `perform(cmd)` therefore dispatches without a downcast, so the
policy layer can resolve owner-id → `Cmd` → dispatch while naming no concrete
component type. Every `sync_*` today gives that up: `get_component_mut` →
`as_any_mut()` → `downcast_mut::<XComponent>()`.

**Adopt the argument, not the return.** `CmdResult` serves two purposes, both
already covered or unused here:

* *"what changed"* for redraw gating — mbv redraws unconditionally at
  `shell.rs:725`, and nothing in `src/` reads a `CmdResult`.
* *reporting new state* — `Msg` is a real enum and beats
  `CmdResult::Custom(&'static str, State::Any(Box<dyn Any>))` outright.

So: `Cmd` in, `Msg` out, `perform` returns `NoChange`.

**Vocabulary.** The built-ins cover most of it — `Move(Direction)`,
`Scroll(Direction)`, `GoTo(Position::Begin|End|At(idx))`, `Submit`, `Cancel`,
`Type(char)`. The residue (series↔episode pane switch, album-track mode entry)
extends through `Cmd::Custom(&'static str)`, which carries no payload — and
does not need to, because every residual action is a payload-free mode switch.
Wrap the tag so the string never escapes:

```rust
enum Action { EnterTrackFocus, ExitTrackFocus, SwitchPane, ... }
impl Action { const fn tag(self) -> &'static str; fn from_tag(&str) -> Option<Self> }
impl From<Action> for Cmd { fn from(a: Action) -> Cmd { Cmd::Custom(a.tag()) } }
```

A round-trip test over all variants pins `tag`/`from_tag`, the same way the
ordering test pins `KEY_POLICY`.

**Cost — mostly schedule, not code.**

1. **Not incrementally adoptable.** While any surface still forwards
   `Msg::Legacy(LegacyTerminalEvent::Key(key))` to `App::handle_key`, the raw
   crossterm key must survive the trip, and `Cmd` cannot carry it. So this
   lands wholesale after 5.3d or not at all.
2. **Modifier handling moves to the policy table.** `Cmd` has no modifier
   concept, so state like `browser.rs:78`'s `Char('/')` *with*
   `modifiers.is_empty()` must resolve to a distinct `Cmd` shell-side. That is
   what the table is for, but it is a real relocation.
3. **28 components change their input surface** — `perform` replaces
   `handle_key`, a move rather than an addition, except during transition when
   both exist.
4. **Component-local key meaning must stay local.** `music_workspace.rs:108`
   reads `Enter` as *enter track focus* only when `track_cursor.is_none()`, and
   `Key::Up` as track-move or album-move depending on the same field. The
   generic `Cmd` preserves this — the component receives `Move(Up)`/`Submit`
   and decides locally. Any design that resolves those shell-side would
   re-couple the shell to component state and undo the migration.

**Deferred deliberately.** Redraw gating is the one genuine capability in
`CmdResult` that mbv lacks, and it is worth having over ssh and on battery. It
is not part of this decision. Adopting it means giving every `perform` and
`view` an accurate changed/unchanged answer, which is a performance project,
not an input-routing one.

**If 5.4 declines this,** it must say so and assert its six proofs against the
`KEY_POLICY` table directly, leaving `#![allow(dead_code)]` in place with a
note naming this decision.


## Risks / Trade-offs

- **TuiRealm delivers to active + subscribers, not first-match.** This differs
  from mbv's current loop but is routine to reproduce, not a real risk: nearly all
  keys go to the focused component, and the small fixed set of global bindings
  (quit, tab, overlay-open, transport) become gated subscriptions. The global key
  set is small and the existing #131 input characterization tests catch a wrong
  guard immediately. → All guards come from the single `key_policy` table. The only
  substantive note is: do not assume the framework hands you first-match for free.
- **Mouse routing rides `EventClause::Any` (the `Mouse` clause ignores
  `kind`/`modifiers`; no per-component forwarding).** The one mildly-unconventional piece — though mbv already hit-tests
  clicks spatially today, so only the plumbing changes. Each visible region
  self-filters by geometry. → Bounded set, `None` for non-owned events, guarded off
  under overlays.
- **`UserEvent` payload bounds (`Eq`/cheap-`Clone`).** Rich completions cannot be
  cheap `Eq`/`Clone`. → The token + shell-staging pattern (D5) keeps models
  shell-owned and preserves every stale-completion guard; components read only
  validated models.
- **`LegacyInput` could become a permanent crutch.** → The completion gate and
  ledger forbid a mixed endpoint; `LegacyInput` and its adapters must be deleted
  in the final checkpoint, verified by enforcement (no `impl App` interaction
  handlers remain).
- **Deferred mouse paths are unavailable in alpha.** → Supported paths use
  component-owned `hit_test`; the global router and hit map are deleted rather
  than retained as a fallback. Later restoration must use painted component
  geometry and may not recreate duplicated coordinate routing. Render-only layout
  state can remain.
- **MSRV bump.** → 1.88 is already met by current toolchains; declared once in
  `[workspace.package]` and asserted in CI.
- **Scope.** 29 interactive rows is a large change even with checkpoints. →
  Split across sequential implementation lanes by risk tier; the ledger tracks
  progress and each row carries its own verification record.

## Migration Plan

The deliverable is the complete migration; there is no acceptable in-between state.
The phases below are an internal implementation sequence, **not** partial
deliverables, mergeable milestones, or declared-done resting points. A mixed
TuiRealm/legacy framework is never an endpoint: the change is finished only when the
completion gate passes and all legacy machinery (`LegacyInput`, `CONTEXT_STACK`,
`AppLayout`, adapters, mirrors) is deleted. The whole conversion lives on one
long-running branch and is released only when complete.

Phasing earns its keep purely as build technique: the Model-drawn legacy path plus
`LegacyInput` keeps the app runnable at each phase so conversions can be tested by hand (there is no production
canary), and behaviour-preserving steps keep that testing meaningful. The phase
order is an implementation detail, not a governance decision. High-level order
(detailed in `tasks.md`):

1. Foundation: dependency + MSRV, Model/`Application` skeleton, `LegacyInput`
   adapter running the loop on TuiRealm, enforcement scaffolding.
2. Low-risk leaves: Help, Confirm/DaemonLost/RemoteReanchor modals, context menu.
3. Medium: Home, Emby browsers, Feeds, global Search (with render-seam
   extraction), inline library Search, Sessions, selection modal, playback
   prompts, Settings popups.
4. High: Queue, Library parent, TV, Music, ABS books/podcasts, Playlists +
   save dialog, Settings + setup forms, root/overlay routing, inline album-track.
5. Completion gate: migrate each remaining surface through the teardown pipeline
   in D17, remove `LegacyInput`, `CONTEXT_STACK`, the global mouse router/hit map,
   interaction adapters, and per-frame interaction mirrors; render-only layout
   state may remain. Flip all ledger rows to `migrated`; verify keyboard,
   responsive/render, supported-mouse, and architecture gates.

## Implementation mapping tables

These four tables close the "maps still needed" list from the readiness audit and
are the pre-implementation deliverable; they resolve the row count (29) and the
task-to-row mapping. Owners/guards apply the decisions above (native focus, the
`get_component_mut` bridge, shell-owned adapters, `LegacyInput`). Source of truth:
`docs/architecture/interactive-surface-ledger.md`, `src/app/input_resolver.rs`
(`CONTEXT_STACK`), and `src/app/mod.rs` (`App::run`).

### Table A — Runtime receiver matrix (drain order in `src/app/mod.rs`)

All receivers stay **shell-owned adapters** (D5); each sets `had_events`, which
drives the existing `wants_terminal_render` redraw (D12/D13). "Replaced" = the shell
swaps this receiver during the process lifetime.

| # | Receiver / drain | Replaced at runtime | `UserEvent` token | Target component(s) | Stale guard |
| - | - | - | - | - | - |
| 1 | `emby_startup_rx` | yes (taken/reinserted per iter) | `Startup` | shell → mounts Browsers | startup generation |
| 2 | `emby_setup_rx` | yes | `Startup` | Settings / Root | setup completion |
| 3 | `drain_audiobookshelf_events` | — | `AbsSocket`/`Startup` | ABS browsers | — |
| 4 | `player_rx` | yes | (Player → shell) | Playback chrome, Queue | may `continue` loop |
| 5 | `drain_notif_actions` | — | (internal) | prompts / toast | — |
| 6 | `lib_rx` | — | `LibraryReady(BrowserKey,Gen)` | Browser | library generation |
| 7 | `maybe_flush_search_debounce` | — | `Clock` | Search sidebar | debounce deadline |
| 8 | `drain_search_results` | — | `SearchReady(gen)` | Search sidebar | search gen |
| 9 | `drain_session_events` | — | `Session(gen)` | Sessions | session gen |
| 10 | `drain_cast_events` | — | `Cast(gen)` | Sessions | cast gen |
| 11 | `drain_shared_events` | — | `SharedData(rev)` | Root / Playback | shared rev |
| 12 | `drain_feed_tab_results` | — | `Feed(key,gen)` | Feeds | feed gen |
| 13 | `drain_feed_add_results` | — | `Feed(key,gen)` | Feeds | — |
| 14 | `card_image_rx` | — | `Image(key)` | shell image cache → components | LRU / key |
| 15 | `drain_image_fetches` | — | `Image` | shell image cache | — |
| 16 | `resize_response_rx` | — | `Image` | shell image cache | mem-key + protocol id |
| 17 | `ws_rx` | yes | `Websocket` | Playback / Sessions | — |
| 18 | `audiobookshelf_socket_rx` | yes | `AbsSocket` | ABS / Playback | — |
| 19 | `idle_feed.items_rx` | — | `Feed`/`Clock` | idle / visualizer | 30-min refetch |

Periodic loop work (not receivers) stays a shell timer: visualizer sync,
settings-save debounce (`settings_save_at`), 1 s session poll, cast-status poll,
30 s ws keepalive, 600 s capabilities. A `Clock` `UserEvent` drives any
component-owned deadline (e.g. Search 300 ms).

### Table B — Input context matrix (`CONTEXT_STACK`, 24 entries, precedence order)

Precedence = list order (first-match). "active" = the key reaches this owner as
TuiRealm's active component; "sub" = a gated `Sub(EventClause::Keyboard, guard)` on
the named owner. Blocking overlays `Swallow` unbound keys; globals are guarded
`Not(IsMounted(blocking overlay))` (D7). Because TuiRealm runs the active component
before subscribers, an active leaf returns `None` for any key a higher-precedence
sub should win; guards come from this one table and are locked by the #131 tests.

| # | Context (name) | TuiRealm owner | Gate | Ledger surface |
| - | - | - | - | - |
| 1 | `context_menu` | active `Overlay(ContextMenu)` | mounted+focused, swallow | Context menu |
| 2 | `selection_modal` | active `Overlay(SelectionModal)` | mounted+focused, swallow | Selection modal |
| 3 | `daemon_lost_modal` | active `Modal(DaemonLost)` | mounted, swallow | Daemon-lost modal |
| 4 | `confirm_modal` | active `Modal(Confirm)` | mounted, swallow | Confirm modal |
| 5 | `remote_reanchor` | active `Modal(RemoteReanchor)` | mounted, swallow | Remote-reanchor popup |
| 6 | `save_playlist` | active `Modal(SavePlaylist)` | mounted, swallow | Save-playlist dialog |
| 7 | `settings` | active `Overlay(Settings)` | mounted+focused | Settings sidebar |
| 8 | `help` | active `Overlay(Help)` | mounted+focused | Help sidebar |
| 9 | `sessions` | active `Overlay(Sessions)` | mounted+focused | Sessions sidebar |
| 10 | `playlists` | active `Overlay(Playlists)` | mounted+focused | Playlists sidebar |
| 11 | `global_overlay_open` | sub on `UiRoot` | `Not(IsMounted(blocking overlay))` | Root/overlay routing |
| 12 | `queue_column_width` | sub on `Queue` | Queue visible | Queue |
| 13 | `search_sidebar` | active `Overlay(Search)` | mounted+focused | Global Search sidebar |
| 14 | `lib_search` | active `InlineSearch(BrowserKey)` | mounted+focused in browser | Inline library Search |
| 15 | `panel_mode_cycle_x` | sub on `Library` | Library visible | Library parent |
| 16 | `confirm_skip_intro` | sub on Playback prompt | prompt visible | Playback prompts |
| 17 | `confirm_next_up` | sub on Playback prompt | prompt visible | Playback prompts |
| 18 | `clear_queue_prompt_c` | sub on `Queue` | prompt visible | Queue (prompt) |
| 19 | `visualizer` | sub on `Playback` | visualizer active | Playback chrome |
| 20 | `playback` | sub on `Playback` | `player_active`/remote gate (`resolve_key` seam) | Playback chrome |
| 21 | `ctrl_l_force_clear` | sub on `UiRoot` | `Always` (global) | Root (global) |
| 22 | `f5_refresh` | sub on `UiRoot` | `Always` (global) | Root (global) |
| 23 | `album_track_mode` | active child of Music `Browser` | `album_track_focus.is_some()` | Inline album-track |
| 24 | `view_dispatch` | active focused destination | the focused Library leaf | active destination (catch-all) |

### Table C — Hierarchy / render matrix (`ComponentId` → placement)

| `ComponentId` | Parent | Mount lifetime | Owns | Outer `Rect` from |
| - | - | - | - | - |
| `UiRoot` | — | session | active-destination + overlay z-order query | full terminal |
| `Playback` | `UiRoot` | session | transport chrome | root arrangement |
| `Queue` | `UiRoot` | session | cursor / scroll / scope | root arrangement |
| `Library` | `UiRoot` | session | **selected destination child** | root arrangement |
| `Home` | `Library` | session | cross-Service rows | Library area |
| `Browser(BrowserKey)` | `Library` | while library exists | list / hero cursor | Library area |
| `Feeds` | `Library` | while exists | grouping / selector | Library area |
| `InlineSearch(BrowserKey)` | `Browser` | while browser offers search | query draft | within browser |
| `Overlay(OverlayId)` | `UiRoot` | open→dismiss | own state; z by focus stack | overlay arrangement |
| `Modal(ModalId)` | `UiRoot` | open→dismiss | blocking; top z | centered arrangement |
| `Popup(PopupId)` | `Overlay(Settings)` | open→dismiss | nested Settings child | within Settings |

Focus is TuiRealm's native LIFO stack (D6); `Library` and `UiRoot` own only
selected-child / z-order, queried by the render plan — not focus.

### Table D — 29-surface transition matrix (ledger row → component → task)

| # | Ledger surface | `ComponentId` | Current owner (ledger) | Task |
| - | - | - | - | - |
| 1 | Root UI + overlay routing | `UiRoot` | `App`, `render/screens/root.rs`, `CONTEXT_STACK` | 5.2 |
| 2 | Playback chrome + global controls | `Playback` | `App`, `action.rs`, `input_mouse_dispatch.rs` | 4.10 |
| 3 | Queue | `Queue` | `App`, `input_queue_keys.rs`, `render/screens/queue.rs` | 4.1 |
| 4 | Library parent | `Library` | `App`, `input_browse_dispatch.rs` | 5.1 |
| 5 | Home | `Home` | `App.home`, `home_actions.rs` | 3.4 |
| 6 | Emby generic/Movies/home-video browser | `Browser` | `App.libs`, library actions | 3.5 |
| 7 | Inline library Search | `InlineSearch` | `LibSearch` in `LibraryTab`, `input_lib_keys.rs` | 3.3 |
| 8 | TV workspace | `Browser` (TV kind) | `LibraryTab` series state | 4.2 |
| 9 | Grouped Music workspace | `Browser` (Music kind) | album/music state | 4.3 |
| 10 | Audiobookshelf podcast browser | `Browser` (ABS podcast) | ABS browse state | 4.5 |
| 11 | Audiobookshelf book browser | `Browser` (ABS book) | ABS book state | 4.6 |
| 12 | Feeds | `Feeds` | feed state/actions | 3.6 |
| 13 | Overlay stack | `UiRoot` (routing) | `App` flags, `CONTEXT_STACK` | 5.2 |
| 14 | Global Search sidebar | `Overlay(Search)` | `SearchSidebar` + `App` paths | 3.2 |
| 15 | Settings sidebar + setup forms | `Overlay(Settings)` | `App` settings/forms | 4.9 |
| 16 | Multiselect popup | `Popup(Multiselect)` | `App.multiselect_popup` | 3.10 |
| 17 | Library-routes popup | `Popup(LibraryRoutes)` | `App.library_routes_popup` | 3.10 |
| 18 | Feed-management popup | `Popup(FeedManage)` | `App.feeds_manage_popup` | 3.10 |
| 19 | Sessions sidebar | `Overlay(Sessions)` | `App` sessions/targets | 3.7 |
| 20 | Playlists sidebar | `Overlay(Playlists)` | `App` playlist state | 4.7 |
| 21 | Save-playlist dialog | `Modal(SavePlaylist)` | `App.save_playlist_dialog` | 4.8 |
| 22 | Help sidebar | `Overlay(Help)` | `App.show_help/help_scroll` | 2.1 |
| 23 | Context menu | `Overlay(ContextMenu)` | `App.context_menu` | 2.5 |
| 24 | Selection modal | `Overlay(SelectionModal)` | `App.selection_modal` | 3.8 |
| 25 | Confirm modal | `Modal(Confirm)` | `App.confirm_modal` | 2.2 |
| 26 | Daemon-lost modal | `Modal(DaemonLost)` | `App.daemon_lost_modal` | 2.3 |
| 27 | Remote-reanchor popup | `Modal(RemoteReanchor)` | `App.remote_reanchor_popup` | 2.4 |
| 28 | Playback prompts (skip-intro/next-up) | Playback-prompt (Root-level; add a `Prompt` id or model under `Playback`) | `App` prompt state | 3.9 |
| 29 | Inline album-track | child of Music `Browser` | `LibraryTab.album_track_focus` | 4.4 |

All 29 ledger rows map to exactly one task; groups 2–5 cover every row, and 5.5
flips them to `migrated`.

## Open Questions

- Which destination components (if any) should be **eagerly** unmounted under
  memory pressure vs. kept mounted for state retention. Deferrable: default is
  keep-mounted-while-library-exists (D6); a later tuning pass can add eviction
  without changing the specs, shapes, or task breakdown.
- ~~`PollStrategy` choice~~ **Resolved:** use `PollStrategy::Once(Duration)` (the
  docs' recommended default, e.g. 10 ms), which delivers at most one event per
  tick and best matches the current one-event-per-iteration loop. It is
  **behaviour-bearing** (batched strategies reorder active-vs-subscription
  messages), so it is a contract, not a cadence tweak.

### **D16 — Mouse is accepted-broken for alpha; the framework is deleted rather than migrated.**

Decided 2026-08-25 by the maintainer, superseding the per-surface plan recorded
under 5.3d *Mouse geometry*.

**The decision.** The remaining legacy mouse framework — `input_mouse.rs`,
`input_mouse_dispatch.rs`, `input_mouse_gestures.rs`, and the layout fields that
exist only to serve them — is deleted outright. The surfaces that had not yet
taken ownership of their own hit geometry (`music_workspace`, and the
`confirm` / `daemon_lost` / `remote_reanchor` / `playback_prompt` bundle) are
**not** migrated first. Mouse interaction on those surfaces is allowed to be
broken in the alpha build and will be verified and repaired at a later date,
against real usage rather than against reconstructed reachability arguments.

**Why this is not a regression in the migration's terms.** 5.3d's deliverable
was never mouse correctness; it was removing the parallel legacy interaction
framework so 5.6's gate can pass. mbv is a terminal client and keyboard is the
product surface. The five units that did land (`browser` `24c550bc`, `home`
`c7784c47`, `queue` `d6d4fada`+`1cce9cd6`, `tv_workspace` `c70e3e0`) are what
make blunt deletion safe rather than total: each of those components already
hit-tests its own painted geometry and sets its own cursors before forwarding,
so removing the legacy sink degrades mouse on the unmigrated surfaces instead
of removing mouse everywhere. Deleting on day one would have had the latter
effect.

**What this cost, deliberately.** Two open questions are closed by deletion
rather than answered. `c70e3e0` removed the episode-row and season-tab branches
from `click_set_cursor` on the unverified premise that a podcast library never
selects an `item_type == "Series"` item — the render gate is
`is_wide_tv_library || is_podcast_library` (`list.rs:125`) while the component
mount gate is `collection_type == "tvshows"` (`shell_tv_workspace.rs:13`), and
the two predicates were never proven to coincide. That premise no longer
matters, because `click_set_cursor` is deleted with the rest. Separately, the
`tv_wide_*` / `wide_music_*` fields on `LayoutMain` are screen-named geometry
describing hero-on-left's child panels, and `is_wide_tv_active()` infers
arrangement state from whether those fields were painted. That naming misled at
least one implementer into treating shared geometry as surface-exclusive. It is
not renamed here; whatever survives deletion inherits the problem.

**What is deferred, and what would close it.** Mouse verification is deferred to
a post-alpha pass driven by manual use. Closing it means deciding, per surface,
whether the gesture is worth restoring at all, and restoring it the same way the
five landed units did — component hit-tests its own geometry, emits a typed
`Msg::Shell`, shell owns timing via `App`'s single click/scroll clock. Do not
reintroduce a global hit map or a second clock.

**Effect on 5.4.** 5.4's six precedence proofs were scheduled to run inside this
deletion unit. The three that concern mouse — simultaneous Queue+Library mouse,
blocking-overlay swallow of mouse, and "geometry cannot drift" — apply only to
the alpha-supported mouse paths. For deferred paths they become structural
checks: the absence of the three `input_mouse*.rs` files and of any global hit
map. The keyboard precedence proofs are unaffected.

### D17 — Group 5 teardown is discovery-led and staged by dependency

Groups 1–4 could be planned by visible surface because their outcome was a
component behind a deliberate mirror. Group 5 cannot be planned as "delete every
mirror" or as one task per screen: a `sync_*` commonly combines mount lifecycle,
content projection, focus, layout derived by the legacy renderer, and a two-way
interaction-state pin. Raw input may independently repeat the component's local
transition before performing shell-owned effects. The real unit of work is one
ownership dependency, not one `sync_*` function.

Before a writer is assigned to a remaining surface, a read-only scout records a
durable symbol-level handoff under `openspec/handoffs/` covering:

1. every input to the mirror and every production writer of those inputs;
2. component-local interaction state versus shell-owned content/cache/effect state;
3. raw input forwarding and the exact existing effect entry points;
4. legacy underpaint, cover/image work, and layout values produced only by that
   renderer;
5. unrelated readers that prevent immediate `App` field deletion; and
6. the smallest compile-complete implementation units and their dependency order.

Discovery and implementation are separate assignments. A normal writer receives
one closed behaviour/ownership family touching roughly three to six production
files. If implementation exposes a missing authority or exceeds that bound, it
stops and returns the coupling instead of absorbing design discovery. Larger
mechanical fan-out requires a named preparation unit first.

A surface normally advances through these teardown stages, omitting a stage only
when the scout proves it does not exist:

1. separate mount reconciliation from per-frame content projection;
2. replace projection with targeted pushes at validated writer choke points;
3. replace raw input forwarding one coherent behaviour family at a time with
   typed intents, keeping shell-owned effects at existing boundaries;
4. remove interaction-state pins and obsolete `App` readers only after all
   remaining consumers are re-homed;
5. detach component geometry/content from legacy underpaint, then delete that
   surface's legacy renderer; and
6. remove the now-empty mirror/mount adapter and legacy handler endpoint.

Direct pushes of validated shell-owned content/cache/effect presentation are not
forbidden mirrors. The forbidden completion state is per-frame or two-way
synchronisation of component-local interaction state. Mount reconciliation may
remain temporarily after projection is removed, but must be renamed or deleted at
the surface completion stage so `sync_*` no longer hides multiple authorities.

The parity authority is current observable behaviour, including current effect
targets. When the component and legacy path resolve the same key differently
(for example a one-item component move versus a legacy multi-column row move),
the discrepancy is a blocking discovery result, not permission to choose the
cleaner interpretation. Characterize or otherwise prove the active production
path first; improvements remain out of scope.

Surface teardown precedes global framework deletion. Only after every remaining
raw-key endpoint and interaction mirror has gone may the campaign re-inventory and
delete `CONTEXT_STACK`, `LegacyInput`, and terminal reconstruction adapters.
Repository-wide line-cap verification runs at the final 5.6 gate; bounded units
run only their named compile, focused/full existing-test, lint, architecture, and
format checks.

### D18 — Emby `wide_movies` is a per-draw render adapter now, component-owned at underpaint removal

The Emby generic/Movies/home-video browser needs to know whether it is in the
wide Movies/home-videos hero-on-left presentation because `columns()` returns one
in that case (matching legacy `App::current_library_columns`). The signal is
`App::layout.main.is_wide_movies_active()`, which reads `movies_wide_right_area`
populated by the legacy wide renderer inside the base frame draw. The component's
own `LayoutMain` never publishes that rail, so the shell pushes it in
(`set_wide_movies`).

Two-stage resolution, forced by D17's stage ordering (detach underpaint at stage
5, before deleting the legacy renderer):

1. **Now (5.3d.15–16): temporary per-draw adapter.** `set_wide_movies` moves out
   of the per-frame `sync_emby_browser` interaction mirror and into
   `render_emby_browser_component` in the draw closure, after the legacy base
   frame has set `movies_wide_right_area` this frame — the same render-only
   adapter shape as `dim_backdrop_active`, which D16 permits to remain. This
   also removes the current one-frame lag (the mirror ran before the base frame).
2. **At 5.3d.17/R1: component-owned derivation.** When the legacy wide renderer
   is deleted, `movies_wide_right_area` disappears and `is_wide_movies_active()`
   becomes dead. The component then derives "wide" from its own library kind
   (Movies/HomeVideos, already present in the `BrowserKey`) plus its geometry
   width at the `shared_hero_presentation`/`wide_library_panes` breakpoint,
   pushed at writers — no per-frame mirror survives.

Do not compute the type × width derivation into the component before underpaint
removal: replicating the breakpoint ahead of stage 5 risks parity drift against
the still-active legacy renderer.

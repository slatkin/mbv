## Context

See `proposal.md` — Why. The relevant current state:

- **Delivery.** `src/app/shell.rs` mounts every component with `vec![]`
  subscriptions except `UiRoot`, which subscribes `EventClause::Any` +
  `SubClause::Always` and collapses `Event::Mouse` to
  `TerminalObserverEvent::Mouse`, dropped at `shell.rs:349-351`. TuiRealm 4.1
  delivers `Event::Mouse` to the focused component and to subscribers whose
  `EventClause` matches; `Application::tick(PollStrategy::Once(..))` returns the
  focused component's message then subscriber messages, and the shell already
  folds a two-message list for keyboard (`shell_run.rs` `router_outcome` /
  `apply_router_outcome`, ADR 0023).
- **Landed surfaces.** `browser`, `home`, `queue`, `tv_workspace`, partial
  `music_workspace` capture painted geometry in `view()` and, when focused,
  translate `Event::Mouse` into `Msg::Shell(ShellRequest::…)` / `Msg::Playback`.
  The shell applies those via `mouse_gestures.rs` (`impl App`) using
  `App.last_click_time` / `last_click_pos` / `last_scroll_at` — a shell-side
  clock keyed by screen position. `msg/hit_regions.rs` holds one `*HitRegion`
  enum per surface.
- **Stubs / dead code.** `handle_mouse_scroll_browse(_delta)` is an empty match.
  `PlaybackComponent` is mounted with `vec![]` and never activated, so its
  `mouse()` handler never runs.
- **Spec constraints.** `interactive-component-framework`,
  `ui-design-system`, and `context-menu` forbid a global hit map / global mouse
  router / duplicated coordinate path, and require component-owned hit geometry.
  ADR 0022 (TuiRealm migration) and ADR 0023 (one keyboard router) are the
  architectural precedents this design parallels.

## Goals / Non-Goals

**Goals:**

- One delivery + arbitration mechanism for mouse, built on TuiRealm
  subscriptions, that every interactive component plugs into the same way.
- Gesture recognition primitives shared by all components, sized so drag and
  hover are additive.
- Full per-surface mouse parity, tracked in the ledger like keyboard parity.
- Verification through `Application::tick()` against the real synchronisation
  order, matching the framework-behaviour testing rule.

**Non-Goals (design-level):**

- Drag-and-drop and hover behaviour, `MouseEventKind::Moved` / `Drag(_)`
  handling. The delivery model must not preclude them; nothing else about them
  is designed here.
- Any change to keyboard routing, the keyboard router fold, or ADR 0023.
- Re-opening which surfaces are `migrated` — mouse parity is added to existing
  rows, it does not change migration status.
- A new dependency. `ratatui-interact` is evaluated in D0 and rejected.

## Decisions

### D0 — No mouse-framework dependency; build a thin house layer on TuiRealm

The ratatui/TuiRealm ecosystem has no drop-in mouse framework. `ratatui` core
exposes only `Rect::contains(Position)` (discussion #1051 unresolved).
`ratatui-interact` implements exactly the pattern we need (`ClickRegionRegistry`:
register `Rect`+tag during render, `handle_click(x,y)` during events) plus a
`FocusManager` and stateful widgets — but it is ratatui-native, its
`FocusManager` and `EventResult` model collide with TuiRealm's focus stack,
subscriptions, and our `Msg`/`Component` architecture, it ships a widget set we
do not want, and it is single-author with no maintenance track record. `cursive`
has full mouse support but is a different framework entirely.

TuiRealm 4.1 already provides the delivery half: `Event::Mouse` through the same
`on()` path as keyboard, `EventClause::Mouse(MouseEventClause { kind, column,
row })` subscription clauses, and a `MouseEventKind` enum that already models
`Drag(MouseButton)` and `Moved`. The house layer is `HitRegions<Tag>` +
`MouseGestureState`, ~300 lines, and `ratatui-interact`'s registry is
independent confirmation the shape is right. We lift the pattern, not the crate.

**Alternative considered:** take `ratatui-interact` as a dependency and adapt.
Rejected — the adaptation surface (bridging two focus models, two event-result
models) is larger than the code it saves, and it couples a core interaction path
to an unproven external crate.

### D1 — Delivery: any-position mouse subscription + component-owned hit-test

Every mounted interactive component subscribes with
`EventClause::Mouse(MouseEventClause { kind: <any>, column: 0..=u16::MAX,
row: 0..=u16::MAX })` + `SubClause::Always` (helper `mouse_sub()`), so it
receives every mouse event while mounted. The component decides ownership in
`on()` by testing coordinates against the geometry it captured in its last
`view()`.

**Alternative considered:** dynamic position-scoped subscriptions, re-registered
each frame to match current geometry. Rejected — subscriptions are set at mount
time; geometry changes every frame (resize, breakpoint, scroll), so this means
tearing down and rebuilding subscriptions per frame. The any-position sub +
in-`on()` hit-test is what the five landed surfaces already do informally.

### D2 — Arbitration: a shell-side mouse fold with fixed surface priority

`shell_run.rs` gains a mouse-message fold beside the keyboard router fold. When
`tick()` returns more than one mouse-derived message, the shell keeps at most
one, chosen by priority: **topmost mounted overlay/modal > active panel >
other visible panel > chrome**. Overlay order comes from `root.rs`
`OVERLAY_IDS` (already the canonical z-order). While a blocking overlay is
mounted, all mouse messages from components beneath it are discarded — the fold
does this centrally, so components never need a "am I blocked" flag.

This is the keyboard router fold's shape (`RouterOutcome` →
`apply_router_outcome`) applied to mouse, not a second event loop or a global
router. Sibling panels do not overlap, so the only real conflict the fold
resolves is overlay-vs-underlying.

**Alternative considered:** each component checks a shared "input blocked" flag.
Rejected — spreads the blocking rule across N components and re-introduces shared
mutable input state.

### D3 — Gesture recognition moves into components (the A/B fork, resolved B)

**B (chosen):** each interactive component owns a private `MouseGestureState`.
It feeds raw `MouseEvent`s in and gets a recognized gesture out — `Click`,
`DoubleClick`, `RightClick`, `Scroll` now; `DragStart/Move/End`,
`HoverEnter/Leave` reserved. The component maps the gesture + its resolved
`HitRegions<Tag>` target to a semantic typed `Msg`. The double-click interval
and wheel throttle live in `MouseGestureState`, per component.

**A (rejected):** keep all timing in `App`'s shell-side clock; components report
raw hit regions; the shell coalesces single-vs-double and routes the resolved
gesture back per surface. This is today's pattern. Rejected because it does not
scale to the planned drag/hover work: a drag is inherently a component-local
interaction (you drag within one surface's rows), and a hover highlight is
component-local visual state. Under A, every new gesture needs new `*HitRegion`
variants and new `ShellRequest` plumbing on every surface; under B it is a
`MouseGestureState` output variant plus one component's `Msg`.

**Reconciling with D16's "do not reintroduce a second clock":** D16 forbade the
*global* completed-frame hit map and a clock keyed by screen position shared
across surfaces. A `MouseGestureState` owned by one component, keyed by nothing
but that component's own recent events, is neither — it is the same shape as a
component's existing private cursor/scroll state. The design.md of
`migrate-tui-to-tuirealm` D16 explicitly says restoration should be "the same
way the five landed units did — component hit-tests its own geometry, emits a
typed `Msg::Shell`"; B extends that, A freezes it mid-way.

### D4 — Components emit semantic `Msg`, never raw coordinates

A component's mouse `Msg` carries the resolved target — a row identity, a
`QueueSlotId`, a pill index, a transport control — not `col`/`row` for the shell
to re-resolve. This is the `interactive-component-framework` "own only
presentation authority" rule (no shell field written to be read back) applied to
mouse. The one apparent exception, the context-menu anchor, is a click position
that is display geometry the component legitimately owns and forwards; it is not
a target the shell re-resolves.

### D5 — Focus-follows-click via the component's claim

A panel component that claims a click emits (or the shell infers from the claim)
a `set_panel_focus` for that panel before the semantic effect runs — exactly how
`handle_mouse_single_click_emby` already calls `self.set_panel_focus(...)`. The
shell applies focus first so a subsequent effect sees the right focused surface.

### D6 — `HitRegions<Tag>` consolidates the per-surface hit enums

New `src/app/components/mouse/hit.rs`: `HitRegions<Tag>` — a component clears it
at the top of `view()`, calls `push(rect, tag)` as it paints, and calls
`resolve(point) -> Option<Tag>` in `on()` (last-pushed-wins for overlap, or
z-ordered explicitly). The per-surface `BrowserHitRegion` / `QueueHitRegion` /
`HomeHitRegion` / `TvHit` enums in `msg/hit_regions.rs` become the `Tag` types
for each surface; the file's shell-side "the shell decides single vs double"
contract is deleted along with the `note_browse_*` helpers.

### D7 — Delivery model chosen so drag/hover are additive

Turning on drag/hover later means: (1) `MouseGestureState` starts emitting the
reserved `Drag*` / `Hover*` variants (it already receives `Drag(_)` and `Moved`
events — crossterm 0.29 sends them, capture is already enabled), (2) the
component adds `Msg` variants and transient local drag/hover state, (3) possibly
a `MouseEventClause` kind filter so hover-move spam is dropped for components
that do not want it. No change to `mouse_sub()`, the fold, or the priority order.

### D8 — `mouse_gestures.rs` keeps effects; recognition leaves

The `impl App` effect handlers in `mouse_gestures.rs` (`handle_mouse_single_click_emby`,
`seek_to_col`, the queue/tv handlers) stay — they are shell-owned effects keyed
off typed `Msg`. What leaves is the recognition glue (`note_browse_double_click`,
`note_browse_scroll`) and the stubbed `handle_mouse_scroll_browse`, which is
replaced by real per-component wheel routing mirroring `Model::handle_home_scroll`.

### D9 — An ADR records the delivery model

A new `docs/adr/0024-mouse-events-through-component-subscriptions.md` fixes D1–D3
as architecture (accepted), so a later contributor cannot re-add a global hit map
without superseding it. The `interactive-surface-ledger.md` "Mouse ownership is
out of scope" section is replaced by a Mouse column populated per row.

### D10 — Phasing

One change, six phases, each independently shippable with its own gate (detailed
in `tasks.md`): 1 delivery spine · 2 shared primitives · 3 main-surface parity +
wheel · 4 overlays & popups · 5 music_workspace + narrow browse · 6 ledger +
precedence-proof close-out. Phase 1 must land the fold and subscriptions with no
new gestures, so regressions are isolated to delivery.

## Risks / Trade-offs

- **Every mounted component hit-testing every mouse event (esp. once hover
  lands)** → N is small (~10 visible), hit-test is a rect check; if hover-move
  volume becomes a problem, a `MouseEventClause` kind filter drops `Moved` for
  components that opt out (D7).
- **The fold mis-prioritises an overlay/panel overlap and a click "falls
  through" or is "eaten"** → this is exactly the deferred D16 precedence proof;
  Phase 6 lands `tick()`-level tests for simultaneous panels and
  blocking-overlay swallow, and Phase 1 lands the overlay-vs-panel case.
- **Phase 2 refactor of the five landed surfaces regresses working clicks** →
  Phase 2 is a pure refactor gated on the existing per-surface mouse tests
  staying green; characterization buffers unchanged.
- **B (D3) is more upfront work than A** → accepted deliberately; A's cost is
  paid back with interest on the first drag-and-drop surface (proposal — Why).
- **Spec deltas touch three large migration requirements** → the deltas
  reproduce each requirement in full (archive-safe) and change only the mouse
  carve-out lines; no other behaviour in those requirements moves.

## Migration Plan

Additive within one branch; each phase is a commit group that leaves the app
runnable. No runtime data migration. Rollback = revert the phase commit; Phase 1
is the only one whose revert restores the "mouse only reaches focused component"
behaviour, and it carries the delivery tests that would catch a regression before
merge.

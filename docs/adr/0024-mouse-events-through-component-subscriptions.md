---
status: accepted
---

# Mouse Events Through Component Subscriptions

mbv delivers raw terminal mouse events to interactive components through TuiRealm
subscriptions. A component receives mouse events while it is mounted **and
painted in the most recent frame**, and it recognizes gestures from those events
itself. There is no global hit map, no global mouse router, and no shell-side
re-resolution of coordinates a component painted.

## Problem

ADR 0022 replaced mbv's framework with TuiRealm and, under decision D16, deleted
the legacy global mouse framework (`input_mouse*.rs`, the completed-frame hit
map, the global router) rather than migrate it. Five surfaces (`browser`,
`home`, `queue`, `tv_workspace`, partially `music_workspace`) took ownership of
their own painted geometry before that decision, but the delivery half was never
rebuilt: `UiRoot`'s observer collapses `Event::Mouse` to
`TerminalObserverEvent::Mouse` and drops it (`shell.rs:349`), TuiRealm otherwise
delivers `Event::Mouse` only to the focused component, and `PlaybackComponent`
carries a full seekbar/transport `mouse()` handler that is never reached because
the component is never made active. Clicking a surface that does not hold
keyboard focus does nothing.

TuiRealm's dispatch does not offer a drop-in replacement for the deleted
framework, and the ecosystem has none either (`ratatui` core exposes only
`Rect::contains`; `ratatui-interact` is ratatui-native and collides with
TuiRealm's focus/subscription/`Msg` model). What TuiRealm 4.1 does provide is the
delivery primitive — `Event::Mouse` through the same `on()` path as keyboard,
`EventClause::Mouse(MouseEventClause { kind, modifiers, column, row })`
subscription clauses, and a `MouseEventKind` enum that already models `Drag` and
`Moved`. Three properties of that primitive shape this decision:

- `MouseEventClause::is_in_range` compares **only `column` and `row`**. It
  ignores `kind` and `modifiers`, despite the crate's own `EventClause::forward`
  doc comment claiming "everything must match".
- `Application::tick` returns `Vec<Msg>` with **no component identity** — the
  focused component's message first, then subscriber messages.
- Both delivery paths call `component.on(&mut self, ev)`: **a component mutates
  itself before it returns a `Msg`.** Discarding the returned `Msg` does not undo
  the mutation. `Application::lock_subs()` is global and would also silence the
  `UiRoot` keyboard router (ADR 0023), so it is unusable here.

This is the mouse counterpart to the problem ADR 0023 solved for keyboard: the
delivery mechanism has no representation for "who should act on this event", and
the naive fix (let every subscriber act, discard the losers) is wrong because the
losers have already mutated.

## Decision

### Delivery — mouse-eligible components subscribe and recognize mouse (D1)

A mouse-eligible component subscribes through a `mouse_sub()` helper with a
single any-position clause:

```rust
Sub::new(
    EventClause::Mouse(MouseEventClause {
        kind: MouseEventKind::Moved,   // some value required; NOT matched
        modifiers: KeyModifiers::NONE, // some value required; NOT matched
        column: 0..u16::MAX,           // half-open Range<u16>, not RangeInclusive
        row: 0..u16::MAX,
    }),
    SubClause::Always,
)
```

so it receives every mouse event at every coordinate. The component then
hit-tests the geometry it last painted. A mounted parent owns a private
`MouseGestureState` and maps recognized gestures to its controls; an embedded
canonical media-list control resolves a point inside the list rectangle to a
stable `Target`; parent-owned pills, Queue scope buttons, overlays, the Playback
seekbar, and non-list wheel/chrome keep their own hit geometry.

**Pinned-dependency assumption.** `mouse_sub()` relies on
`MouseEventClause::is_in_range` ignoring `kind` and `modifiers` in tuirealm 4.1.
That is upstream behaviour we depend on, not a documented contract. Therefore:

- `tuirealm` stays pinned at `4.1` for this change; any bump re-verifies
  `is_in_range` before merge. If a later version honours `kind`, `mouse_sub()`
  becomes one subscription per relevant kind (or `EventClause::Any` plus an
  in-`on()` filter).
- **All kind filtering happens inside the component's `on()`**, never in the
  clause. Nothing in this architecture may rely on the clause dropping event
  kinds (e.g. a component that does not want hover drops
  `MouseEventKind::Moved` as the first arm of its `on()`).

Dynamic position-scoped subscriptions re-registered per frame to match current
geometry were rejected: geometry changes every frame (resize, breakpoint,
scroll), so that means tearing subscriptions down and rebuilding them every
frame. The any-position sub plus in-`on()` hit-test is what the five landed
surfaces already do informally.

### Arbitration — decided by who is subscribed, before `on()` runs (D2)

Because `component.on()` mutates before returning, a shell-side fold that
discards a loser's `Msg` cannot undo the loser's mutation: a click outside a
modal would still move the underlying Feeds cursor, and a wheel event over a
mounted-but-unpainted Browser would still scroll it. **Arbitration therefore
happens in the subscription table, not in a post-hoc fold.**

The shell maintains the set of mouse-eligible components and adds/removes the
`mouse_sub()` subscription as that set changes, in a
`sync_mouse_subscriptions()` that runs in the same synchronisation pass as, and
off the same derivation as, `sync_active_destination()`. Eligibility is a
three-rung ladder:

1. If a **blocking** overlay/modal is mounted → **only** that overlay is
   mouse-eligible.
2. Else if a non-blocking overlay/popup that paints over panel content is
   mounted → the topmost such overlay (by `root.rs` `OVERLAY_IDS` z-order) is
   eligible **exclusively**; underlying panels are not. The popup still receives
   outside-clicks, so its dismissal policy is unaffected.
3. Else → the components painted in the current frame: the active destination,
   Queue, Playback, and chrome. Every other mounted destination is **not**
   eligible.

A non-eligible component's `on()` is never called, so there is no mutation and
no message to undo. One mechanism resolves all three previously-separate
problems: blocking-overlay swallow, overlay-over-panel precedence, and
mounted-but-not-painted destinations.

**The fold survives with a smaller job.** `shell_run.rs` folds the mouse-derived
messages out of a `tick()` beside the keyboard router fold: it applies at most
one (first in `tick()` order) and, in debug builds, asserts that at most one was
produced. It does **not** rank surfaces. Eligible components paint disjoint
rectangles and each emits only for points inside its own painted geometry, so
two mouse messages for one event is a geometry bug and the debug assertion is
how it is found. There is no `Msg`-variant→rank table and no origin
`ComponentId` threaded through mouse messages — that would not solve the
mutation problem anyway, and it would be a second source of truth for z-order
that drifts from `OVERLAY_IDS`.

**One residual bypass, deliberately accepted.** `forward_to_active_component`
delivers to the focused component regardless of subscriptions. This is safe
because the focused component is always painted, and a blocking overlay always
takes focus (`overlay_holds_focus()` gates `sync_active_destination`). A test
asserts exactly that invariant, so a future change that mounts a blocking
overlay without taking focus fails loudly rather than leaking clicks.

### Gesture recognition — in the mounted parent (D3)

Each mounted destination parent owns a private `MouseGestureState`: raw
`MouseEvent`s in, a recognized gesture out — `Click`, `DoubleClick`,
`RightClick`, `Scroll` now; `DragStart`/`DragMove`/`DragEnd`,
`HoverEnter`/`HoverLeave` reserved. The mounted parent maps the gesture to a
semantic request, delegating canonical list point resolution to the embedded
control's `resolve_point`. The double-click interval and wheel throttle live in
`MouseGestureState`, per mounted parent.

Keeping all timing in `App`'s shell-side clock (today's pattern) was rejected: it
does not scale to the planned drag/hover work. A drag is inherently
component-local (you drag within one surface's rows) and a hover highlight is
component-local visual state; under the shell-clock model every new gesture
needs new hit-region variants and new `ShellRequest` plumbing on every surface,
where under parent-owned recognition it is a `MouseGestureState` output variant
plus one parent's `Msg`.

This does **not** reintroduce the second clock D16 forbade. D16 forbade the
*global* completed-frame hit map and a clock keyed by screen position shared
across surfaces. A `MouseGestureState` owned by one mounted parent, keyed only by
that parent's own recent events, is the same shape as a component's existing
private cursor/scroll state.

### Mouse eligibility follows paint, and that is not ADR 0022 migration debt (D11)

A mouse event is a coordinate on the frame the user is looking at. Resolving it
against anything other than the most recently painted geometry is the drift bug,
not the fix. ADR 0022's authority bar is therefore read as:

- **Forbidden:** a *keyboard or behaviour* path branching on what was painted,
  and any component reading geometry it did not itself paint.
- **Required here:** a *pointer* path resolving a point against the geometry the
  resolving component itself painted in the most recent frame, and the shell
  making a component mouse-eligible only while it is painted.

"Mounted" is not "visible": `shell_destination_mounts.rs` keeps a `Browser` /
`TvWorkspace` / `InlineSearch` mounted for every live library, and only the
active one is painted. Without the eligibility gate, a wheel event over the
visible Emby list would also scroll three invisible Browsers — mutation no fold
can undo. The eligibility set is derived from the shell's existing
active-destination derivation, not from a second paint ledger, so there is no
new "did I paint" fact stored anywhere. The `interactive-component-framework`
spec is amended: a component receives mouse events while it is mounted **and
painted in the most recent frame**.

### ADR 0022 Residual A closes in this change (D13)

Residual A ("chrome hit geometry", `docs/adr/0022-…md`) has three sites, all
retired here:

| Site | Closure |
| --- | --- |
| `mouse_gestures.rs:27` — `seek_to_col` reads `layout.playback.seekbar_area` | `PlaybackRequest::SeekTo(u16)` becomes `SeekTo(f64)` carrying a fraction resolved by `PlaybackComponent` against its own `seekbar_area`; `App::seek_to_col` becomes `seek_to_fraction(f64)`. |
| `mouse_gestures.rs:13` — `is_browse_layout_current` reads `layout.main.browse_destination` on a paint-inference gate | Both callers (the stubbed `handle_mouse_scroll_browse` and the Home wheel) are deleted or moved to component-owned recognition, so the gate is deleted. |
| `input.rs:97` — `ensure_tab_visible` reads `layout.tabs_area.width` on a **keyboard** path | The tab-bar width derivation (`render/arrangements/chrome.rs:113`, from terminal width and right-panel visibility) becomes a pure arrangement function called by both the painter and `ensure_tab_visible`, so the keyboard path stops reading the painted rect. |

The third is a shared layout invariant, so it lands in the arrangement layer
(one primitive both callers use), not as a local fix in `input.rs`. When all
three are closed, Residual A is struck from ADR 0022 and its "Known deviations"
list names only Residuals B and C.

## Rules

- One delivery mechanism. Every mouse-eligible component plugs in the same way:
  `mouse_sub()` + in-`on()` hit-test against self-painted geometry. Adding a
  global hit map, a global mouse router, or a shell method that re-resolves a
  coordinate a component painted is a violation regardless of where the code
  lives.
- Eligibility is the arbitration. A component that should not act on a mouse
  event is *unsubscribed*, not left to emit a message the fold discards. No
  per-component "am I blocked" flag.
- Mouse eligibility follows paint, never mount. A mounted-but-unpainted
  component is not mouse-eligible.
- The fold ranks nothing. It applies at most one mouse message and debug-asserts
  no more than one was produced. Two claims for one event is a geometry bug.
- All `MouseEventKind` filtering happens inside `on()`. Nothing depends on the
  subscription clause dropping event kinds.
- Components emit semantic `Msg` with resolved targets (pill index, slot-id
  action, transport control, child-returned row identity), never raw `col`/`row`
  for the shell to re-resolve. The context-menu anchor — a click position that
  is display geometry the component owns — is not an exception.
- Gesture timing (double-click window, wheel throttle) lives in a mounted
  parent's private `MouseGestureState`, never in a shell-side clock keyed by
  screen position.
- `tuirealm` stays pinned at `4.1` for this change; any bump re-verifies that
  `MouseEventClause::is_in_range` still ignores `kind` and `modifiers` before
  merge.

## Considered Options

- **`ratatui-interact` as a dependency.** Rejected — the adaptation surface
  (bridging two focus models, two event-result models) is larger than the code
  it saves, and it couples a core interaction path to an unproven external
  crate. TuiRealm 4.1 already supplies delivery; the house layer is ~250 lines.
- **Per-component "am I blocked" flag.** Spreads the blocking rule across ~10
  components, re-introduces shared mutable input state, and every new component
  can forget it.
- **`SubClause::HasAttrValue` self-reported paint flag.** Gates before `on()`
  correctly, but a component can only know it *did* paint, never that it
  *didn't*; making the flag go false needs a shell-written frame generation read
  back by the component, which the presentation-authority rule forbids.
- **`Application::lock_subs()`.** Global — silences the `UiRoot` keyboard router
  and breaks ADR 0023.
- **Rank the folded messages by a hand-maintained `Msg`-variant→surface table.**
  Does not solve the mutation problem (it already happened), and the table is a
  second source of truth for z-order that drifts from `OVERLAY_IDS`.
- **Keep gesture timing in `App`'s shell-side clock; shell coalesces
  single-vs-double.** Today's pattern; rejected because it does not scale to
  drag/hover — each new gesture needs new hit-region variants and new
  `ShellRequest` plumbing on every surface.
- **Dynamic per-frame position-scoped subscriptions.** Rejected — geometry
  changes every frame, so subscriptions would be torn down and rebuilt every
  frame.

## Consequences

- This ADR parallels ADR 0023 for the pointer: ADR 0023 makes keyboard
  precedence a function (`UiRoot` router) rather than an emergent property of
  fan-out delivery; this ADR makes mouse arbitration a function of the
  subscription table rather than a post-hoc fold. Both sit on the same
  `Application::tick` two-message fold in `shell_run.rs`, and neither changes the
  other — keyboard routing and the ADR 0023 fold are untouched.
- ADR 0022's completion bar is unaffected: mouse parity is added to existing
  ledger rows and does not re-open any row's migration status. ADR 0022 Residual
  A is closed and struck; Residuals B (#643) and C remain.
- The `interactive-component-framework` spec gains "mounted **and** painted in
  the most recent frame" as the mouse-delivery precondition.
- Drag-and-drop and hover become additive: `MouseGestureState` starts emitting
  the reserved `Drag*`/`Hover*` variants and the component adds `Msg` variants
  and transient local state — no change to `mouse_sub()`, the eligibility rule,
  or the fold.
- The `is_in_range` kind/modifier-ignoring behaviour is a pinned-version
  assumption; a `tuirealm` bump is a gated event that must re-verify it.

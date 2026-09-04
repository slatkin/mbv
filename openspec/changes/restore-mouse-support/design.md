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
  enum per surface. `feeds`, `audiobookshelf_book`, `audiobookshelf_podcast`,
  and `home` additionally self-gate their handler with
  `if !self.focused { return None }` (`feeds.rs:322`, `abs_book.rs:214`,
  `abs_podcast.rs:204`, `home.rs:385`) — verified against the tree: only
  `feeds.rs:322` is in a `handle_mouse`; the other three sit in `handle_key` /
  `handle_crossterm_key` (keyboard), and the three components' mouse paths
  already carry no focus check after the Phase 2 migrations — so, e.g., Feeds
  already scrolls its own
  list on the wheel *while focused*; what is missing there is not row wiring but
  the focus gate.
- **Stubs / dead code.** `handle_mouse_scroll_browse(_delta)` is an empty match.
  `PlaybackComponent` is mounted with `vec![]` and never activated, so its
  `mouse()` handler never runs.
- **Overlay/popup surfaces already handling mouse.** `help.rs:159`,
  `sessions.rs:196`, `settings.rs:388`, `playlists.rs:302`,
  `inline_search.rs:249`, plus `context_menu`, `selection_modal`, `confirm`,
  `daemon_lost`, `remote_reanchor`.
- **Mount lifetime.** `shell_destination_mounts.rs` keeps a `Browser` /
  `TvWorkspace` / `InlineSearch` mounted for *every live library*; a mount is
  retired only when its library leaves the catalog. Only the active destination
  is painted (`shell_library.rs:23` `sync_active_destination`). "Mounted" is
  therefore not "visible".

**TuiRealm 4.1 facts this design depends on** (verified against
`tuirealm-4.1.0/src/core/{application,subscription}.rs`, workspace pin
`tuirealm = "4.1"`, lockfile `4.1.0`):

- `MouseEventClause` fields are `kind: MouseEventKind`, `modifiers`,
  `column: Range<u16>`, `row: Range<u16>` — **half-open `Range`, not
  `RangeInclusive`**.
- `MouseEventClause::is_in_range` compares **only `column` and `row`**; it
  ignores `kind` and `modifiers` despite the `EventClause::forward` doc comment
  claiming "everything must match". Kind filtering must happen in the component.
- `Application::tick` returns `Vec<Msg>` with **no component identity**: the
  focused component's message first (`forward_to_active_component`), then
  subscriber messages (`forward_to_subscriptions`).
- `forward_to_subscriptions` **skips the focused component**, so a component
  that is both focused and subscribed receives an event exactly once.
- Both delivery paths call `view.forward(id, ev)`, i.e.
  `component.on(&mut self, ev)`. **The component mutates itself before it
  returns a `Msg`.** Discarding the returned `Msg` does not undo the mutation.
  Confirmed mutating mouse handlers: `feeds.rs:341`,
  `audiobookshelf_book.rs:326`, `music_workspace.rs:518`.
- `Application::subscribe(id, sub)` / `unsubscribe(id, clause)` exist and can be
  called at any time after mount; `EventClause` is `PartialEq`, so
  `unsubscribe` matches the identical clause value.
- `SubClause` supports `Always`, `IsMounted`, `HasAttrValue`, `HasState`, and
  the boolean combinators, evaluated **before** `on()` is called.
- `lock_subs()` is global: it would also silence the `UiRoot` keyboard observer
  and break ADR 0023. It is not usable here.
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
`Drag(MouseButton)` and `Moved`. The house layer is `HitRegions<Tag>` (for
irregular chrome only, D6) + `MouseGestureState` + a `resolve_point` method on
each canonical list control, ~250 lines, and `ratatui-interact`'s registry is
independent confirmation the shape is right for the irregular case. We lift the
pattern, not the crate.

**Alternative considered:** take `ratatui-interact` as a dependency and adapt.
Rejected — the adaptation surface (bridging two focus models, two event-result
models) is larger than the code it saves, and it couples a core interaction path
to an unproven external crate.

### D1 — Delivery: mouse-eligible components subscribe and recognize mouse

A mouse-eligible component subscribes with

```rust
Sub::new(
    EventClause::Mouse(MouseEventClause {
        kind: MouseEventKind::Moved,          // ignored by is_in_range; see below
        modifiers: KeyModifiers::NONE,        // ignored by is_in_range; see below
        column: 0..u16::MAX,
        row: 0..u16::MAX,
    }),
    SubClause::Always,
)
```

behind a `mouse_sub()` helper, so it receives every mouse event at any
coordinate. Note the half-open `Range<u16>` (`0..u16::MAX`) — the earlier
`0..=u16::MAX` in this design did not type-check.

The `kind` and `modifiers` fields must be given *some* value but are **not
matched**: `MouseEventClause::is_in_range` tests column and row only. That is
an upstream behaviour we depend on, not a documented contract — the crate's own
doc comment claims otherwise. Consequences, recorded as a **pinned-dependency
dependency**:

- `tuirealm` stays pinned at `4.1` for this change. Any bump re-verifies
  `is_in_range` before merge; if a later version honours `kind`, `mouse_sub()`
  becomes one subscription per kind we care about (or `EventClause::Any` with an
  in-`on()` filter).
- **All kind filtering happens inside the component's `on()`**, never in the
  clause. Nothing in this design may rely on the clause dropping event kinds.

The mounted parent owns the `MouseGestureState` and maps recognized gestures to
its controls. An embedded canonical media-list control resolves a point inside
the painted list rectangle to a stable `Target` (D6). Parent-owned pills, Queue
scope buttons, overlays, Playback seekbar, and non-list wheel/chrome retain
their own hit geometry.

**Alternative considered:** dynamic position-scoped subscriptions, re-registered
each frame to match current geometry. Rejected — geometry changes every frame
(resize, breakpoint, scroll), so this means tearing down and rebuilding
subscriptions per frame. The any-position sub + in-`on()` hit-test is what the
five landed surfaces already do informally. (Note: D2's eligibility churn is a
different thing — it is coarse and changes only when the *visible set* changes,
not when geometry moves.)

### D2 — Arbitration is decided by *who is subscribed*, not by a post-hoc fold

**The problem this decision exists to solve (S1).** TuiRealm forwards an event
by calling `component.on(&mut self, …)`. The component mutates its own state
*before* it returns a `Msg`. A shell-side fold that discards a loser's `Msg`
therefore does **not** undo the loser's mutation: a click outside a modal would
still move the underlying Feeds cursor (`feeds.rs:341`), and a wheel event over
a mounted-but-unpainted Browser would still scroll it. The earlier version of
this design asserted "the fold means components never need an 'am I blocked'
flag" — that premise is false, and every guarantee built on it was unachievable.

**Chosen: shell-owned mouse-subscription eligibility.** The shell maintains the
set of mouse-eligible components and adds/removes the `mouse_sub()`
subscription as that set changes, in a `sync_mouse_subscriptions()` that runs
in the same synchronisation pass as, and off the same derivation as,
`sync_active_destination()` (`shell_library.rs:23`, `library_child_id()`).
Eligibility is:

1. If a **blocking** overlay/modal is mounted → **only** that overlay is
   mouse-eligible.
2. Else if a non-blocking overlay/popup that paints over panel content is
   mounted → the topmost such overlay (by `root.rs` `OVERLAY_IDS`) is eligible
   **exclusively**; underlying panels are not. The popup keeps receiving
   outside-clicks, so its own dismissal policy is unaffected.
3. Else → the components painted in the current frame: the active destination,
   Queue, Playback, and chrome. Every other mounted destination is **not**
   eligible.

Because gating happens in the subscription table, a non-eligible component's
`on()` is never called: no mutation, no message, nothing to undo. The same
single mechanism resolves all three problems the previous design handled
separately — blocking-overlay swallow, overlay-over-panel precedence, and
mounted-but-not-painted destinations (D11).

**The fold survives, with a smaller job.** `shell_run.rs` still folds the
mouse-derived messages out of a `tick()` beside the keyboard router fold: it
applies at most one (first wins in `tick()` order) and, in debug builds,
asserts that at most one was produced. It does **not** rank surfaces. This
resolves S2 (`Application::tick` returns an anonymous `Vec<Msg>` with no
component identity) by removing the need for identity: eligible components paint
disjoint rectangles and each emits only for points inside its own painted
geometry, so two mouse messages for one event is a geometry bug, and the debug
assertion is how we find it. No `Msg`-variant→rank table, and no origin
`ComponentId` threaded through every mouse message.

**One residual bypass, deliberately accepted.** `forward_to_active_component`
delivers to the focused component regardless of subscriptions. This is safe
because the focused component is always painted, and a blocking overlay always
takes focus (`overlay_holds_focus()` gates `sync_active_destination`). Phase 1
lands a test asserting exactly that invariant, so a future change that mounts a
blocking overlay without taking focus fails loudly rather than leaking clicks.

**Alternatives rejected:**

- *Per-component "am I blocked" flag.* Spreads the blocking rule across ~10
  components, re-introduces shared mutable input state, and every new component
  can forget it. Rejected. (The previous design rejected this too, but on the
  false premise that the fold made it unnecessary.)
- *`SubClause::HasAttrValue` self-reported paint flag.* Would gate before
  `on()` correctly, but a component can only know that it *did* paint, never
  that it *didn't*; making the flag go false needs a frame generation handed in
  by the shell and read back out — shell-written state read back by a component,
  which the `interactive-component-framework` presentation-authority rule
  forbids. Rejected.
- *`Application::lock_subs()`.* Global: it silences the `UiRoot` keyboard
  observer too and breaks ADR 0023. Rejected.
- *Rank the folded messages by a hand-maintained `Msg`-variant → surface table.*
  Does not solve S1 at all (mutation already happened), and the table is a
  second source of truth for z-order that drifts from `OVERLAY_IDS`. Rejected.

### D3 — Gesture recognition moves into mounted parents (the A/B fork, resolved B)

**B (chosen):** each mounted destination parent owns a private
`MouseGestureState`. It feeds raw `MouseEvent`s in and gets a recognized gesture out — `Click`,
`DoubleClick`, `RightClick`, `Scroll` now; `DragStart/Move/End`,
`HoverEnter/Leave` reserved. The mounted parent maps the gesture to a semantic
request, delegating canonical list point resolution to the embedded control's
`resolve_point` (D6). The double-click interval
and wheel throttle live in `MouseGestureState`, per mounted parent.

**A (rejected):** keep all timing in `App`'s shell-side clock; components report
raw hit regions; the shell coalesces single-vs-double and routes the resolved
gesture back per surface. This is today's pattern. Rejected because it does not
scale to the planned drag/hover work: a drag is inherently a component-local
interaction (you drag within one surface's rows), and a hover highlight is
component-local visual state. Under A, every new gesture needs new `*HitRegion`
variants and new `ShellRequest` plumbing on every surface; under B it is a
`MouseGestureState` output variant plus one mounted parent's `Msg`.

**Reconciling with D16's "do not reintroduce a second clock":** D16 forbade the
*global* completed-frame hit map and a clock keyed by screen position shared
across surfaces. A `MouseGestureState` owned by one mounted parent, keyed by
nothing but that parent's own recent events, is neither — it is the same shape as
a component's existing private cursor/scroll state. The design.md of
`migrate-tui-to-tuirealm` D16 explicitly says restoration should be "the same
way the five landed units did — component hit-tests its own geometry, emits a
typed `Msg::Shell`"; B extends that (the mounted parent is that component), A
freezes it mid-way.

### D4 — Components emit semantic `Msg`, never raw coordinates

A mounted parent emits a mouse `Msg` with a resolved semantic target — a pill
index, `QueueSlotId` scope action, transport control, or child-returned row
identity — not `col`/`row` for the shell to re-resolve. Canonical list row
identity is resolved only by the embedded child. This is the `interactive-component-framework` "own only
presentation authority" rule (no shell field written to be read back) applied to
mouse. The one apparent exception, the context-menu anchor, is a click position
that is display geometry the component legitimately owns and forwards; it is not
a target the shell re-resolves.

### D5 — Focus-follows-click via the component's claim

A panel component that claims a click emits (or the shell infers from the claim)
a `set_panel_focus` for that panel before the semantic effect runs — exactly how
`handle_mouse_single_click_emby` already calls `self.set_panel_focus(...)`. The
shell applies focus first so a subsequent effect sees the right focused surface.

### D6 — Uniform lists resolve a point arithmetically; a rect registry is for irregular chrome

The previous version of this decision had it backwards. `WideMediaList` and
`InlineMediaBrowser` are **pure state/geometry models with no `view()` and no
painting** — "clear it at the top of `view()`, `push(rect, tag)` as it paints"
describes methods they do not have and would have to grow. Their row flow is
uniform and *already exported*: `WideMediaList::row_geometry(viewport_height)`
and `InlineMediaBrowser::row_geometry(viewport_height, detail_rows)` return a
`RowGeometry<Target>` with `offset()` and `targets()`. Row → target is
`(point.y - list_area.y) as usize + offset` plus a lookup. A rect registry buys
nothing there and adds a per-row `Vec<Rect>` rebuilt every frame.

**Chosen split:**

- **Uniform canonical lists** get a small method, not a registry:

  ```rust
  // WideMediaList<Target>
  pub fn resolve_point(&self, list_area: Rect, point: Position) -> Option<&Target>
  // InlineMediaBrowser<Target> — same, plus the detail_rows the parent painted
  pub fn resolve_point(&self, list_area: Rect, detail_rows: usize, point: Position)
      -> Option<&Target>
  ```

  ~15 lines each, built on the existing `row_geometry`. The parent passes the
  rectangle *it* painted and the detail-row count *it* painted, so the control
  never reads geometry it did not receive from its painter (ADR 0022 authority
  bar) and there is no second copy of the row flow to drift from the painted one.

- **Irregular parent chrome** — pills, Queue scope buttons, transport buttons,
  seekbar, group selectors, overlay/popup rows — gets `HitRegions<Tag>` in new
  `src/app/components/mouse/hit.rs` (`clear`, `push(rect, tag)`,
  `resolve(point) -> Option<&Tag>`, last-push-wins). This is not new shape: it
  formalises the hand-rolled `Vec<(Rect, Target)>` these components already
  keep (e.g. `feeds.rs` `layout.selector_tabs`). Each such component clears and
  repopulates it in the same code that paints those rects.

This change still owns the full per-surface row-hit migration: for every surface
(Queue included) the bespoke `*HitRegion` row enum is replaced by the embedded
control's `resolve_point` and the enum is deleted. Nothing is deferred to
another change. Parent chrome target types and overlay/popup target enums MAY
remain as their own types, held in `HitRegions<Tag>`.

**Alternative rejected:** `HitRegions<Target>` on the canonical controls as
originally written. It requires giving the two controls a paint-time hook they
do not have, stores a `Vec<Rect>` that is a derived duplicate of
`row_geometry`, and would have to be invalidated on every scroll/resize —
exactly the "store the same fact twice" failure mode. Rejected.

### D7 — Delivery model chosen so drag/hover are additive

Turning on drag/hover later means: (1) `MouseGestureState` starts emitting the
reserved `Drag*` / `Hover*` variants (it already receives `Drag(_)` and `Moved`
events — crossterm 0.29 sends them, capture is already enabled), (2) the
component adds `Msg` variants and transient local drag/hover state. No change to
`mouse_sub()`, the eligibility rule, or the fold.

Hover-move spam is dropped **in the component**, as the first match arm of its
`on()` — `MouseEventKind::Moved => return None` for components that do not want
hover. A `MouseEventClause` kind filter is not available: `is_in_range` ignores
`kind` in tuirealm 4.1 (D1). Nothing in this design may be sourced to that
filter.

### D8 — `mouse_gestures.rs` keeps effects; recognition and geometry leave

The `impl App` effect handlers in `mouse_gestures.rs`
(`handle_mouse_single_click_emby`, the queue/tv handlers) stay — they are
shell-owned effects keyed off typed `Msg`. What leaves is:

- the recognition glue (`note_browse_double_click`, `note_browse_scroll`) and
  the stubbed `handle_mouse_scroll_browse`;
- **`seek_to_col(col: u16)` → `seek_to_fraction(fraction: f64)`**, and
  `PlaybackRequest::SeekTo(u16)` → `SeekTo(f64)`. Today `playback.rs:147` emits
  a raw terminal column and `mouse_gestures.rs:27` re-resolves it against
  `self.layout.playback.seekbar_area` — the shell reading geometry another
  component painted. `PlaybackComponent` already owns `seekbar_area` from its
  own `view()` (`playback.rs:193`), so it resolves the fraction itself and the
  shell handler takes a resolved value. This is D4 applied to the one place that
  violated it, and it deletes ADR 0022 Residual A's second site.
- **`browse_mouse_ready` / `is_browse_layout_current`** (`mouse_gestures.rs:13`),
  a shell-side "is the painted layout current for this tab?" gate reading
  `self.layout.main.browse_destination`. Its only callers are
  `handle_mouse_scroll_browse` (a stub, deleted) and `shell_home.rs:125`
  (Home wheel, which becomes component-owned recognition). Both go, so the read
  goes with them — Residual A's first site. `normalize_stale_browse_destination`
  stays; it has an unrelated caller at `cw_library_tab_actions.rs:43`.

Parent wheel handling remains for non-list chrome while canonical list scrolling
is returned by the embedded control.

### D9 — An ADR records the delivery model

A new `docs/adr/0024-mouse-events-through-component-subscriptions.md` fixes
D1–D3, D11 (mouse eligibility follows paint; that is not Residual debt) and D13
(Residual A closed) as architecture (accepted), so a later contributor cannot
re-add a global hit map, or gate mouse on mount rather than paint, without
superseding it. It records the tuirealm 4.1 `is_in_range` dependency from D1 as a
pinned-version assumption. The same change strikes Residual A from
`docs/adr/0022-…md`. The `interactive-surface-ledger.md` "Mouse ownership is out
of scope" section is replaced by a Mouse column populated per row.

### D11 — Mouse resolves against the last painted frame; that is not ADR 0022 debt

A mouse event is a coordinate on the frame the user is looking at. Resolving it
against anything other than the most recently painted geometry is the drift bug,
not the fix. This design therefore states explicitly what ADR 0022's authority
bar does and does not forbid:

- **Forbidden (and closed by D8):** a *keyboard or behaviour* path branching on
  what was painted, and any component reading geometry it did not itself paint.
- **Required here:** a *pointer* path resolving a point against the geometry the
  resolving component itself painted in the most recent frame, and the shell
  making a component mouse-eligible only while it is painted (D2).

Concretely, "mounted" is not "visible": `shell_destination_mounts.rs` keeps a
`Browser` / `TvWorkspace` / `InlineSearch` mounted for every live library, and
only the active one is painted. Without D2's eligibility gate, a wheel event
over the visible Emby list would also scroll three invisible Browsers — mutation
that no fold can undo (S1). The spec is amended accordingly: a component
receives mouse events while it is mounted **and painted in the most recent
frame**.

The eligibility set is derived from the shell's existing active-destination
derivation, not from a second paint ledger, so there is no new "did I paint"
fact stored anywhere.

### D12 — The `if !self.focused` mouse guards come out in Phase 3, not Phase 1

`feeds.rs:322` (`FeedsComponent::handle_mouse`) returns `None` from its mouse
handler unless focused. Removing it is the visible payoff of this change *and*
an observable behaviour change, which Phase 1's "delivery only, no observable
change" gate forbids. It comes out in Phase 3 (task 4.2), behind that phase's
live-review gate.

The original four-guard list also named `audiobookshelf_book.rs:214`,
`audiobookshelf_podcast.rs:204`, and `home.rs:385` — verified against the tree
before implementation, those three sit in `handle_key` /
`handle_crossterm_key`, i.e. they are **keyboard** guards, not mouse. The mouse
paths of those same components (`AudiobookshelfBookComponent::handle_mouse`,
`AudiobookshelfPodcastComponent::handle_mouse`, `HomeComponent::handle_mouse`)
already carry no focus check after the Phase 2 migrations, so 4.2 is a
one-guard deletion plus per-surface verification that the already-unfocused
mouse paths act from any focus state.

This makes Phase 1's own verification awkward — a delivery test aimed at a
guarded component would assert `None` and prove nothing. Phase 1 therefore
verifies delivery through `PlaybackComponent`, which has no focus guard, never
holds focus, and is the phase's one intended observable win: an injected click
on the seekbar while another component is focused must produce
`Msg::Playback(PlaybackRequest::SeekTo(_))` out of a real `tick()`. Asserting an
emitted `Msg` — not merely that `on()` ran — is what makes the test meaningful.

### D13 — ADR 0022 Residual A closes in this change

Residual A ("chrome hit geometry", `docs/adr/0022-…md:65-67`) is assigned to
#638 and has three sites:

| Site | Closure |
| --- | --- |
| `mouse_gestures.rs:27` (`seek_to_col` reads `layout.playback.seekbar_area`) | D8 — `SeekTo` carries a fraction resolved by `PlaybackComponent`. |
| `mouse_gestures.rs:13` (`is_browse_layout_current` reads `layout.main.browse_destination`) | D8 — both callers are deleted, so the gate is deleted. |
| `input.rs:97` (`ensure_tab_visible` reads `layout.tabs_area.width` on a **keyboard** path) | The tab-bar width is derived by the chrome arrangement (`render/arrangements/chrome.rs:113`) from terminal width and right-panel visibility. That derivation becomes a pure arrangement function called by both the painter and `ensure_tab_visible`, so the keyboard path stops reading the painted rect. |

The third is a shared layout invariant, so it lands in the arrangement layer
(one primitive both callers use), not as a local fix in `input.rs`. The change
is not complete until Residual A is struck from ADR 0022 and its "Known
deviations" list names only Residuals B and C.

### D10 — Phasing

One change, six phases, each independently shippable with its own gate (detailed
in `tasks.md`): 1 delivery spine + eligibility + Residual A · 2 shared
primitives · 3 main-surface parity + wheel + focus-guard removal · 4 overlays,
popups, and the five already-handling surfaces (`help`, `sessions`, `settings`,
`playlists`, `inline_search`) · 5 music_workspace narrow + narrow browse
(**net-new mouse surfaces**, see below) · 6 ledger + precedence-proof close-out.
Phase 1 must land subscriptions, eligibility, and the fold with no new gestures
(D12), so regressions are isolated to delivery.

**Phase 5 is new work, not migration.** `music_workspace`'s narrow list and
track table have no `*HitRegion` enum and no narrow mouse path;
`src/app/components/browser/` (`content`, `keyboard`, `navigation`, `paint`,
`mod`) has **no narrow mouse handling at all** — there is nothing to keep and no
enum to delete. Both surfaces are designed and built here from the primitives,
and are budgeted as such.

**Both breakpoints are verified.** Narrow is where geometry differs most and
where two of these surfaces exist only at one breakpoint, so every phase gate
from 3 onward verifies wide *and* narrow.

**Continuous verification and acceptance (rule).** Each phase's implementation,
representative tests and automated gate, review, and acceptance form one
uninterrupted slice without a pre-test visual-approval checkpoint. Any phase
that changes observable pointer, cursor, or rendered-UI behaviour — Phases 3,
4, 5, 6, and any observable part of Phase 2 — receives live review after its
automated gate and before phase acceptance. A defect found there is fixed as a
bug and the affected tests and gate are rerun. Each affected phase gate in
`tasks.md` references this rule.

## Risks / Trade-offs

- **Every eligible component hit-testing every mouse event (esp. once hover
  lands)** → N is small (D2 keeps only painted surfaces eligible; ~5), and a
  hit-test is a rect check. Hover-move spam is dropped by an early
  `MouseEventKind::Moved => None` arm in each component that opts out (D7); the
  clause-level kind filter does not exist in tuirealm 4.1 and nothing here
  depends on it.
- **A component is eligible while unpainted, or ineligible while painted** →
  this is now the single failure mode that produces click-through or dead
  clicks, so it is where the tests go: Phase 1 asserts the eligibility set
  matches the painted set across a destination switch, a breakpoint change, and
  overlay mount/unmount, and Phase 6's precedence proofs drive it through the
  real `tick()`.
- **The focused component bypasses eligibility** (`forward_to_active_component`)
  → accepted, guarded by a Phase 1 test asserting that a mounted blocking
  overlay always holds focus (D2).
- **Subscription churn on destination switch** → coarse and rare (once per
  visible-set change, not per frame), two `Application::unsubscribe` /
  `subscribe` calls; measured against `PollStrategy::Once(50ms)` it is noise.
  Contrast the rejected per-frame geometry-scoped subscriptions (D1).
- **Phase 2 refactor of the five landed surfaces regresses working clicks** →
  Phase 2 is a representation-only path swap: the existing per-surface mouse
  tests are ported to `resolve_point(list_area, point) -> Option<&Target>` and
  stay green, and characterization buffers are unchanged (hit geometry is never
  painted).
- **A `tuirealm` bump silently breaks `mouse_sub()`** → `is_in_range`'s
  kind/modifier-ignoring behaviour contradicts its own doc comment, so it could
  be "fixed" upstream. The pin stays at `4.1` for this change and a bump
  re-verifies it (D1); the Phase 1 clause unit test fails loudly if delivery
  narrows.
- **B (D3) is more upfront work than A** → accepted deliberately; A's cost is
  paid back with interest on the first drag-and-drop surface (proposal — Why).
- **Spec deltas touch three large migration requirements** → the deltas
  reproduce each requirement in full (archive-safe) and change only the mouse
  carve-out lines; no other behaviour in those requirements moves.

## Migration Plan

`restore-mouse-support` is the final change on `feat/migrate-tui-to-tuirealm`. It
depends on all five `compose-canonical-media-lists` slices being merged; it is
not revised as a prerequisite by that campaign and is not a dependency of any
slice. PR #606 merges only after this change lands. It archives with its own
umbrella (#603).

Additive within one branch; each phase is a commit group that leaves the app
runnable. No runtime data migration. Rollback = revert the phase commit; Phase 1
is the only one whose revert restores the "mouse only reaches focused component"
behaviour, and it carries the delivery tests that would catch a regression before
merge.

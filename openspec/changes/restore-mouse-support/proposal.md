> Tracking issue: [#638](https://github.com/slatkin/mbv/issues/638). Umbrella: #603.

## Why

The TuiRealm migration deleted the legacy global mouse framework (`input_mouse*.rs`,
the completed-frame hit map, the global router) under design decision **D16** and
accepted mouse interaction as broken for the alpha build. What survives is
half-connected: five surfaces (`browser`, `home`, `queue`, `tv_workspace`, and
partially `music_workspace`) hit-test their own painted geometry and emit typed
requests, but TuiRealm delivers `Event::Mouse` only to the **focused** component,
`UiRoot`'s observer drops every mouse event on the floor (`shell.rs:349`), and
`PlaybackComponent` — which carries a full `mouse()` handler for the seekbar and
transport buttons — is never made active, so its handler is unreachable. Wheel
scrolling on Emby/ABS/Feeds browse lists is a stubbed no-op
(`handle_mouse_scroll_browse(_delta)`). Clicking any surface that does not
currently hold keyboard focus does nothing.

The rest of the migration is nearly complete and this is the last major piece of
deferred functionality. It needs the same care the migration itself got — an ADR,
lettered design decisions, a per-surface ledger, and phased verification gates —
because mouse is about to become a primary interaction surface, not an
afterthought: drag-and-drop and hover affordances are planned once this lands, and
the framework must absorb them additively.

## What Changes

- **New mouse delivery spine.** Every mounted destination parent subscribes to
  mouse events through TuiRealm's `EventClause::Mouse` subscription clause and
  hit-tests the non-list chrome it last painted, delegating canonical list points
  to its embedded control. Each parent-produced mouse message crosses the
  component boundary in a runtime-only typed envelope carrying its originating
  mounted surface/source tag and semantic message. A shell-side arbitration fold
  (parallel to the existing keyboard router fold) resolves overlapping claims with
  a fixed priority: topmost overlay > active panel > sibling panel > chrome. A
  mounted blocking overlay discards all mouse messages from underlying components;
  the envelope is unwrapped only after the winner is selected.
- **Shared framework primitives.** A `HitRegions<Tag>` collector (fill painted
  rects + tags during `view()`, resolve a point to a `Tag` during `on()`) for
  embedded canonical list controls, and a mounted-parent `MouseGestureState`
  recognizer (raw events in; `Click`,
  `DoubleClick`, `RightClick`, `Scroll` out — `DragStart/Move/End` and
  `HoverEnter/Leave` reserved). These replace the scattered `note_browse_*`
  helpers, and — for canonical list row hits only — the per-surface row-hit
  `*HitRegion` enums in `msg/hit_regions.rs` (removed by the canonical media-list
  slices as they migrate each destination). Parent chrome target types (pills,
  Queue scope buttons, seekbar/transport) and overlay/popup target enums MAY
  remain as their own types.
- **Gesture recognition moves into mounted parents.** The double-click window and
  scroll throttle move off `App`'s shell-side clock into each mounted parent's
  `MouseGestureState`. This is not the global position-keyed clock D16 forbade;
  design.md records why they differ, and why the alternative (keep timing
  shell-side) was rejected as unscalable for drag/hover.
- **Parent-owned mouse parity.** Every mounted destination parent recognizes
  gestures. Parent-owned pills, Queue scope buttons, overlays, Playback seekbar,
  and non-list wheel/chrome retain their own geometry. Embedded canonical
  media-list controls own view-populated `HitRegions<Target>` for painted list
  rectangles; parents delegate point resolution. Queue/list row-hit migration
  belongs to the canonical media-list slices, with no duplicate coordinate path
  delivered here.
- **`PlaybackComponent` mouse reachable.** Seekbar scrub and transport-button
  clicks work regardless of which component holds focus.
- **Browse wheel ownership.** Parent-owned non-list wheel/chrome behavior is
  restored here; Emby/ABS/Feeds canonical list scrolling belongs to the
  canonical media-list slices and is reached through parent gesture delivery.
- **Ledger and gates.** `docs/architecture/interactive-surface-ledger.md` gains a
  Mouse ownership/verification column replacing its "Mouse ownership is out of
  scope" section. The three deferred D16 precedence proofs (simultaneous
  Queue+Library mouse, blocking-overlay swallow, geometry-cannot-drift) land as
  tests.
- **No new dependency.** `ratatui-interact` was evaluated and rejected (ratatui-
  native, collides with TuiRealm's focus/subscription/`Msg` model, unproven
  maintenance, unwanted widget set). TuiRealm 4.1 already delivers `Event::Mouse`
  and models `Drag`/`Moved`; the house layer is ~300 lines.
- **Out of scope (deliberately, not precluded):** drag-and-drop, hover
  highlighting, hover previews, `MouseEventKind::Moved`/`Drag` handling. Phase 1's
  delivery model is chosen so enabling them later is additive.

## Capabilities

### New Capabilities
- `mouse-input`: how raw terminal mouse events are delivered to interactive
  components, how overlapping hit claims are arbitrated, how gestures are
  recognized, and the per-surface parity contract every migrated interactive
  surface must meet.

### Modified Capabilities
- `interactive-component-framework`: the "Migration preserves existing contracts"
  scenario currently limits mouse parity to "the alpha-supported mouse paths"; it
  changes to require full mouse parity as defined by `mouse-input`. The
  deliberately-inert request-arm rationale drops "mouse-only under D16" as a
  standing reason. A scenario is added for mouse delivery through a live `tick()`
  to subscribed non-focused components.
- `ui-design-system`: the "Screens use canonical UI ownership boundaries"
  requirement scopes hit-target ownership to "mouse paths supported by the alpha
  migration"; it broadens to all mouse paths. The "Deferred mouse support is
  restored later" scenario is satisfied by this change.
- `context-menu`: "Mouse-triggered menu" currently fires on a limited set of
  "supported" items; it broadens to every migrated interactive surface that paints
  a selectable row.

## Impact

- **Code:** `src/app/shell_run.rs` (mouse fold in the tick loop), `src/app/shell.rs`
  (component mount sites gain mouse subscriptions; `TerminalObserverEvent::Mouse`
  retired), `src/app/shell_messages.rs` / `src/app/shell_playback.rs` (mouse
  request handlers), `src/app/mouse_gestures.rs` (effects retained, recognition
  removed), new `src/app/components/mouse/` module, and `Event::Mouse` arms across
  every component under `src/app/components/`.
- **Docs:** `docs/architecture/interactive-surface-ledger.md` (new column, section
  replaced); a new or extended ADR fixing the delivery model.
- **Dependencies:** none added. TuiRealm 4.1 and crossterm 0.29 already present.
- **Tests:** new integration coverage for cross-panel routing and overlay swallow;
  per-component mouse tests; the three D16 precedence proofs.
- **No BREAKING changes** to keyboard, rendering, or protocol surfaces.

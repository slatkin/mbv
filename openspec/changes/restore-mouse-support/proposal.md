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
transport buttons — is never made active, so its handler is unreachable. Four
components additionally self-gate their handler with
`if !self.focused { return None }` (`feeds.rs:322`, `audiobookshelf_book.rs:214`,
`audiobookshelf_podcast.rs:204`, `home.rs:385`) — Feeds, for instance, already
scrolls its own list on the wheel *while focused* (`feeds.rs:327`), so restoring
it is deleting a focus gate, not wiring rows. The shell's cross-surface browse
wheel (`handle_mouse_scroll_browse(_delta)`) is a stubbed no-op. Clicking any
surface that does not currently hold keyboard focus does nothing. Meanwhile the
shell still re-resolves a raw terminal column against the seekbar rectangle
another component painted (`mouse_gestures.rs:27`) — one of the three sites of
ADR 0022 **Residual A**, which is assigned to this issue and closed here.

The rest of the migration is nearly complete and this is the last major piece of
deferred functionality. It needs the same care the migration itself got — an ADR,
lettered design decisions, a per-surface ledger, and phased verification gates —
because mouse is about to become a primary interaction surface, not an
afterthought: drag-and-drop and hover affordances are planned once this lands, and
the framework must absorb them additively.

## What Changes

- **Final change on the feature branch.** This is the FINAL change on
  `feat/migrate-tui-to-tuirealm`. It depends on all five
  `compose-canonical-media-lists` slices being merged. PR #606 merges only after
  this lands. It is not revised as a prerequisite by that campaign and is not a
  dependency of any canonical slice.
- **New mouse delivery spine.** A component that is mounted **and painted in the
  most recent frame** subscribes to mouse events through TuiRealm's
  `EventClause::Mouse` subscription clause and hit-tests the non-list chrome it
  last painted, delegating canonical list points to its embedded control.
- **Arbitration by eligibility, not by a post-hoc fold.** TuiRealm calls
  `component.on(&mut self, …)`, so a component mutates itself *before* it
  returns a message — discarding a loser's message cannot undo its mutation. The
  shell therefore decides *who is subscribed*: while a blocking overlay is
  mounted only that overlay is mouse-eligible; while a popup paints over panels
  only the topmost such overlay is; otherwise only the painted surfaces are. A
  mounted-but-unpainted destination (the shell keeps one Browser mounted per
  library) receives nothing. The shell-side fold parallel to the keyboard router
  fold survives, applying at most one mouse message per event and asserting in
  debug that no two eligible surfaces claimed the same point.
- **Shared framework primitives.** A `HitRegions<Tag>` collector (fill painted
  rects + tags while painting, resolve a point to a `Tag` during `on()`) for
  **irregular parent chrome** — pills, Queue scope buttons, transport, seekbar,
  group selectors, overlay rows — formalising the hand-rolled `Vec<(Rect, T)>`
  those components already keep; and a mounted-parent `MouseGestureState`
  recognizer (raw events in; `Click`, `DoubleClick`, `RightClick`, `Scroll` out —
  `DragStart/Move/End` and `HoverEnter/Leave` reserved). These replace the
  scattered `note_browse_*` helpers. The already-landed
  `WideMediaList`/`InlineMediaBrowser` instead get a small
  `resolve_point(list_area, point) -> Option<&Target>` built on their existing
  `row_geometry()` — a uniform row flow needs arithmetic, not a per-row rect
  registry rebuilt every frame. Every per-surface `*HitRegion` enum in
  `msg/hit_regions.rs` (Queue included) is migrated onto that and deleted.
  Parent chrome target types and overlay/popup target enums MAY remain as their
  own types.
- **Gesture recognition moves into mounted parents.** The double-click window and
  scroll throttle move off `App`'s shell-side clock into each mounted parent's
  `MouseGestureState`. This is not the global position-keyed clock D16 forbade;
  design.md records why they differ, and why the alternative (keep timing
  shell-side) was rejected as unscalable for drag/hover.
- **Parent-owned mouse parity.** Every mounted destination parent recognizes
  gestures. Parent-owned pills, Queue scope buttons, overlays, Playback seekbar,
  and non-list wheel/chrome retain their own geometry. Parents delegate list
  point resolution to the embedded canonical control's `resolve_point`, passing
  the rectangle they painted. Queue/list row-hit migration is owned here.
- **`PlaybackComponent` mouse reachable, and `SeekTo` carries a fraction.**
  Seekbar scrub and transport-button clicks work regardless of which component
  holds focus. `PlaybackRequest::SeekTo(u16 column)` becomes `SeekTo(f64
  fraction)` resolved by the component that painted the seekbar, so
  `App::seek_to_col` becomes `seek_to_fraction` and stops reading painted
  geometry.
- **ADR 0022 Residual A closes here.** All three sites —
  `mouse_gestures.rs:27` (seekbar column), `mouse_gestures.rs:13`
  (`is_browse_layout_current`), and `input.rs:97` (`ensure_tab_visible` reading
  the painted `tabs_area` on a *keyboard* path) — are retired, and Residual A is
  struck from `docs/adr/0022-…md`.
- **Browse wheel ownership.** Parent-owned non-list wheel/chrome behavior is
  restored here; Emby/ABS/Feeds canonical list scrolling is wired here onto the
  landed canonical controls through parent gesture delivery, and the four
  `if !self.focused` mouse guards are deleted per surface.
- **Net-new mouse surfaces.** Music's narrow list and track table, and narrow
  browse (`src/app/components/browser/`), have **no** existing mouse handling
  and no `*HitRegion` enum. They are designed and built here rather than
  migrated. The five surfaces that already handle mouse but appear in no
  migration ledger row — `help`, `sessions`, `settings`, `playlists`,
  `inline_search` — are brought onto the primitives in the overlay phase.
- **Ledger and gates.** `docs/architecture/interactive-surface-ledger.md` gains a
  Mouse ownership/gestures/breakpoints-verified/verification column replacing its
  "Mouse ownership is out of scope" section; a surface that renders at both
  breakpoints is verified at both. The three deferred D16 precedence proofs
  (simultaneous Queue+Library mouse, blocking-overlay swallow,
  geometry-cannot-drift) land as tests, with the swallow proof asserting that no
  underlying component was *mutated*, not merely that no message survived.
- **No new dependency; `tuirealm` stays pinned at 4.1.** `ratatui-interact` was
  evaluated and rejected (ratatui-native, collides with TuiRealm's
  focus/subscription/`Msg` model, unproven maintenance, unwanted widget set).
  TuiRealm 4.1 already delivers `Event::Mouse` and models `Drag`/`Moved`; the
  house layer is ~250 lines. The subscription clause depends on 4.1's
  `MouseEventClause::is_in_range` matching column/row only and ignoring `kind`
  and `modifiers` (contrary to its own doc comment), so a version bump must
  re-verify that before merge; all kind filtering lives in components.
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
  retired), `src/app/shell_library.rs` (mouse-eligibility sync beside
  `sync_active_destination`), `src/app/shell_messages.rs` /
  `src/app/shell_playback.rs` (mouse request handlers, `SeekTo` fraction),
  `src/app/shell_home.rs` (Home wheel becomes component-owned),
  `src/app/mouse_gestures.rs` (effects retained; recognition, `seek_to_col`, and
  `browse_mouse_ready` removed), `src/app/input.rs` +
  `src/app/render/arrangements/chrome.rs` (tab-bar width derived by the
  arrangement, not read off the painted rect), `src/app/components/msg/playback.rs`,
  new `src/app/components/mouse/` module, `src/app/components/media_list/{wide,inline}.rs`
  (`resolve_point`), and `Event::Mouse` arms across every component under
  `src/app/components/`.
- **Docs:** `docs/architecture/interactive-surface-ledger.md` (new column, section
  replaced); a new ADR 0024 fixing the delivery model; `docs/adr/0022-…md`
  (Residual A struck).
- **Dependencies:** none added. TuiRealm 4.1 and crossterm 0.29 already present.
- **Tests:** new integration coverage for cross-panel routing and overlay swallow;
  per-component mouse tests; the three D16 precedence proofs.
- **No BREAKING changes** to keyboard, rendering, or protocol surfaces.

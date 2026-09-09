# Repair Canonical Media-List Ownership Design

## Context

See `proposal.md` for motivation. TuiRealm 4.1 supplies
`Component::view(&mut self, &mut Frame, Rect) -> ()`; current canonical painters
instead accept parent-established panel and row-flow geometry and return row
facts. Queue reconstructs row rectangles after painting, and Grouped Music
rebuilds row maps and a track hit map from paint results. Landed PR #683 makes
the latest painted owner authoritative for mouse eligibility, so retained
geometry must describe the current paint and never an earlier frame.

This change proves a component-view seam in Queue and Grouped Music only. Both
destinations already own persistent list controls. Their current responsive
anchor, search, parent gesture, and layout ownership remain unchanged.

## Goals / Non-Goals

**Goals:**

- Preserve parent-owned panel framing, claim, and row-flow placement with no
  visual change.
- Let an embedded control paint its supplied row flow once and retain current
  frame facts so a parent does not rebuild uniform row maps or pass geometry
  back for point resolution.
- Prove the seam on Queue and Grouped Music while preserving #683 mouse
  arbitration and existing parent-owned behavior.

**Non-Goals:**

- Changing active-control ownership, responsive anchors, row target identity,
  Browser grid state, content projections, or any destination outside Queue
  and Grouped Music.
- Changing Grouped Music's album/track ownership, image behavior, Inline
  Search, parent gestures, or visible framing and spacing.
- Universal architecture rules, source ratchets, `CONTEXT.md`/ledger/comment
  sweeps, or multi-select.
- Moving Queue to Inline replacement: Queue remains a fixed-row
  `WideMediaList` in every panel mode.

## Decisions

### D1. Parents retain frame, claim, and row-flow placement

A destination parent continues to place its panel/frame, current claim
rectangle, and row-flow rectangle using its existing arrangement. Before its
embedded control views a frame, the parent configures those established claim
and row-flow rectangles; `Component::view` then paints ordinary rows once and
retains a read-only result for that frame. Queue configures equal claim and
row-flow rectangles. Grouped Music preserves its existing full-width claim
rectangle and padded row-flow rectangle. The control does not choose
destination padding, panel width, or other parent layout policy.

The retained result exposes only legitimate parent facts: current claim/content
rectangles, selected target/selected-row rectangle, and Inline admitted-detail
rectangle. `RowGeometry` remains internal. A later point-resolution call
accepts only the point and uses the retained result; it does not accept an area
or detail height. Compatibility painters keep their existing parent geometry
contract for destinations not migrated by this change.

### D2. Retained results have an explicit lifecycle

Configuring a control, starting a view, or receiving an empty/zero-area view
invalidates its prior result. A result becomes readable only after the current
view completes. Parents treat no current result as no list claim and no detail
region.

This prevents #683 from delivering an eligible pointer to stale row geometry.

### D3. Queue and Grouped Music are bounded proofs

Queue proves the fixed-row Wide path with stable `QueueSlotId` targets in every
panel mode. It keeps scope-pill and drag gesture authority while its child owns
painted row claim and selection facts.

Grouped Music proves the parent-framed Wide album rail, Wide track table, and
Normal/Narrow Inline album list. It retains its grouped content, images, Inline
Search, responsive anchor handoff, provider detail, and typed requests. Its
parent consumes only the current retained child results for uniform row
resolution; irregular pill geometry remains parent-owned.

## Risks / Trade-offs

- **[A parent reads before current view]** → focused before/no/after-view and
  empty/zero-area tests make the control report no claim or detail.
- **[Buffer parity changes while moving the painter]** → retain existing row
  primitives and prove Queue and Grouped Music representative Wide/Inline
  buffers unchanged.
- **[#683 mouse behavior changes]** → preserve real-`Application::tick()`
  arbitration, throttle, and one-row tests while routing migrated points
  through retained geometry.
- **[Music has multiple lists and distinct rectangles]** → preserve each
  parent-established claim/row-flow pair while verifying both Wide rails and the
  Inline list, including the unchanged responsive anchor handoff.

## Migration Plan

1. Confirm the PR #683 baseline tests before changing shared list code.
2. Add the semantic policy, component implementations, retained-result
   lifecycle, and compatibility behavior for untouched destinations.
3. Convert Queue, then Grouped Music's Wide album rail, Wide track table, and
   Inline album list to call each child view once and consume retained results.
4. Run focused control, Queue, Grouped Music, buffer, and live-tick tests plus
   standard package gates. A human verifies Queue and all affected Grouped
   Music presentations plus focused-sidebar wheel arbitration.

Rollback is a normal commit revert; no persisted data or protocol changes
occur.

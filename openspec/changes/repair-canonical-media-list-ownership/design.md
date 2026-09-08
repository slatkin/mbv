# Repair Canonical Media-List Ownership Design

## Context

See `proposal.md` for motivation. TuiRealm 4.1 supplies `Component::view(&mut self, &mut Frame, Rect) -> ()`; the current canonical painters instead accept caller-derived paint/content rectangles and return geometry. Queue reconstructs row rectangles after painting, and Feeds rebuilds selectable maps from a render result. Landed PR #683 makes the latest painted owner authoritative for mouse eligibility, so retained geometry must describe the current paint and never an earlier frame.

This change proves a component-view seam in Queue and Feeds only. Those destinations already own persistent list controls; their current lockstep responsive ownership is intentionally left unchanged.

## Goals / Non-Goals

**Goals:**

- Establish one rectangle source and one ordinary-row painter for an embedded canonical control.
- Let parents read the current frame's legitimate list facts without rebuilding row maps or passing geometry back into the control.
- Prove the seam on Queue and Feeds while preserving their user-visible behavior and #683 mouse contract.

**Non-Goals:**

- Changing active-control ownership, responsive anchors, row target identity, Browser grid state, content projections, or any destination outside Queue and Feeds.
- Universal architecture rules, source ratchets, `CONTEXT.md`/ledger/comment sweeps, or multi-select.
- Moving Queue to Inline replacement: Queue remains a fixed-row `WideMediaList` in every panel mode.

## Decisions

### D1. The child owns all list rectangles

`Component::view` is the only input of an outer rectangle. Before that call, the parent supplies a closed semantic paint policy: focused/selected treatment, optional throbber, and Inline desired-detail treatment. The policy carries no `Rect`, inset value, raw style, callback, provider data, or effect.

Inside `view`, the control derives its claim/content/flow rectangles, paints ordinary rows once, and retains a read-only result for that frame. The result exposes only parent facts: claim/content rectangles, selected target/selected-row rectangle, and Inline admitted-detail rectangle. `RowGeometry` remains internal. Later point resolution receives only a point and uses the retained painted result; it does not accept an area or detail-height argument.

Alternative: keep `paint_area` and `content_area` as parent inputs. Rejected because frame geometry then has multiple authorities and a hit can disagree with the current row flow.

### D2. Retained results have an explicit lifecycle

Configuring a control, starting a view, or receiving an empty/zero-area view invalidates its prior result. A result becomes readable only after the current view has completed. Parents treat no current result as no list claim and no detail region.

Alternative: retain prior geometry until replacement. Rejected because #683 would then deliver an eligible pointer to a surface using stale row geometry.

### D3. Queue and Feeds are bounded proofs

Queue proves the Wide path with stable `QueueSlotId` targets across all panel modes. It keeps scope-pill and drag gesture authority, while its child owns painted row claim/selection facts. Feeds proves Wide framed inset and Inline detail admission/fallback, retaining its selector/filter chrome, feed detail, and typed requests. This proof does not change Feeds' current lockstep responsive movement; that is a later ownership change.

Alternative: migrate every canonical destination with the seam. Rejected because destination exceptions and ownership changes obscure the protocol being proved.

## Risks / Trade-offs

- **[A parent reads before current view]** → result invalidation and focused before/no/after-view tests make the control report no claim or detail.
- **[Buffer parity changes while moving the painter]** → keep existing row primitives and prove Queue/Feeds representative Wide/Inline buffers.
- **[#683 mouse behavior changes]** → preserve its real-`Application::tick()` arbitration, throttle, and one-row tests while routing points through retained geometry.

## Migration Plan

1. Confirm the PR #683 baseline tests before changing shared list code.
2. Add the semantic policy, component implementations, and retained-result lifecycle, retaining existing row primitives beneath the controls.
3. Convert Queue, then Feeds, to call each child view once and consume only its current result; remove their redundant map reconstruction.
4. Run focused control, Queue, Feeds, buffer, and live-tick tests plus the standard package gates. A human verifies Queue and Feeds at their applicable presentations and focused-sidebar wheel arbitration.

Rollback is a normal commit revert; no persisted data or protocol changes occur.

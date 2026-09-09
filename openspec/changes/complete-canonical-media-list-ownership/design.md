# Complete Canonical Media-List Ownership Design

## Context

See `proposal.md` for motivation. PR #684 is archived at `openspec/changes/archive/2026-09-09-repair-canonical-media-list-ownership/`; it established the persistent component-view and retained-current-frame geometry seam on Queue and Grouped Music. The behavioral endpoint remains `openspec/specs/canonical-media-lists/spec.md`.

The earlier broad repair was correctly split into bounded implementation changes, but no active artifact inherited its complete visible-screen inventory. Issue #681 comments became the only cross-change checklist and drifted: they describe Queue/Feeds although PR #684 landed Queue/Grouped Music. The interactive-surface ledger tracks a different, already-completed whole-screen migration and cannot represent this nested media-list ownership campaign.

## Goals / Non-Goals

**Goals:**

- Keep one durable, reviewable screen inventory and fixed follow-on order.
- Preserve the shared decisions and screen-specific boundaries between changes.
- Make every bounded proposal traceable to one umbrella row and a defined acceptance gate.
- Distinguish behavioral truth, campaign status, historical evidence, and issue mirroring.

**Non-Goals:**

- Implementing any screen repair in this umbrella.
- Replacing the main specification or the interactive-surface ledger.
- Treating every list-like screen as a canonical media list.
- Predetermining source-level edits that each bounded proposal must establish through focused reconnaissance.

## Decisions

### D1. Visible screens are the campaign vocabulary

The umbrella and follow-on titles, scope, status, and progress SHALL name visible screens first: Queue, Grouped Music, Home, Feeds, Movies, Emby homevideos feed view, Emby podcast channel list, TV Series, Audiobookshelf Podcasts, and Audiobookshelf Books. Internal component and type names MAY appear in technical mappings only after the visible screen is identified.

Alternative: organize the campaign by shared Rust types. Rejected because it hides user-visible scope, produced ambiguous planning questions, and makes proposal completion difficult to understand without source-level knowledge.

### D2. Four records have distinct authority

- `openspec/specs/canonical-media-lists/spec.md` owns normative behavior.
- This umbrella's `tasks.md` owns campaign status, ordering, dependencies, and evidence links.
- Each bounded change owns the design and implementation plan for its named visible screens.
- Issue #681 mirrors status and links the authoritative OpenSpec artifacts; it owns no unique requirement.

The interactive-surface ledger continues to describe whole-screen Interactive Component ownership. Its existing `migrated` status SHALL NOT be interpreted as completion of this campaign.

Alternative: maintain the complete checklist in issue #681. Rejected because it already drifted and violates the repository rule that durable plans live in OpenSpec.

### D3. The PR #684 seam is the common baseline

Every follow-on SHALL cite PR #684 and its archived change as the accepted foundation: persistent embedded controls, one component-view paint entry, semantic paint policy, retained current-frame result, invalidation before a new frame, and point resolution from retained geometry.

A follow-on SHALL preserve the screen-level endpoint already stated by the main specification:

- one live position owner for the visible list;
- one ordinary-row painter;
- only the active responsive presentation moves;
- one `ViewportAnchor` handoff on a discrete Wide/Normal transition;
- ordinary content refresh preserves or clamps local state and does not synchronize presentations;
- screen-specific workspaces remain owned by their screen;
- effects receive resolved values rather than recomputing list state.

Alternative: restate or redesign the shared seam in every proposal. Rejected because it permits divergence and loses the value of the accepted foundation.

### D4. Screen-specific boundaries are recorded once

- Queue remains a fixed-row list in every panel mode.
- Grouped Music's track table remains Music-specific workspace state; the unfinished row concerns album-position ownership.
- Other Emby library tabs using a non-hero two-column catalog preserve that presentation and screen-owned grid interaction; they are not forced into one-column controls.
- TV seasons and episodes, including the season grid, remain TV-specific workspace state.
- Audiobookshelf episode filters and episode selection remain Podcast-specific workspace state.
- Audiobookshelf surname buckets, chapters, and absolute chapter seeking remain Book-specific workspace state.
- Non-grouped Music views are explicitly out of scope for this campaign and the Grouped Music follow-on: V1 album-folder states whose levels do not start with `group`, V2 transient group-list roots, V3 non-album intermediate levels, and V4 deeper-than-configured levels are not closed by any campaign row. Default-config non-grouped Music remains a separate product decision/change.

Alternative: repeat exclusions in every change. Rejected because repeated exemption bookkeeping was a primary cause of the original plan becoming too large and inconsistent.

### D5. Follow-ons have a fixed dependency order

The ordered course is:

1. complete Grouped Music album-position ownership;
2. repair Home;
3. repair Feeds;
4. repair Movies, the Emby homevideos feed view, and the Emby podcast channel list while isolating preserved two-column catalogs;
5. repair TV Series after the shared Normal-library decisions are accepted;
6. repair Audiobookshelf Podcasts;
7. repair Audiobookshelf Books;
8. perform final enforcement and documentation/spec reconciliation.

Home, Feeds, and the Audiobookshelf screens have no technical dependency on one another beyond PR #684, but their campaign order remains fixed to prevent ad hoc selection. TV Series depends on the accepted Movies-family decision because its Normal presentation shares that route. Final ratchets depend on every screen change so they encode surviving boundaries rather than speculative exemptions.

Alternative: allow maintainers to choose any next screen. Rejected because that recreates the continuity failure this umbrella exists to repair.

### D6. Every bounded proposal closes one ledger row or declared row group

Before implementation, a follow-on SHALL identify:

- its umbrella row or rows;
- prerequisite accepted changes;
- visible screen behavior being repaired;
- preserved screen-specific workspace;
- exact exclusions;
- focused component, buffer, and real-`Application::tick()` evidence required;
- applicable Normal/Wide human verification.

After acceptance and archive, the umbrella row SHALL record the change path, PR, accepted commit, automated evidence, human evidence or explicit waiver, and main-spec sync status. A row is not complete merely because code merged.

Alternative: infer completion from merged PRs. Rejected because PR #684 itself demonstrates that a bounded proof can be complete while the broader visible screen remains partial.

## Risks / Trade-offs

- **[The umbrella becomes another mega-change]** → It contains governance only; no screen implementation or duplicated behavioral delta.
- **[Status drifts from merged work]** → Updating the umbrella row is part of every follow-on's archive/acceptance gate.
- **[Internal names leak back into campaign scope]** → Visible screen names are mandatory in titles, rows, summaries, and issue updates.
- **[Preserved workspaces are accidentally absorbed]** → D4 is inherited by every follow-on and may change only through an explicit umbrella revision.
- **[Temporary compatibility becomes permanent]** → The final row cannot close until every named screen is accepted and remaining compatibility paths are explicitly reconciled.
- **[Parallel work violates ordering]** → The ledger records prerequisites; a proposal may be drafted early but cannot be accepted before its prerequisite rows.

## Migration Plan

1. Record PR #684 and the approved screen inventory in `tasks.md`.
2. Create bounded follow-on changes in D5 order; do not implement through this umbrella.
3. Update the owning row when each follow-on is proposed, accepted, synced, and archived.
4. After all screen rows are accepted, create the final enforcement/documentation change.
5. Reconcile the main specification, `CONTEXT.md`, `AGENTS.md`, the interactive-surface ledger, comments, and temporary compatibility paths.
6. Validate and archive this umbrella only after every row has acceptance evidence; then close issue #681.

Rollback is documentary: revert an incorrect ledger update and reopen the affected row. No runtime or persisted-data rollback applies.

# Finish Canonical Media-List Ownership Design

## Context

See `proposal.md` for motivation. The current behavioral truth is already defined by `openspec/specs/canonical-media-lists/spec.md`, `openspec/specs/interactive-component-framework/spec.md`, `openspec/specs/right-panel-arrangements/spec.md`, ADR 0022, and `CONTEXT.md`.

PR #684 established and verified the component-view/retained-current-frame seam for Queue and Grouped Music painting. It fully completed Queue. A later source-to-spec audit found that Grouped Music still retains parent album position authority, ordinary-render reseeding, and paint-result writeback, while other named destinations retain analogous ownership or geometry violations. Functional visual verification and prior review passes did not establish full architectural conformance.

Wide hero orientation is not an implementation defect: current source follows the normative browser-left/detail-right arrangement. Stale names/comments and contradictory scenario titles are documentation reconciliation items.

## Goals / Non-Goals

**Goals:**

- Maintain one accurate agent-facing ledger of verified destination status.
- Keep the governing specifications unchanged and require implementation to conform to them.
- Split future work into bounded, independently authorized visible-screen changes.
- Make conformance evidence durable and auditable before a row closes.
- Keep GitHub issue #681 and this umbrella linked and semantically aligned.

**Non-Goals:**

- Implementing any destination repair through this umbrella.
- Pre-creating or pre-authorizing follow-on proposals.
- Reopening Queue or non-grouped Music.
- Converting non-hero two-column catalogs.
- Treating provider workspaces, selectors, images, effects, persistence, or arrangement placement as list-control state.
- Adding speculative universal ratchets or a documentation sweep before concrete accepted work establishes the need.

## Decisions

### D1. Current specifications remain the acceptance contract

The campaign does not weaken or reinterpret the current specifications to match landed code. A destination is complete only when source and evidence satisfy the applicable requirements from the union of the canonical-list, Interactive Component, and arrangement contracts.

Alternative: revise the specification to describe PR #684's bounded seam endpoint. Rejected because that would erase the active-control-only ownership decision that motivated the campaign and would legitimize known duplicate state.

### D2. The audit establishes the initial ledger truth

The initial status is:

| Destination | Status | Verified gap |
| --- | --- | --- |
| Queue | complete | No counterevidence found against the full contract. |
| Grouped Music | incomplete | Parent album cursor/scroll, inactive-control synchronization, render-time target/scroll reseeding, paint-result writeback, and compatibility geometry remain. |
| Home | incomplete | Wide/Inline controls move in lockstep, shell per-section position remains, and compatibility geometry/point resolution remains. |
| Feeds | incomplete | Wide/Inline controls move in lockstep and parent row-map/compatibility point resolution remains. |
| Movies, Emby homevideos feed view, Emby podcast channel list | incomplete | Browser parent cursor/scroll mirrors the controls and compatibility geometry remains. |
| TV Series | incomplete | Series cursor mirrors the Wide control and row hits are re-derived from parent geometry, including a blank-hit fallback. |
| Audiobookshelf Podcasts | incomplete | The Wide show control is constructed per frame; parent show position, row maps, and paint writeback remain. |
| Audiobookshelf Books | incomplete | The Wide book control is constructed per frame; parent book/chapter position, row maps, render mutation, and paint writeback remain. |

Alternative: preserve the earlier “Queue and Grouped Music complete” baseline. Rejected because that statement described functional and bounded seam acceptance, not conformance to the full specification.

### D3. Follow-ons are screen-bounded and independently authorized

Each row or genuinely cohesive family starts with interactive exploration. Only after scope confirmation may one bounded proposal be created; implementation requires a later separate apply request. This umbrella records status and evidence but never makes proposal creation an apply task.

The initial families are:

1. Grouped Music.
2. Home.
3. Feeds.
4. Movies, Emby homevideos feed view, and Emby podcast channel list, preserving non-hero two-column catalogs.
5. TV Series, after the Browser-family ownership decisions it shares are accepted.
6. Audiobookshelf Podcasts.
7. Audiobookshelf Books.
8. Final conformance and only evidence-required reconciliation.

Only TV has a fixed technical dependency on the Browser family because its Normal presentation uses `BrowserComponent`. Other rows may be selected interactively rather than chained by an artificial serial order.

Alternative: one broad implementation change or a fixed serial campaign. Rejected because both recreate the sampling and continuity failures that caused the original plan to fail review.

### D4. Queue is the concrete ownership precedent

Future exploration should compare each destination against Queue's working properties: a persistent control owns position, stable target identity survives refresh, shell re-anchor is explicit, movement resolves once, pointer resolution uses retained current-frame geometry, and no parent row map or cursor mirror remains. Destination-specific workspaces remain outside that control.

This is a comparison tool, not permission to force Queue's fixed-row presentation onto Inline or provider-workspace surfaces.

Alternative: derive every repair independently from prose requirements. Rejected because a conforming landed precedent reduces ambiguity without introducing another abstraction.

### D5. Row closure requires a durable conformance record

A follow-on can close its umbrella row only after its archive/status update records, for every applicable requirement:

- the source owner and relevant path;
- the focused automated evidence;
- applicable live Normal/Wide evidence;
- reviewer disposition for any unclear or not-applicable clause;
- accepted commit/PR and main-spec sync status where behavior changed.

`openspec validate`, automatic spec sync, functional verification, and review are inputs, not substitutes for this trace. A bare `PASS` without durable reasoning cannot close a row.

Alternative: infer completion from checked slice tasks or a merged PR. Rejected because both previously recorded completion despite source-level nonconformance.

### D6. GitHub and OpenSpec are paired records

GitHub issue #681 is the human-readable campaign plan, current status, and index. This umbrella is the agent-facing status and execution ledger. Every accepted status change updates both and links the bounded change. Neither record may claim completion absent from the other.

Alternative: use only OpenSpec or only GitHub. Rejected because the user requires a human plan and agent execution detail, and the previous replacement of one with the other caused lost intent.

### D7. Reconciliation is evidence-driven

Stale comments, contradictory Wide-orientation scenario titles, compatibility paths, and structural ratchets are corrected with the family that proves their disposition, or in the final row if they remain. The final row cannot absorb an unimplemented destination repair.

Alternative: begin with a global docs/ratchet cleanup. Rejected because premature universal rules and exemption bookkeeping were a primary source of scope growth.

## Risks / Trade-offs

- **[The umbrella becomes another implementation plan]** → Keep its tasks limited to status transitions, bounded-change links, and evidence; implementation details belong only to authorized follow-ons.
- **[A family is marked complete from functional behavior again]** → Require the per-requirement conformance record before closure.
- **[GitHub and OpenSpec drift]** → Updating both records is one acceptance action for every status transition.
- **[Preserved workspace state is mistaken for a violation]** → Each follow-on identifies the exact list position/geometry authority separately from legitimate provider workspace state.
- **[Cleanup hides unfinished migration]** → Final reconciliation may not change an incomplete destination's status or include its source repair.

## Migration Plan

1. Establish this planning-only umbrella and the matching human record in issue #681.
2. Explore and authorize bounded follow-ons one family at a time; do not implement through this umbrella.
3. After each follow-on is accepted, synced where applicable, and archived, add its durable conformance record and update the matching GitHub status.
4. Run the final source-to-spec audit after every included destination row is complete.
5. Reconcile only surviving comments, scenario wording, compatibility paths, and narrow structural enforcement proven by that audit.
6. Archive this umbrella and close issue #681 only when the two records agree and every included row has auditable evidence.

Rollback is documentary: revert an incorrect ledger update and reopen the affected row. Source rollback belongs to the bounded follow-on that changed it.

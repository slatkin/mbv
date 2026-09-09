# Complete Canonical Media-List Ownership — Retired Campaign Record

## Context

This umbrella was created to recover continuity after PR #684, but its fixed follow-on rows incorrectly made proposal creation part of campaign execution. Planning for a visible screen is interactive: the user selects the problem, evidence and scope are explored, and a bounded proposal is created only after explicit authorization.

PR #684 remains the accepted historical baseline. Its archived change and merge `f647136a` established the Queue and Grouped Music painting/geometry seam. This record does not extend that behavior.

## Decisions

### D1. Preserve the accepted baseline

Rows 1.1 and 1.2 in `tasks.md` retain the provenance for the accepted PR #684 foundation and the approved historical inventory/boundaries. They are records of what was accepted, not an active work queue.

### D2. Preserve the non-grouped Music reconciliation

Row 1.3 records that the discovered non-grouped Music states were outside the Grouped Music campaign scope. The evidence and the decision were recorded in commit `577683a1`. This is historical scope bookkeeping only: no campaign row claims those views are correct or complete, and no standalone backlog commitment is created here.

### D3. Withdraw the proposal-generating campaign

The former rows 2–9 are withdrawn and removed from the ledger. Their fixed ordering, dependencies, and follow-on proposal names have no remaining authority. No proposal is to be generated from this retired record.

The planning-only artifacts accidentally created for row 2.1 in `5030adcd` were removed by `bd243f34`.

### D4. Use an interactive planning lifecycle for future work

A future screen effort follows this sequence only after a new user request:

1. Explore the selected visible screen/problem interactively and gather bounded source evidence.
2. Resolve material scope, preserved workspace, exclusions, and acceptance expectations with the user.
3. Create one bounded OpenSpec proposal after explicit authorization.
4. Review and revise that proposal interactively as needed.
5. Implement only after a separate explicit apply request.

No campaign ledger should pre-authorize proposal generation or select the next screen without that interaction.

## Non-goals

- Implementing or reviewing source code.
- Creating a replacement follow-on proposal.
- Syncing behavioral specs.
- Opening a backlog commitment for deferred non-grouped Music.
- Treating the historical inventory or scope decision as normative runtime behavior.

## Closure

After the three accepted baseline rows remain as historical evidence, this change is archived. Issue #681 is marked superseded so it cannot act as a parallel campaign authority.

## 1. Reconcile the three documents (#614)

- [x] 1.1 `docs/architecture/interactive-surface-ledger.md`: confirm every row's
      state, owner, and painter against the tree, using the per-breakpoint
      owner/painter column #625 added. Correct any row whose notes assert
      something the code no longer does — lines 66/68/69's "narrow = sole legacy
      renderer (D5)" is the known case, but check every row rather than only
      those. Verify: each row cites a symbol or test that exists.
- [x] 1.2 ADR 0022: bring its description of state ownership in line with the
      post-#626 tree. Verify: no statement in the ADR describes an `App` field
      that no longer exists, and none describes a boundary the types do not
      enforce.
- [x] 1.3 `openspec/specs/interactive-component-framework/spec.md`: merge the
      applied deltas from #621 and #625. Verify: the merged spec has no
      requirement contradicting another, and the one-owner/one-painter
      requirement reads as the general invariant #625 intended.
- [x] 1.4 Record #607's acceptance criterion as met, citing
      `delete-browse-level-cursor-scroll` task 4.3's resolved inventory.
      Verify: the citation points at a specific resolved list, not a claim.

## 2. Archive

- [x] 2.1 Archive the completed changes in the chain
      (`split-browse-state-interaction-fields`,
      `migrate-narrow-browse-to-components`,
      `delete-browse-level-cursor-scroll`, and this one) per AGENTS.md, dating
      each as archived. Verify: `openspec/changes/` retains only in-flight
      changes.
- [x] 2.2 Report anything that could not be made true by editing documents
      alone. Verify: the list is empty, or it is stated with the code change
      each item would need. Do not make that change here.
      See `findings.md` — two items (Queue dual-painter → #629;
      `FeedHomeVideoState::video_cursor` split → design D6), each with the code
      change it would need. Both already disclosed truthfully in the ledger.

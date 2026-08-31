## Why

Issue #614 asks to reconcile the interactive-surface ledger, ADR 0022, and the
source completion state so all three describe the same thing. They currently do
not, and the preceding changes in this chain are what make a truthful
reconciliation possible:

- The ledger claims every row reached `migrated` (2026-08-27) and that narrow
  Movies, TV, and Music are each a "sole legacy renderer (D5)". Verified false
  for Movies, and silent on who owns the narrow cursor —
  `migrate-narrow-browse-to-components` (#625) is what makes the claim true.
- ADR 0022 and
  `openspec/specs/interactive-component-framework/spec.md` describe a boundary
  that `App` still violated while `BrowseLevel::cursor` existed —
  `delete-browse-level-cursor-scroll` (#626) is what closes it.

Writing the reconciliation before those land would just move the inaccuracy
into a different document.

## What Changes

- Update `docs/architecture/interactive-surface-ledger.md`, ADR 0022, and
  `openspec/specs/interactive-component-framework/spec.md` so all three
  describe the same completion state.
- Record #607's acceptance criterion as met, citing the resolved inventory
  rather than asserting it.
- Merge the applied spec deltas from the chain into `openspec/specs/` and
  archive the completed changes, per AGENTS.md.

## Non-goals

- No code change. If a document cannot be made true without one, that is a
  finding to report, not a fix to make here.
- The per-breakpoint owner/painter ledger column is added by #625, not here;
  this change consumes it.

## Capabilities

No new spec requirements — this change merges the deltas the chain already
proposed.

## Impact

`docs/architecture/interactive-surface-ledger.md`, `docs/adr/` (ADR 0022),
`openspec/specs/interactive-component-framework/spec.md`,
`openspec/changes/archive/`.

## Sequencing

Last in the chain: depends on `delete-browse-level-cursor-scroll` (#626), which
depends on `migrate-narrow-browse-to-components` (#625), which depends on
`split-browse-state-interaction-fields` (#621).

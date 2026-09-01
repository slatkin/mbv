# Design

## Boundary and dependencies

This is a documentation-led cleanup after accepted canonical foundation,
Home/Feeds, Music/Audiobookshelf, and Queue slices. It stacks on PR #606's
`feat/migrate-tui-to-tuirealm` work but is a distinct feature/rollback boundary;
PR #606 must not be rolled back implicitly. No destination slice is included.
#640 is superseded by the Music/Audiobookshelf slice and is not revived.

## Cleanup method

1. Inventory obsolete `render_*_rows` loops, bespoke painters, old selection /
   scroll / hit geometry, and `AppLayout::main` left/hero/selector/wide-family
   fields using ast-grep plus grep. Record symbol and caller sets.
2. For each candidate, confirm zero references across Rust, tests, and docs;
   delete only candidates with an explicit zero-reference gate.
3. Re-run inventory after each deletion, then format and enforce the 800-line
   source limit.
4. Reconcile ADR 0022, `interactive-surface-ledger.md`, `CONTEXT.md`, and
   `.agents/skills/mbv-frontend/SKILL.md` only where their migration-era wording
   describes the removed loops or obsolete ownership. Keep canonical controls,
   non-hero two-column layout, Queue fixed rows, and feed terminology intact.

No new abstraction, painter, dependency, protocol, Service, Player, persistence,
or provider API is allowed. Canonical controls remain the only list vocabulary;
there are no bespoke exceptions.

## Verification

Static gates must include `rtk ast-grep scan`, targeted ast-grep symbol/caller
queries, `rtk grep` zero-reference searches, `rtk make check-code-file-lines`,
`rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`,
and relevant `rtk cargo nextest run` suites. Run strict OpenSpec validation.

Visual acceptance is live, not inferred from screenshots: after cleanup, the
user verifies narrow 60x20 and Wide 120x40 and 140x30, including list selection,
scrolling, hero/rail framing, non-hero two columns, Queue fixed rows, and both
feed meanings. UI tests may be changed only after that confirmation.

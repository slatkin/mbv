# Design

## Boundary and dependencies

The canonical foundation lands first. Home/Feeds, Music/Audiobookshelf, and
Queue then proceed as independent sibling destination slices. This cleanup
starts only after all four accepted slices are complete; accepted SHAs are
recorded at implementation-issue time and are deliberately not pinned in this
plan. #640 is superseded by the Music/Audiobookshelf slice and is not revived.

Cleanup is a separate, independently reversible deletion/documentation-only PR
targeting `feat/migrate-tui-to-tuirealm`; that branch is not a source boundary.
It owns only cross-family obsolete loop/painter/geometry deletion, its own docs
reconciliation, and strict validation. Destination defects and all umbrella
4.x/5.x final gates remain with their owners. No visual correction is allowed
here.

## Cleanup method

1. Inventory the exact obsolete `render_*_rows` loops and bespoke painters,
   old selection/scroll/cursor geometry (NOT row-hit / `*HitRegion`
   geometry, which #638 owns; and NOT component-owned viewport geometry,
   which the canonical controls keep), and `AppLayout::main`
   left/hero/selector/wide-family fields. Record symbols plus callers/readers/
   writers with ast-grep and grep.
2. For each candidate, prove staged zero production callers/readers/writers
   before deletion. Track test and documentation references separately; do not
   treat their presence as production use.
3. Delete only candidates whose production gate passes. Re-run the inventory
   after each deletion.
4. Reconcile ADR 0022, `docs/architecture/interactive-surface-ledger.md`,
   `CONTEXT.md`, and `.agents/skills/mbv-frontend/SKILL.md` to final
   `WideMediaList`/`InlineMediaBrowser` ownership and terminology. Update stale
   UI tests/docs and run automated gates plus a final whole-tree zero-reference
   check. Then perform live acceptance at the required breakpoints. Preserve
   canonical controls, the non-hero two-column arrangement, Queue fixed rows,
   and the Feeds Service/homevideos distinction. Route any discovered visual
   defect to its owning slice and rerun affected cleanup gates before acceptance.

No new abstraction, painter, dependency, protocol, Service, Player,
persistence, provider API, destination behavior, or visual variant is allowed.

## Verification

Run strict OpenSpec validation, ast-grep symbol/caller checks, `rtk grep`
zero-reference checks for every deleted candidate, file-size validation,
formatting, workspace checks, and relevant tests. Live acceptance follows the automated gates at narrow 60x20 and Wide 120x40
and 140x30, covering selection, scrolling, hero/rail framing, non-hero two
columns, Queue fixed rows, and both feed meanings; it is not inferred from
screenshots. There is no pre-test visual-approval checkpoint.

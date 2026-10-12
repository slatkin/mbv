# Dedupe wheel Scroll-contract tests across surfaces (issue #929)

Status: decided — delete all three per-surface wheel tests, re-point the
ledger rows at the shared owners, no spec delta.

## Context

PR #925 added per-surface wheel tests that repeat the same assertion shape:
the wheel claim is returned, the selection is unchanged, and the offset moves
by the step. Issue #929 (found in the /simplify review) asks how many
per-surface wheel tests each route needs, and to update
`docs/architecture/interactive-surface-ledger.md` wheel rows if tests are
removed. This change is test-and-docs only; no production code changes.

Deliverable per `/opsx-propose`: an OpenSpec change (`dedupe-wheel-contract-tests`)
with proposal, design, and tasks; `skip_specs` declared (user decision — no
spec delta: re-pointing ledger proof references does not change behavior, and
the mouse-input wheel-ledger requirement stays satisfied). Implementation
happens only on an explicit apply request.

## User decisions (2026-10-12)

1. Disposition: **delete all three** per-surface wheel tests. Every ledger row
   cites the shared tests only, matching the Home/Podcast pattern. The
   component-level wheel routing for TV and books becomes unguarded by
   direct tests — accepted risk (the shared `media_list` and
   `tree_browser` tests still own the Scroll contract itself).
2. Spec delta: **none** — `skip_specs`.

## Findings (code facts)

Shared contract owners (survive; they keep the whole Scroll contract):

- Canonical-list wheel: `crates/mbv-components/src/media_list/tests.rs:187`
  `a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched` —
  claim → `Scroll`, selection facts untouched, offset by delta.
- Tree wheel: `crates/mbv-components/src/list/tree_browser/tests/structure.rs:343`
  `a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node`.
- Claim-gate-on-retained-geometry shape (task 4.1):
  `crates/mbv-components/src/emby_library_content/tests.rs:181`
  `wheel_reach_resolves_the_last_painted_selectable_row_as_an_item_index`.

Tests to delete:

1. `crates/mbv-components/src/tv_content/tests/wheel_tests.rs` — whole file
   (both tests: `show_tree_wheel_keeps_the_selection_and_reports_the_claim`,
   `flat_series_rail_wheel_keeps_the_selection_and_reports_the_claim`), plus
   the `mod wheel_tests;` line in `crates/mbv-components/src/tv_content/tests.rs:10`.
2. `crates/mbv-components/src/book_content/tests.rs`
   `book_wheel_scrolls_viewport_without_a_selection_report` (test fn only).

Orphan check (verified): `tv_tree_context` is used by four other
`tv_content/tests/` files — it stays. The `finish_view` setup, imports, and
`book`/`state_with_books` helpers in `book_content/tests.rs` are used by other
tests there — they stay. `wheel_tests.rs` deletes cleanly with no shared
fixture loss.

Ledger rows to re-point (Home row :78 and Podcast row :82 are the pattern —
cite the shared test plus one prose sentence "delegates the wheel into the
same shared Scroll; adds no step of its own"):

- Wide TV row (`interactive-surface-ledger.md:77`): drop the two
  `tv_content::tests::wheel_tests::…` citations; keep the tree `Scroll`
  citation (`list::tree_browser::tests::structure::a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node`);
  prose: `TvContent` routes the wheel into its shared owners (carrier list or
  tree) and adds no step of its own.
- Audiobookshelf books row (`:83`): replace the
  `book_content::tests::book_wheel_scrolls_viewport_without_a_selection_report`
  citation with the shared `media_list` test citation + the Podcast-style
  prose.

Ruled out: `#[case]` table folding — invariant 14 rule 4
(`docs/invariants/14-test-ownership.md:30`) forbids identical-outcome cases,
and the fixtures differ in kind.

## Files to modify

- `crates/mbv-components/src/tv_content/tests/wheel_tests.rs` — delete file.
- `crates/mbv-components/src/tv_content/tests.rs` — drop `mod wheel_tests;`.
- `crates/mbv-components/src/book_content/tests.rs` — delete one test fn
  (and its now-unused local `use` lines inside it, if any become orphaned).
- `docs/architecture/interactive-surface-ledger.md` — re-point Wide TV and
  Audiobookshelf books wheel rows.

## Reuse

- Ownership rules: `docs/invariants/14-test-ownership.md` (delete-by-default,
  provenance, one contract one owner).
- Tests matrix: `docs/architecture/tui-frontend.md` §Tests ("A removed
  assertion names its surviving owner" — the ledger prose does this).
- Ledger pattern rows to copy: `interactive-surface-ledger.md:78` (Home),
  `:82` (Podcast).

## Steps (after /opsx-apply → openspec apply)

- [x] Create the change with `openspec new change dedupe-wheel-contract-tests`
      declaring `skip_specs`; write proposal.md, design.md, tasks.md from this
      plan.
- [x] Delete `tv_content/tests/wheel_tests.rs` and its `mod` line.
- [x] Delete `book_wheel_scrolls_viewport_without_a_selection_report` from
      `book_content/tests.rs`.
- [x] Re-point the Wide TV and Audiobookshelf books ledger rows at the shared
      `media_list` / `tree_browser` tests with Podcast-style prose; confirm no
      dangling test names remain in the Wheel-proof column.
- [x] `cargo nextest run -p mbv-components`; `cargo clippy --workspace
      --all-targets -- -D warnings`; `cargo fmt`.
- [x] Check no file crossed 800 lines (ledger is well under; deletions only
      shrink the test files).

## Verification

- `cargo nextest run -p mbv-components` — suite passes; net test count drops
  by exactly 3.
- `rg -n "wheel_tests|book_wheel" docs crates/mbv-components` — no surviving
  references to the deleted tests (ledger rows re-pointed, no compile errors).
- Manual skim of the two ledger rows: each names the shared owner test and a
  one-sentence delegation note, matching the Home/Podcast row shape.

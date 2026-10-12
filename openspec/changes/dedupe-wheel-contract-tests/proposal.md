# Proposal

## Why

PR #925 added per-surface wheel tests that repeat the same assertion shape:
the wheel claim is returned, the selection is unchanged, and the offset moves
by the step. The shared Scroll contract already has one owner per route
(`media_list::tests::a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched`
for canonical lists,
`list::tree_browser::tests::structure::a_wheel_scroll_moves_the_viewport_and_keeps_the_selected_node`
for trees, and the task-4.1 emby test for the claim-gate-on-retained-geometry
shape). Three surface tests re-assert that shape with their own fixtures
(issue #929, from the /simplify review of PR #925). Each redundant assertion
locks the surfaces in place and turns every refactor into bulk test repair;
invariant 14 says a test whose contract is owned elsewhere should not exist.

The user chose the aggressive disposition: delete all three. The component-level
wheel routing for TV and books then has no direct test guard — an accepted
risk, matching the ledger pattern the Home and Podcast rows already follow
(they cite only the shared test with a "adds no step of its own" note).

## What Changes

- Delete the three per-surface wheel tests:
  `tv_content::tests::wheel_tests::{show_tree_wheel_keeps_the_selection_and_reports_the_claim, flat_series_rail_wheel_keeps_the_selection_and_reports_the_claim}`
  (whole file plus its `mod` line) and
  `book_content::tests::book_wheel_scrolls_viewport_without_a_selection_report`.
- Re-point the Wide TV and Audiobookshelf books wheel rows in
  `docs/architecture/interactive-surface-ledger.md` at the surviving shared
  tests, with the Home/Podcast-style delegation prose.
- Net test-count change: −3. No production code changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Tests and architecture docs only (`skip_specs: true`): re-pointing the
ledger's Wheel-proof references does not change behavior, and the mouse-input
wheel-ledger requirement stays satisfied.

## Impact

- `crates/mbv-components/src/tv_content/tests/wheel_tests.rs` (deleted),
  `crates/mbv-components/src/tv_content/tests.rs` (one `mod` line),
  `crates/mbv-components/src/book_content/tests.rs` (one test fn).
- `docs/architecture/interactive-surface-ledger.md` (two wheel rows).
- `tv_tree_context` and the book test helpers stay: other tests use them
  (orphan check verified).

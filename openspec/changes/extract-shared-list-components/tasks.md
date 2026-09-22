# Tasks

## 1. Baselines and forced file splits

- [ ] 1.1 Record the pre-change baseline into `openspec/changes/extract-shared-list-components/baseline.md`: `wc -l` for every file in `src/app/components/media_list/` and every `src/app/components/music_tree*.rs`, plus `cargo llvm-cov` line coverage for the `music_tree*` and `media_list` modules. Verify: the file exists with both numbers recorded — task group 4's gate is measured against it and cannot be evaluated without it.
- [ ] 1.2 Split `src/app/components/media_list/mod.rs` (~30K) into cohesive files under the 800-line cap, moving code only — no signature or behavior changes. Verify: `cargo check -p mbv` clean, `cargo nextest run -p mbv` green, and `git diff --stat` shows only moves.
- [ ] 1.3 Split `src/app/components/music_tree.rs`, `music_tree_model.rs` and `music_tree_selection.rs` to the same cap, moving code only. Note these are `include!`-composed today — preserve that composition. Verify: `cargo check -p mbv` clean, `cargo nextest run -p mbv` green, `git diff --stat` shows only moves.
- [ ] 1.4 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`. Verify: both clean before any behavior work begins.

## 2. The seam

- [ ] 2.1 Define `RowFlow<Target>` in a new `src/app/components/list/` module: ordered rows, selectable-with-target versus structural-without, `len`, `row_at`, `position_of(&Target)`. No dependency on `tui-treelistview` or on `media_list` types. Verify: unit tests over a minimal in-test implementation cover structural rows occupying positions without being addressable, per the `shared-list-components` scenarios.
- [ ] 2.2 Add `Cursored` as default methods over `RowFlow`: move/first/last/index/select_target, skipping structural rows, clamping at both ends. Verify: unit tests cover skip-over-structural, clamp-no-wrap, and unreachability of rows absent from the flow.
- [ ] 2.3 Add `Viewported` as default methods: offset, clamp, keep-selection-visible (preserve prior offset where bounds permit, else minimum scroll), plus the named paging-policy hook from design D4. Verify: unit tests cover preserve-vs-minimum-scroll, geometry-change clamping in place, and that two shapes with different paging policies do not inherit each other's.
- [ ] 2.4 Add `PaintRetained` as default methods: begin/finish view, explicit invalidation (design D6), `claims_point`, `resolve_point -> Option<&Target>`, `selected_row_rect`. Verify: unit tests cover claiming no point while invalid, and resolution returning a stable target with no row map exposed.
- [ ] 2.5 Add `MarkSelection`: ordered membership in stable targets, add/remove preserving relative order. Aggregation deliberately excluded (design D5). Verify: unit tests cover order after interleaved add/remove.
- [ ] 2.6 Add `Expandable`: expand/collapse, parent/child movement, expansion survival by stable identity, and mark aggregation. Verify: unit tests over a minimal in-test nesting implementation cover collapse removing descendants while selection and viewport stay valid.

## 3. Shape implementations

- [ ] 3.1 Implement the seam traits for the flat shape over the existing `MediaList`/`WideMediaList`, delegating to existing behavior without deleting anything yet. Verify: `cargo nextest run -p mbv` fully green — this step must be behavior-neutral and additive.
- [ ] 3.2 Confirm the flat shape is behavior-neutral across destinations by running the canonical and destination suites (`media_list`, queue, home, feeds, podcast, book, emby library, TV, inline search) plus the tick-integration suites. Verify: all green with no test edits; any edit needed here means 3.1 was not behavior-neutral.
- [ ] 3.3 Add the internal target↔node map to `MusicTreeBrowser` and convert its public surface to stable targets: `hit_node`, `projected_nodes`, `model_is_track`, `select_index`, `row_rect_for` and every other arena-`usize` entry point. Make `MusicNodeKey` and the arena private. Handle explicitly the case of a target present in the model but absent from the current projection (design D2). Verify: `cargo check -p mbv` clean with no `usize` node id reachable from outside the tree module.
- [ ] 3.4 Update `music_content*.rs` call sites to the target surface. Verify: `cargo check -p mbv` clean and `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] 3.5 Delete the arena-index assertions in `music_tree_tests.rs` and `music_tree_browser_tests.rs` and write fresh tests against the target surface — do not translate case by case. Verify: `cargo nextest run -p mbv` green.
- [ ] 3.6 Update `music_content_tree_tests.rs`, `render/tests_music_characterization.rs` and `render/tests_music_groups.rs` to the target surface, preserving what they assert about painting. Verify: buffer assertions unchanged in intent and green.
- [ ] 3.7 Implement the seam traits for the tree shape over `MusicTreeBrowser`, including `Expandable`, preserving its existing mark aggregation divergence for hidden filtered children. Verify: `src/app/tests_tick_integration_music*.rs` green — composition, not direct method calls, is the backstop here.

## 4. The deletion gate

- [ ] 4.1 Delete the cursor, viewport and keep-visible arithmetic now duplicated in `media_list/` and in the tree module, routing both through the seam's defaults. Verify: `cargo nextest run -p mbv` green.
- [ ] 4.2 Delete the duplicated retained-geometry and point-resolution code from both, including the tree's `paint_complete` flag in favour of explicit invalidation (design D6). Verify: `cargo nextest run -p mbv` green and no `paint_complete` field remains.
- [ ] 4.3 Delete the duplicated ordered-multi-selection bookkeeping (`selection_order` in the tree, its flat counterpart) in favour of `MarkSelection`. Verify: `cargo nextest run -p mbv` green.
- [ ] 4.4 **Acceptance gate.** Compare `wc -l` across `media_list/` and the tree module against `baseline.md` from 1.1. Verify: the combined production line count is substantially net-negative. If it is not, the seam is a parallel abstraction rather than a shared one — stop, report, and propose reverting rather than shipping (design: Risks).
- [ ] 4.5 Compare `cargo llvm-cov` for the `music_tree*` and `media_list` modules against the 1.1 baseline. Verify: coverage is at or above the baseline for both; a shortfall is a blocker, not a note.

## 5. Verification and close-out

- [ ] 5.1 Confirm no seam type exposes a `tui-treelistview` type and that `media_list` has no path to that crate. Verify: the crate appears only behind the tree module's boundary.
- [ ] 5.2 Run the full gate: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo nextest run -p mbv`, and the workspace test run. Verify: all clean.
- [ ] 5.3 Confirm every governed file is at or under 800 lines. Verify: no file in `media_list/`, the tree module, or the new `list/` module exceeds the cap — a PR must not open while one does.
- [ ] 5.4 Run `openspec validate extract-shared-list-components --strict`. Verify: passes.
- [ ] 5.5 Delete `baseline.md` now that both gates have been evaluated, and commit the change with its planning artifacts. Verify: worktree clean.

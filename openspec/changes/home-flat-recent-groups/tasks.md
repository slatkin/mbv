# Tasks

## 1. Flat Home owner (design D1–D4)

- [x] 1.1 In `mbv-ui-model`: remove `HomeTreeTarget`, `HomeRoot`, `RecentAge`, and `recent_age` with its `#[case]` test table. Keep `HomeItemTarget`, `HomeLatestLaunchWindow`, and `RECENTLY_ADDED_CUTOFF_SECS`. Add a small helper in `home_latest.rs` that maps a timestamp and `now_secs` to `Option<FeedAgeGroup>`, returning `Some` only for `New` and `Recent` (use `feed_age_group`). Verify: `cargo check -p mbv-ui-model`; the compiler flags the callers that rows 1.2 and 1.3 fix.
- [x] 1.2 Replace `TreeBrowser<HomeTreeTarget>` in `HomeContent` with `MediaListCarrier<HomeItemTarget>`. Replace `home_content/tree.rs` with a flat projection (`home_content/flat.rs`) that emits the headers, spacers, and item rows of design D2, with split titles and date gutters as today. Change `set_recent_rows` to take `now_secs` (design D4), and drop the owner's `launch_window` field and accessor. Update `push_home_content` in `src/app/shell/home.rs` to pass `current_time_secs()`. Contract: the spec requirement "Recently added is grouped by age with the Feeds rule". Verify: one projection test in `home_content/tests.rs`: an item 1 day old goes under `New`, one 13 days old under `Recent`, one 14 days old is absent, and an empty header is omitted. `cargo check -p mbv-components -p mbv`.
- [x] 1.3 Port input to the flat carrier (design D5): move it to `home_content/keyboard.rs` and remove tree-only keys. Enter plays; Ctrl+Enter, Ctrl+W, Delete, and `.` act on the marked items in display order, or on the selected row. Keep the widened `Vec<HomeItemTarget>` payloads and the per-Service shell handling. Contract: the spec requirement "Home actions resolve the row's own section and Service identity". Delete the tree tests. Verify: two interactive tests: marking two items under `New` and pressing Ctrl+Enter emits them in display order; one item under both sections gives two separately selectable targets. `cargo nextest run -p mbv-components`.
- [x] 1.4 Hero and restoration (design D6): the hero resolves the selected item directly; `reset_presentation` and re-anchor select the first item row; a legacy Latest selector lands on the first item. Delete the stem and first-descendant hero code. Rewrite the `home_content/launch_state.rs` tests against the flat list, one per behavior. Verify: `cargo nextest run -p mbv-components`.
- [x] 1.5 Fix the shell tests and callers the removals break, with the smallest change each. Verify: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run -p mbv`.

## 2. Remove the tree secondary title (design D7)

- [ ] 2.1 Delete `TreeNode.secondary`, `with_secondary`, its search-text use, the tree painter's split-title branch, and the painter test from row 2.2 of the archived change. Fix every literal the compiler flags. Verify: `cargo check --workspace --all-targets`, and `cargo nextest run -p mbv-render -p mbv-components` with Music and TV tree tests green.

## 3. Standards findings (design D8)

- [ ] 3.1 Replace the `_` arm in `handle_home_request` (`src/app/shell/home.rs`) with the remaining `ShellRequest` variants, or a documented no-op arm per AGENTS.md. Verify: `cargo check -p mbv`.
- [ ] 3.2 Move the inline test module in `src/app/dispatch/library/load.rs` to `src/app/dispatch/library/load/tests.rs`. Verify: `cargo nextest run -p mbv -E 'test(load)'` passes with the same tests.
- [ ] 3.3 In the Home files this change touches (`home_content.rs`, `home_content/*.rs`, `src/app/shell/home*.rs`, their tests), retarget comments that cite the archived change's design numbers or task rows to `openspec/specs/home-latest-sections` or `destination-latest-modes`. Verify: `rg "design D[0-9]|row [0-9]\.[0-9]" crates/mbv-components/src/home_content* src/app/shell/home*` returns nothing.

## 4. Docs and gates

- [ ] 4.1 Update `CONTEXT.md`: "Home view" is one flat list with group headers `Continue watching` and `Recently added`; "Recently added" has the age groups `New` (0–1 days) and `Recent` (2–13 days), by the Feeds rule, and no roots, branches, or collapse. Verify: `qmd query "Recently added"` returns the updated term.
- [ ] 4.2 Run the gates: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All pass.
- [ ] 4.3 Just before push, run `make check-code-file-lines`. Split any governed file over 800 lines along responsibility seams. The check passes.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive the change after review.

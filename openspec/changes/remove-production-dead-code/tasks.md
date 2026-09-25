# Tasks

## 1. Inventory and baseline

- [ ] 1.1 Enumerate every `allow(dead_code)`, `cfg_attr(..., allow(dead_code))`, and test-module `allow(dead_code, ...)` occurrence under `src/` and `crates/`, including the issue-listed list/media-list, render, destination, shell/input, `mpris`, and test-support paths; verify the inventory covers all current matches and record a live/test-only decision for each item before editing.
- [ ] 1.2 Run the baseline `cargo check -p mbv` and hermetic `cargo nextest run -p mbv`, recording pre-existing failures separately; verify the starting point is known before removing any declarations.

## 2. Shared list seam

- [ ] 2.1 Audit and clean `src/app/components/list/expandable.rs`, `marks.rs`, `viewport.rs`, and `paint.rs`: remove production-unreachable members and only their test-only consumers, or unsuppress proven-live members, then clean imports; verify with `cargo check -p mbv` and the focused `cargo nextest run -p mbv list` tests.
- [ ] 2.2 Audit the shared tree browser in `src/app/components/list/tree_browser/mod.rs` and `types.rs` and its `tests/` modules, preserving stable-target and shared-list behavior while deleting only unreachable seams; verify with `cargo check -p mbv` and `cargo nextest run -p mbv tree_browser`.
- [ ] 2.3 Clean the three-line list in `src/app/components/list/three_line/mod.rs` and its tests, and remove the speculative “second adopter” convention from `src/app/components/list/mod.rs`; verify with `cargo check -p mbv` and `cargo nextest run -p mbv three_line`.

## 3. Media-list seam

- [ ] 3.1 Audit `src/app/components/media_list/anchor.rs`, `carrier.rs`, and their directly related tests, removing test-only anchor/carrier state and stale attributes without changing the canonical row-flow or hit-geometry contracts; verify with `cargo check -p mbv` and `cargo nextest run -p mbv media_list`.
- [ ] 3.2 Audit `src/app/components/media_list/mod.rs`, `wide.rs`, and `tests.rs`, retaining production-used projections and deleting only unused wide-list helpers/tests; verify with `cargo check -p mbv` and `cargo nextest run -p mbv media_list`.

## 4. Render and theme surfaces

- [ ] 4.1 Audit the item- and module-level suppressions in `src/app/render/theme/mod.rs`, `palette.rs`, `surface.rs`, `surface_resolve.rs`, and `surface_table.rs`; remove dead theme/surface members, remove the stale palette rationale if applicable, and unsuppress live members; verify with `cargo check -p mbv` and the theme-related nextest filters.
- [ ] 4.2 Audit `src/app/render/components/tree_browser/mod.rs` and `chrome_player_context.rs`, removing unreachable render helpers and their test-only support while preserving the shared tree painter and playback presentation; verify with `cargo check -p mbv` and `cargo nextest run -p mbv render`.

## 5. Destination and component content

- [ ] 5.1 Audit the suppressions in `src/app/components/feeds_content/mod.rs`, `tv_content/mod.rs`, `tv_content/episode_rows.rs`, and `tv_content/tree_projection.rs`; remove only production-unreachable feed/TV helpers and tests, retaining all live browse, filter, expansion, and presentation behavior; verify with `cargo check -p mbv` and the feeds/TV nextest filters.
- [ ] 5.2 Audit `src/app/components/music_content/mod.rs`, `library_panel/content.rs`, `library_panel/panel/mod.rs`, `library_panel/wide/mod.rs`, and `state/music_grouping.rs`; delete unreachable music/library projection helpers and their sole-purpose tests, then clean visibility/import fallout; verify with `cargo check -p mbv` and the music/library nextest filters.
- [ ] 5.3 Audit the remaining destination and typed-request suppressions in `components/msg/shell.rs`, `music_tree_target.rs`, `inline_search.rs`, and `context_menu.rs`, preserving live Msg and search behavior while deleting only unreachable contract surface; verify with `cargo check -p mbv` and `cargo nextest run -p mbv inline_search`.
- [ ] 5.4 Audit `components/queue/mod.rs`, `playlists.rs`, `save_playlist.rs`, `tab_panel.rs`, `status_bar_panel.rs`, `user_event.rs`, and `mouse/hit.rs`; remove dead component helpers and tests that exist only for them, without changing queue authority, mouse routing, or input precedence; verify with `cargo check -p mbv` and the affected nextest filters.

## 6. Shell, input, and test-only support

- [ ] 6.1 Audit `src/app/shell/mod.rs`, `shell/library_panel.rs`, `input/key_policy/mod.rs`, and `dispatch/session/connect.rs`; remove unreachable shell/input/session helpers, drop the matching dead-code rationale if its primitive is deleted, and leave the single keyboard router and live session-connect paths unchanged; verify with `cargo check -p mbv` and the shell/input/session nextest filters.
- [ ] 6.2 Audit `src/mpris.rs`, `crates/mbv-core/src/config/tests/launch_state.rs`, and all compound test-module allows under dispatch, input, and render test helpers; remove dead support and the `dead_code` portion wherever it is no longer needed, retaining an independently justified `unused_imports` allowance only if required; verify with `cargo check -p mbv-core --all-targets`, `cargo nextest run -p mbv-core`, and `cargo check -p mbv`.
- [ ] 6.3 Re-run the complete suppression search and inspect the diff for accidental API or behavior changes, correcting any remaining dead-code allowance at its source rather than adding a replacement; verify the search returns no `dead_code` suppression in the audited tree and `cargo fmt --all -- --check` succeeds.

## 7. Workspace verification

- [ ] 7.1 Run `cargo nextest run --workspace` and confirm all retained production-behavior tests pass; verify failures are triaged without weakening tests or adding sleeps/live external dependencies.
- [ ] 7.2 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace`, and `make check-code-file-lines`; verify the workspace is warning-free and no file exceeds the repository line-count gate.
- [ ] 7.3 Review the final diff against the issue acceptance criteria: every production-unreachable item and sole-purpose test is gone, every retained item is production-reachable and unsuppressed, the list/session conventions are removed where obsolete, and no new `dead_code` allowance or user-visible behavior change remains; verify the review checklist and the complete workspace gates all pass.

# Fix Browser Media-List Ownership Tasks

## 1. Characterize Browser ownership

- [x] 1.1 Add or update focused component/media-list tests proving stable-target ordinary refresh, local clamp, explicit re-anchor, bidirectional `ViewportAnchor` handoff (Wide↔Narrow preserving target plus offset), and retained-geometry hits including blank/header no-op; verify the targeted tests pass.
- [x] 1.2 Add Wide AND Normal/Narrow shell-tick integration coverage through `Application::tick()` and the shell sync pass proving wheel/click move the control with no `BrowserCursorIndex`-driven App recompute, one-painter per breakpoint (canonical paint count == 1, legacy painters == 0 on canonical paths), two-column Generic isolation (zero canonical-control output, canonical state untouched), retained-geometry hits, and a `ViewportAnchor` Wide↔Narrow round-trip preserving target plus offset; model on `tests_tick_integration_home.rs` post-`23291256`; verify the targeted tests pass.

## 2. Transfer Browser list authority

- [x] 2.1 Refactor `BrowserComponent` so the active persistent control solely owns live cursor and scroll: delete the parent cursor/scroll fields and all write-backs (movement echo, `apply_position`, `set_content`, `view()` tail, `claim_list_point`), make anchor transfer control-to-control only with no parent-field fallback, and switch `resolve_row_target` to point-only `resolve_current_point` / `current_selected_target` plus `current_detail_rect`; verify component tests cover movement and handoff without parent-mirror synchronization.
- [x] 2.2 Refactor Browser shell projection and render seam so the shell keeps only resting/restore state written from component-resolved values (identity-gated push seeds become explicit re-anchor; extras/poster prefetch seed from the control target), wheel/click keep focus plus nav-effect side calls without resting writes, canonical paints stop publishing `left_row_map` / `left_item_rows` / `left_sorted_indices` while legacy shims stay for other destinations, and non-hero two-column catalogs remain isolated screen-owned grid interaction; verify retained control geometry resolves hits and Browser-owned content/workspaces/pills/images/effects/persistence remain unchanged.

## 3. Verify the bounded repair

- [x] 3.1 Run relevant Browser component, render, and shell-tick tests at Wide and Normal/Narrow breakpoints; verify the base frame does not underpaint the mounted Browser surface and exactly one list painter runs per frame on canonical paths.
- [x] 3.2 Run `cargo fmt --all -- --check`, `cargo check -p mbv`, relevant `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; record any unrelated failure without widening this change.

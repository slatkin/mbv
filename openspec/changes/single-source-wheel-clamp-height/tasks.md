# Tasks

Package for every command: `mbv-components`. Test contracts are in `design.md` D1 to D4. Add no test beyond the ones named here.

## 1. Retained last content rectangle

- [x] 1.1 In `crates/mbv-components/src/list/paint.rs`, add `last_content_rect: Option<Rect>` to `PaintRetainedState`. `store_completed` sets it. `begin` and `invalidate` do not touch it. Add `pub fn last_content_rect(&self) -> Option<Rect>` with a doc line that says "survives invalidation, unlike `content_rect`". Verify: extend the existing `invalid_geometry_claims_no_point_or_selected_row` test (contract: retained paint state) with two asserts. After `begin()` and after `invalidate()`, `last_content_rect()` still equals the stored rectangle while `content_rect()` is `None`. Run `cargo nextest run -p mbv-components list::paint`.

## 2. ThreeLineFlatList and Sessions

- [x] 2.1 In `list/three_line.rs`, change `scroll_viewport(&mut self, delta: i64, painted_height: u16)` to `scroll_viewport(&mut self, delta: i64)`. Read the height from `self.paint.last_content_rect()` (`map_or(0, |r| r.height)`). Keep `visible_items(height).max(1)`. Update the doc comment. Verify: add one test in `list/three_line/tests.rs` that paints 10 items, calls `scroll_viewport` twice past the end with no repaint between, and asserts the offset equals the paint-time max offset (the second call must not fall back to a one-item viewport). Cite #928 in the test name. Run `cargo nextest run -p mbv-components list::three_line`.
- [x] 2.2 In `sessions.rs`, delete the `painted_content_area` field, its init, and its write in `view`. The wheel arm calls `self.list.scroll_viewport(delta)`. `content_area_for_test` returns `self.list.last_content_rect()`. Add a `last_content_rect` forwarding method on `ThreeLineFlatList` only if no existing public accessor serves it. Verify: `rg painted_content_area crates` returns nothing, and `cargo nextest run -p mbv-components sessions` passes with no test body changed except accessor call sites.

## 3. MediaList wheel clamp

- [ ] 3.1 In `media_list.rs`, thread a `viewport_len: usize` through the core operation path so the `Scroll` arm passes it to `scrolled_viewport_offset` in place of the literal `1`. The core's public `delegate_operation(op)` keeps its signature and passes `1`. Add the sibling entry that takes the length (crate-visible only). Update the `scroll_wheel` doc comment. Verify: `a_wheel_scroll_never_builds_the_row_flow` in `media_list/tests.rs` passes unchanged.
- [ ] 3.2 In `media_list/wide.rs`, make `WideMediaList::delegate_operation` call the sibling entry with the height of `self.paint.last_content_rect()` (`.max(1)`). Verify: add one test in the wide list's test module (contract: a wheel at the end leaves the last row at the bottom of the viewport, #928). Paint 20 rows into a 5-row content rectangle, apply `Scroll` until the offset stops moving, assert the offset is 15. Run `cargo nextest run -p mbv-components media_list`.

## 4. TreeBrowser

- [ ] 4.1 In `list/tree_browser.rs` and `list/tree_browser/render.rs`, delete the `last_painted` field, its init, and its write in `view`. In `list/tree_browser/operations.rs`, change `viewport_height` to `configured_geometry` content height, else `self.paint.last_content_rect()` height. Verify: `rg last_painted crates/mbv-components/src/list` returns nothing, and `cargo nextest run -p mbv-components tree_browser` passes with no test changed.

## 5. Spec sync and gates

- [ ] 5.1 Check `docs/architecture/interactive-surface-ledger.md` and `docs/architecture/tui-frontend.md` for text that names a per-surface wheel clamp height. Fix any hit. Verify: `rg -n "painted_height|painted_content_area|last_painted" docs openspec/specs` shows no stale reference.
- [ ] 5.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-components`. Verify all three pass.
- [ ] 5.3 Sync the delta into `openspec/specs/mouse-input/spec.md` with `/opsx:sync`. Verify: the boundary scenario in the main spec has the two added lines.

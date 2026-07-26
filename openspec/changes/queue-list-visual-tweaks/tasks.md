## 1. Remove queue panel top border row

- [x] 1.1 In `render_power_queue_panel_frame` (`src/app/render/power_widgets.rs`), delete the block that draws the `\u{2594}` ("▔") row at the top of `area`.
- [x] 1.2 Update the returned content `Rect`: `y` becomes `area.y` (was `area.y + 1`), and the reserved border-row count drops from `area.height.min(2)` to `1` (bottom `▁` row only) when `area.height > 0`.
- [x] 1.3 Leave the bottom-border (`\u{2581}` / "▁") drawing block untouched.
- [x] 1.4 Update the three tests in `src/app/render/tests_queue.rs` that assert the `\u{2594}` symbol at the queue panel's top row (`power_queue_panel_uses_selected_media_frame_and_background`, `power_queue_panel_remains_visible_when_unfocused`, `short_power_queue_panel_drops_padding_before_rows`) to match the new border-less top row and the one-row-taller content area.

## 2. Minimum group size of 3 for queue group headers

- [x] 2.1 In `build_queue_rows` (`src/app/ui_util.rs`), keep the existing grouping-key computation (album for audio, series for episodes) unchanged.
- [x] 2.2 Add a run-length gate so a `QueueRow::Header` (and its paired `QueueRow::Spacer`) is only emitted when the upcoming run of same-key items has length >= 3; runs of 1 or 2 render as plain `QueueRow::Track` rows with no header/spacer, same as ungrouped items.
- [x] 2.3 Confirm `group_for_header` stays populated 1:1 with actually-emitted headers (no stale/extra entries for suppressed short runs).
- [x] 2.4 Add unit tests covering: a run of 1 item (no header), a run of 2 items (no header), a run of exactly 3 items (header present), alongside existing 4-item-run coverage.
- [x] 2.5 Verify `queue_group_start_row` (`src/app/render/queue.rs`) and its existing tests still behave correctly when a short run has no header to snap to (should behave like today's ungrouped-item case).

## 3. Verification

- [x] 3.1 Run `cargo fmt --all -- --check`.
- [x] 3.2 Run the narrowest relevant tests: `cargo test` filtered to `render::power_widgets`, `render::queue`, `render::tests_queue`, and `ui_util`.
- [ ] 3.3 Manually verify in a live TUI session: queue panel renders with no top border row (bottom border still visible, one extra content row visible), and a queue containing runs of 1, 2, and 3+ same-album/series items shows a group header only for the 3+ run.

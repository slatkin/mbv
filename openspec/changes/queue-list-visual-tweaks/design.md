## Context

The queue panel is the left column of the power view. `render_power_queue_panel_frame` (`src/app/render/power_widgets.rs:247-288`) paints the panel's background, draws a `▔` (U+2594) row across the full width at the top of `area` and a `▁` (U+2581) row at the bottom, then returns a shrunk `Rect` (`y: area.y + 1`, `height: area.height.saturating_sub(border_rows)` where `border_rows = area.height.min(2)`) for the actual list content. That returned Rect is passed straight into `render_power_queue` (`src/app/render/queue.rs:145`) as its content area, via the call site in `src/app/render/mod.rs:413-414`.

Group headers are produced by `build_queue_rows` (`src/app/ui_util.rs:210-248`), which walks `items` and, whenever the grouping key changes (`album_id` for audio via `format!("a:{}", item.album_id)`, `series_name` for episodes via `format!("e:{}", item.series_name)`), emits a `QueueRow::Header` (plus a `QueueRow::Spacer` before it if there was a prior group) before the run of `QueueRow::Track` rows. There is currently no minimum run length — a single track that happens to be the only item with its album/series still gets its own header line. `build_power_queue_rows` (`power_widgets.rs:290-302`) wraps this to also insert a `Spacer` after each `Header`.

## Goals / Non-Goals

**Goals:**
- Remove the queue panel's top `▔` border row and reclaim that row as queue-list content, leaving the bottom `▁` border row untouched.
- Only show a group header for a run of 3+ consecutive same-key items; runs of 1-2 render as flat, header-less tracks (identical to how already-ungrouped items render).
- Keep both changes scoped to the queue list only — no changes to other panels, borders, or list styles.

**Non-Goals:**
- Not changing the bottom border, panel background, focus coloring, or any other panel's frame.
- Not changing the grouping *key* (still album for audio, series for episodes) or header label/style — only whether a header is emitted for a given run.
- Not changing scroll/cursor behavior beyond what naturally follows from headers disappearing for short runs (see Risks).

## Decisions

### 1. Remove the top border by simply not drawing it and adjusting the returned Rect
In `render_power_queue_panel_frame`, delete the block that renders the `\u{2594}` line (currently lines 260-266), and change the returned content Rect's `y`/`height` math so the top row is no longer reserved: content `y` becomes `area.y` (not `area.y + 1`), and `border_rows` becomes `1` when `area.height > 0` (only the bottom `▁` row is reserved) instead of `area.height.min(2)`. The bottom-border block (currently lines 267-279) is unchanged.

Alternative considered: keep drawing the row but fill it with background color / blank spaces instead of `▔` glyphs (i.e. "hide" it visually). Rejected per explicit instruction — the user wants the row itself gone, not just blanked, so the list should visibly gain a full row of content, not a blank spacer row.

### 2. Minimum-group-size-of-3 via a post-pass over the grouped runs, not a rewrite of the grouping key logic
`build_queue_rows` already computes contiguous runs keyed by `last_group_key`. The minimum-size rule is a *header-emission* gate, not a grouping-key change: keep the existing key computation exactly as-is, but only push `QueueRow::Header` (and its paired `Spacer`) when the run about to start has length >= 3. Concretely, this means the run length must be determined by look-ahead (peeking how many upcoming items share the new key) before deciding to emit a header for that run, since the header/spacer is written *before* the run's tracks are appended.

Simplest implementation shape: first pass groups items into runs (key, item indices) exactly as today; second pass emits `Header`+`Spacer` only for runs where `indices.len() >= 3`, then always emits the run's `Track` rows regardless of header. This avoids restructuring the streaming single-pass loop into something harder to follow, at the cost of one extra small pass over already-grouped data (runs, not items — negligible, queues are not large).

Alternative considered: track a small lookahead buffer inline in the existing single loop (peek up to 2 items ahead before deciding whether to flush a header). Rejected as more error-prone for equivalent benefit — the two-pass version keeps the existing key/label computation untouched and isolates the new rule to one clearly-named step, easier to reason about and test in isolation.

### 3. `group_for_header` stays 1:1 with emitted headers
No change needed to how `group_for_header` is consumed (`render/queue.rs:210-213` counts `QueueRow::Header` entries to index into it) — it already only needs one label per *emitted* header, so as long as the label vector is built in lockstep with the (now-conditional) header emission, existing indexing logic keeps working unmodified.

## Risks / Trade-offs

- [Three existing tests in `tests_queue.rs` (lines ~309, ~367, ~470) assert the `▔` symbol at the queue panel's top row] → Update these to assert the top row is no longer `▔` (either the first content row's expected character, or simply removing the border-specific assertion), in the same change that removes the border.
- [No existing test pins down 1-item or 2-item group behavior] → Add unit tests for `build_queue_rows`/`build_power_queue_rows` covering: a run of 1 (no header), a run of 2 (no header), and a run of exactly 3 (header present) alongside the existing 4-item-run coverage, so the new threshold is pinned precisely at the boundary.
- [`queue_group_start_row` (`render/queue.rs:15-21`) walks backward to the nearest `Header`/start-of-display when scrolling snaps upward; for a short run with no header, "group start" naturally becomes the run's first track, which is already correct today for ungrouped items — no code change needed there, just worth confirming with a test since short runs now behave like ungrouped runs.]
- [Reclaiming the top border row changes the exact `y` pixel row every existing queue-panel snapshot/position-based test expects] → Audit `tests_queue.rs` (and any home/list tests that assert absolute row offsets within the queue panel) for off-by-one row assumptions baked in before this change.

## Migration Plan

1. Remove the top-border draw + adjust the returned Rect in `render_power_queue_panel_frame`; update the three affected border-assertion tests.
2. Add the run-length->=3 gate to `build_queue_rows`; add new boundary tests (1, 2, 3, 4-item runs); update/verify `queue_group_start_row`-related tests still pass given header-less short runs.
3. Run `cargo fmt --all -- --check` and the narrowest relevant test modules (`render::power_widgets`, `render::queue`, `render::tests_queue`, `ui_util`).
4. Manually verify in a live TUI session: queue panel shows no top border row (bottom border still present), and a queue with runs of 1, 2, and 3+ same-album/series tracks shows headers only for the 3+ runs.

## Open Questions

None — both changes are narrowly scoped and confirmed with the user (queue-list only).

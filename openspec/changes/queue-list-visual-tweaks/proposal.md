## Why

Two small, unrelated visual issues in the queue panel (left column of the power view) are worth cleaning up together since both only touch `src/app/render/queue.rs` / `power_widgets.rs`:

1. The queue panel currently reserves a full row for a top border line drawn as a repeated `▔` (UPPER ONE EIGHTH BLOCK, U+2594) character. That row costs one line of visible queue content for a purely decorative line the user no longer wants — visually the panel looks better and shows one more queue row without it.
2. Group headers (e.g. "Artist: Album" for a run of same-album audio tracks, or a series name for a run of episodes) are currently shown for *any* run of consecutive same-key items, including runs of just 1 or 2 items. A header above a single track (or a pair) adds visual noise without meaningfully helping a user scan the queue — grouping only earns its keep once there are enough items to actually group.

## What Changes

- Remove the queue panel's top border row entirely (the `▔` line drawn by `render_power_queue_panel_frame` in `src/app/render/power_widgets.rs`). The bottom border row (`▁`) is unaffected and still renders. The row previously consumed by the top border is returned to the queue list's content area, so the visible list gains one row of height where the border used to be.
- Change the queue list's grouping logic (`build_queue_rows` in `src/app/ui_util.rs`) so a group header (and its associated spacer) is only emitted for a run of **3 or more** consecutive items sharing the same grouping key (album for audio, series for episodes). Runs of 1 or 2 items render as plain flat track rows with no header and no extra spacer, exactly like ungrouped items do today.
- This is a queue-list-only change: no other panel, list, or border (library list, album/series detail borders, other overlays) is affected.

## Capabilities

### New Capabilities
- `queue-list`: rendering behavior of the power-view queue list — its panel border rows and its grouping/header rules.

## Impact

- Affected code:
  - `src/app/render/power_widgets.rs` — `render_power_queue_panel_frame` (remove top `▔` row + border/content Rect math) and `build_power_queue_rows` (unaffected logic, but consumes the changed grouping output).
  - `src/app/ui_util.rs` — `build_queue_rows` grouping/header-emission logic (add minimum-run-length-of-3 condition).
  - `src/app/render/mod.rs` — call site of `render_power_queue_panel_frame` (no signature change expected, just behavior).
- Affected tests:
  - `src/app/render/tests_queue.rs` — three tests assert the `▔` top-border symbol at the queue panel's top row (`power_queue_panel_uses_selected_media_frame_and_background`, `power_queue_panel_remains_visible_when_unfocused`, `short_power_queue_panel_drops_padding_before_rows`); these need updating to reflect the removed top row (and the resulting one-row-taller content area).
  - `src/app/render/queue.rs` tests and any `build_queue_rows` tests — need new coverage for 1-item and 2-item runs (no header) alongside existing 3+/4+ item coverage (header present), since no test today pins down sub-3 group behavior.
- No persisted data, network protocol, or external API is affected — this is purely in-process TUI rendering/layout.

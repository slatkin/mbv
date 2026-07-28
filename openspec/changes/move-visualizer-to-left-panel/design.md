## Context

main view has a two-column layout: a narrow left panel (queue/card column, width controlled by `queue_column_width`) and a wider right panel (tab bar, player, library listing, status bar). The audio visualizer currently renders as an 11-row strip at the bottom of the right panel, carved out of the library content area via `split_visualizer_area`.

The left panel has unused vertical space below the queue list. Adding a second visualizer strip there reuses the existing render path with no new state.

## Goals / Non-Goals

**Goals:**
- Render a visualizer strip at the bottom of the left panel, same height (`VISUALIZER_HEIGHT = 11`) as the right-panel strip.
- Both strips respond to the same `visualizer_enabled` toggle and `visualizer_frame` data.
- No changes to the right-panel visualizer behaviour.

**Non-Goals:**
- Removing or relocating the existing right-panel visualizer.
- Independent toggle or separate frame data per panel.
- Resizing the left panel width.

## Decisions

**Reuse `split_visualizer_area` and `render_visualizer` as-is.**
The existing methods are width-agnostic — they scale bars to whatever `area.width` is passed. The left panel's narrower width will simply produce fewer bars. No new methods needed.
*Alternative considered:* a dedicated narrower render function — rejected because the existing code already handles arbitrary widths.

**Split the visualizer area from `left_area` (or `left_content`), not from a new source.**
The left panel's full column rect (`left_area`) or its padded content rect is the natural input. The split carves `VISUALIZER_HEIGHT` rows off the bottom, and the remaining area is used for the existing queue/card content. This mirrors exactly how the right panel splits `render_lib_area`.
*Alternative considered:* using a fixed position at the bottom of the screen — rejected because it wouldn't respect panel collapse or resize.

**Gate on `queue_column_collapsed`.**
When the queue column is collapsed (`left_w == 0`), no left-panel visualizer renders. This is consistent with the rest of the left panel being hidden.

## Risks / Trade-offs

- [Narrow width produces very few bars] → The existing bar-scaling logic in `render_visualizer` already handles this gracefully; at extreme narrow widths the visualizer may show 1-2 bars or nothing. Acceptable for step one.
- [Visual clutter with two identical strips] → This is the desired outcome per the user's request. Future iterations could differentiate them (e.g. different colour or mirror mode).

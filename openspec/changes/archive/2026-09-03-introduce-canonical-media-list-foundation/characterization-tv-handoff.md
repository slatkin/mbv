# TV breakpoint handoff characterization

Date: 2026-09-02
Change: `introduce-canonical-media-list-foundation`
Baseline: accepted HEAD `74bd1ce4`

## Source characterization

- `src/app/components/tv_workspace/mod.rs` stores the current cursor in `TvWorkspaceComponent.cursor` (line 36) and scroll offset in `.scroll` (line 37).
- `selected_item_id()` (lines 158–164) derives the selected target from the cursor's current item.
- The existing breakpoint path carries cursor/scroll through `BrowseLevel.resting` on Wide → Narrow and re-anchors on the layout flip back to Wide.
- No selected-row screen-row offset is currently stored; `ViewportAnchor` is not yet wired to the TV workspace.

## Live observation

The user ran the current app and confirmed that the view collapses as expected through the breakpoint transition. Exact column widths are tuning values, not a contract, and were intentionally not recorded. No visual selection or handoff issue was reported.

## Composition bar

Row 3.4 must preserve the selected target and relative selected-row placement while receiving geometry clamps the viewport. It must not make the breakpoint column widths part of the behavior contract.

No test or fixture was added or edited for this characterization.

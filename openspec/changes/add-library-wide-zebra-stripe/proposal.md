## Why

Queue zebra striping (change `add-media-list-zebra-stripe`, PR #719) proved the pattern: alternating row backgrounds carry the eye across a wide panel. The library's Wide lists have the same dense one-column rows and the same readability problem, in both the Browser pane and provider workspaces. Meanwhile 719 leaves the new gutter-selected style (bold focus-accent title, no selected background) behind a per-list opt-in flag — a caller-selected variant arm with exactly one user, which the `ui-design-system` spec names as a defect to remove, not extend. The follow-up should stripe the library and delete the option in the same motion.

## What Changes

- Enable zebra striping on both library Wide lists (Browser pane and provider workspace) with secondary focused `#48584e` and secondary unfocused `#2d353b`, caller-supplied on the paint policy per 719's D1.
- Make the gutter-selected style the unconditional Wide selected-row treatment: delete the `with_selected_gutter` opt-in, simplify the queue call site to zebra-only, and adopt no flag on the library side.
- Precondition (719 must land first, with its spec corrected): 719's implementation stripes the 1st/3rd visible items while its delta spec mandates 2nd/4th, and its delta spec still mandates "selected-row background always" which the gutter style violates. This change inherits 719's code as-merged; any remaining spec-truth gap is 719's to close before this change cites it.
- Out of scope: Narrow/Inline presentation (the inline hero replaces the selected row; unchanged); inline-search results, which stay on the legacy painters until issue #720 converts them to media_list rows.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `canonical-media-lists`: extend the Wide zebra-stripe requirement to the library Wide lists with their colour pair, and change the Wide selected-row requirement from selected-background to the gutter treatment (bold focus-accent title, no selected background, invisible when unfocused).

## Impact

- `src/app/components/library_panel/panel_list.rs` — both Wide policy arms gain the library zebra pair (single seam for both library Wide sites).
- `src/app/components/media_list/mod.rs` — `WideMediaListPaintPolicy` loses the `selected_gutter` opt-in; gutter becomes unconditional.
- `src/app/render/components/media_list/wide.rs`, `row.rs` — Wide adapter/painter stop threading the flag.
- `src/app/render/components/queue.rs` — drops `.with_selected_gutter()`, keeps its zebra pair.
- Render regression tests for library zebra parity and gutter-selected titles.

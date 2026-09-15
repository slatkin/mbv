## Context

See proposal.md — Why/What Changes for motivation and scope.

Today, `WideMediaListPaintPolicy` carries `focused: bool` and `selected_surface: SelectedRowSurface`. The `render_wide_media_list` function in `src/app/render/components/media_list/wide.rs` passes `selected_bg: Color` down to `media_list_row` in `row.rs`, which applies it only to the selected row (`ListItem::style(if selected { bg(selected_bg) } else { Style::default() })`). Unselected rows have no explicit background.

The row painter iterates display rows in viewport order but does not track which are selectable `Item` rows vs. structural `Heading`/`Spacer` rows for the purpose of alternating backgrounds.

## Decisions

### D1 — Zebra colours live on the paint policy, not the palette

The zebra secondary colours are caller-supplied on `WideMediaListPaintPolicy` rather than palette constants. Each list site may want different colours (queue vs. library), and the policy is already the closed semantic channel for per-list paint configuration. A `ZebraStripe { focused: Color, unfocused: Color }` struct on the policy keeps the API small and additive.

Alternative: palette constants. Rejected because it conflates a per-list opt-in with a global default and requires palette expansion for each new site.

### D2 — Screen-row item parity, not source-row parity

Zebra counts only selectable `Item` rows in the visible window, numbering them 0, 1, 2, … from the top. Odd-numbered items get the secondary background. This means the visual pattern is stable as you scroll — the second visible item is always striped.

Alternative: source-row parity (item N in the data is always striped). Rejected because scrolling by one row flips every row's stripe, which is visually jarring.

### D3 — Item counter passed into `media_list_row`

The render loop in `render_wide_media_list` maintains a running `item_index: usize` that increments only for selectable `Item` rows. It passes `item_index % 2 == 1` (or the resolved zebra colour) into `media_list_row`. The row painter receives an optional `zebra_bg: Option<Color>` and uses it as the `ListItem` style background for non-selected rows when present.

This avoids any new state on `MediaList` or `RowGeometry` — zebra is purely a paint-time concern.

### D4 — Selected row always wins

When the selected row falls on a zebra position, the selected-row background takes precedence. The existing `if selected { bg(selected_bg) }` branch fires first; the zebra `else` branch applies only to non-selected rows.

### D5 — Queue is the only initial consumer

Only `WideMediaListPaintPolicy::for_queue` will supply the zebra policy. The `new()` and `for_library_workspace()` constructors leave it `None`. Queue's focused zebra is `#3c4841`, unfocused is `#333c43`.

## Risks / Trade-offs

- [Zebra with structural rows] The queue currently has no `Heading`/`Spacer` rows, so the item-only counter is equivalent to a display-row counter. If structural rows are added to the queue later, the stripe pattern will skip them as specified — this is correct but might surprise someone expecting display-row alternation. → No mitigation needed; spec is explicit.

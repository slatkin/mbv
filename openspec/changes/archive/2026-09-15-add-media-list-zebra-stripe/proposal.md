## Why

Queue rows are visually uniform — every unselected row has the same transparent background, making it hard to track a row across the width of the panel. Zebra striping (alternating row backgrounds) is a standard readability aid for dense lists.

## What Changes

- Add an optional zebra-stripe policy to `WideMediaListPaintPolicy` that carries a focused and unfocused secondary background colour pair. Disabled by default.
- When enabled, the Wide row painter applies the secondary background to odd-numbered visible selectable `Item` rows (screen-row parity among items, not display-row index). Headings and Spacers always paint with no background regardless of zebra.
- Enable zebra striping on the Queue list only, with secondary focused `#3c4841` and secondary unfocused `#333c43`.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `canonical-media-lists`: Add a requirement that the Wide presentation supports an optional per-list zebra-stripe policy with caller-supplied secondary colour pair, applying the alternate background to odd selectable items by screen-row parity.

## Impact

- `src/app/components/media_list/mod.rs` — `WideMediaListPaintPolicy` gains an optional zebra field.
- `src/app/render/components/media_list/wide.rs` — `render_wide_media_list` threads the zebra colour to the row painter.
- `src/app/render/components/media_list/row.rs` — `media_list_row` applies the alternate background on qualifying rows.
- Queue caller site — passes `with_zebra(...)` on its `WideMediaListPaintPolicy::for_queue` construction.
- `src/app/palette.rs` or the queue caller — two new colour constants for the queue zebra pair.

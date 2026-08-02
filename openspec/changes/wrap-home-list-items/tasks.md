## 1. Model Home item display layout

- [ ] 1.1 Inspect the existing Home row construction and define a display-layout representation that retains the logical item index, wrapped physical lines, item height, styles, marker placement, and duration placement.
- [ ] 1.2 Preserve the existing rendered content and styles for Music and all non-episode items; do not add artist, album, year, or other metadata.
- [ ] 1.3 Implement non-episode wrapping with one row when the existing label fits and indented continuation rows otherwise.
- [ ] 1.4 Implement episode measurement: retain the current inline series/title rendering when it fits, and use the stacked series/title layout only when it does not.

## 2. Render variable-height Home rows

- [ ] 2.1 Render each item's complete physical span, placing the marker only on the first line and aligning continuation lines with the existing content column.
- [ ] 2.2 Keep duration on the first line of the applicable label block, right-aligned with the current spacing, without repeating it on continuation lines.
- [ ] 2.3 Extend focused selection backgrounds and any row decorations across the item's full height.
- [ ] 2.4 Preserve the existing empty-section rendering as a one-row `(empty)` item.

## 3. Synchronize scrolling and interaction

- [ ] 3.1 Compute total Home content height as the sum of physical item heights and use the actual list width so wrapping reflows on resize.
- [ ] 3.2 Update cursor visibility and page/scroll calculations to use each selected item's cumulative physical top and bottom rather than assuming a one-row span.
- [ ] 3.3 Keep the physical scroll offset, viewport, and scrollbar thumb synchronized with the variable-height content.
- [ ] 3.4 Record hitboxes that cover every physical row of an item so mouse clicks on continuation lines select the correct logical item.
- [ ] 3.5 Verify partially visible items at the top and bottom of the viewport render and interact correctly.
- [ ] 3.6 Define and implement oversized-item behavior: when a selected item exceeds the viewport, keep its marker/first row visible while allowing physical scrolling through its continuation rows.
- [ ] 3.7 Resolve scrollbar-gutter width before final wrapping with a deterministic measurement pass, then use that final width consistently for height, rendering, hitmaps, and scrollbar geometry.

## 4. Add focused rendering coverage

- [ ] 4.1 Test a non-episode label that fits and confirm it remains one row with its existing content.
- [ ] 4.2 Test a non-episode label that wraps and verify complete text, continuation indentation, marker placement, and first-line duration.
- [ ] 4.3 Test an episode whose existing inline representation fits and verify the one-row inline form remains unchanged.
- [ ] 4.4 Test an episode that does not fit and verify the stacked series/title form, colors, complete text, duration, and full-height selection background.
- [ ] 4.5 Test a Music item using the existing default representation and verify wrapping changes layout only, not displayed fields or meaning.
- [ ] 4.6 Test narrow and resized list widths, including the scrollbar fit boundary and the extreme-width duration fallback, without truncating label content.
- [ ] 4.7 Test a selected item taller than the viewport and verify the marker row remains reachable while continuation rows can be revealed by physical scrolling.

## 5. Verify

- [ ] 5.1 Run the focused Home renderer tests.
- [ ] 5.2 Run `cargo fmt --all -- --check`.
- [ ] 5.3 Run `cargo check --workspace --all-targets` and the relevant workspace test command.

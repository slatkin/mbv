## 1. Two-Column Layout in render_power_home_list

- [x] 1.1 Gate layout selection on `area.width >= 80` in `render_power_home_list()`
- [x] 1.2 In two-column mode: compute `hero_col_width` as `(area.width * 2 / 5)` with existing min/max clamps, and `list_col_width` as remaining width
- [x] 1.3 Position hero area in left column (`content_area` left edge, full content height) and list area in right column (`content_area` left edge + hero width + gap, full content height)
- [x] 1.4 In two-column mode: hero image fills available vertical space below metadata (flip the existing image-right metadata-left arrangement)
- [x] 1.5 In vertical mode (width < 80): unchanged behavior — hero on top, list below
- [x] 1.6 Pill row rendering is unchanged (already full-width at top)

## 2. Verification

- [x] 2.1 Run existing home render tests to confirm no regression
- [ ] 2.2 Visually verify: wide terminal (>= 80 col right panel) shows hero on left, list on right
- [ ] 2.3 Visually verify: narrow terminal (< 80 col right panel) preserves existing hero-above-list-below layout
- [ ] 2.4 Visually verify: scrolling the list in two-column mode works identically
- [ ] 2.5 Visually verify: cursor highlight moves between list items correctly in two-column mode
- [ ] 2.6 Visually verify: pill selection changes update hero and list for both layouts

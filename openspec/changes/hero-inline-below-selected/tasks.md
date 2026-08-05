# Tasks: hero inline, just below the selected item

## 1. Compute the selected item's display row

- [x] 1.1 In `render_power_list` (src/app/render/list.rs), after
      the row packing for the top section, identify the display
      row index of the row containing the cursor. This is the
      position the hero will be inserted below.
- [x] 1.2 If the list is empty or the cursor is past the end, no
      hero is painted (the list takes the full content area).
- [x] 1.3 The selected item's display row is computed in the same
      coordinate system as the row map (a `Vec<Option<usize>>` of
      length `content_area.height`).

## 2. Render the top section (items above and including selected)

- [x] 2.1 In `render_power_list`, before painting the hero, call
      the row renderer to paint rows from item 0 through the
      row containing the cursor. This is the "top section."
- [x] 2.2 The top section uses the same `cols` (1 or 2) as the
      list would without the hero.
- [x] 2.3 The top section ends with the row containing the cursor.
      The cursor's display row within the top section is the
      position the hero goes below.

## 3. Paint the hero below the selected row

- [x] 3.1 Compute the hero's position: `hero_y = top_section_bottom_y`
      (the row just below the cursor's row in the top section).
- [x] 3.2 Compute the hero's height: `hero_height_for_width(content_width)`
      (the same formula as the top-hero design).
- [x] 3.3 Compute the hero's `Rect`: `x = content_area.x,
      y = hero_y, width = content_area.width, height = hero_height`.
- [x] 3.4 Call `render_power_compact_detail` (or equivalent) with
      the selected item and the hero's `Rect`. The hero is
      full content width.
- [x] 3.5 Store the hero's `Rect` in `layout.hero_area` (same
      field as the top-hero design, different value).

## 4. Render the bottom section (items below selected)

- [x] 4.1 In `render_power_list`, after painting the hero, call
      the row renderer to paint rows from the item after the
      cursor's row to the end of the list. This is the
      "bottom section."
- [x] 4.2 The bottom section's `Rect` starts at
      `y = hero_y + hero_height` and extends to the bottom of
      the content area.
- [x] 4.3 The bottom section's `x` and `width` are the same as
      the top section's (full content area).
- [x] 4.4 The bottom section uses the same `cols` as the top
      section.
- [x] 4.5 The row map entries for the bottom section are
      offset by `top_section_height + hero_height` display
      rows.

## 5. Update the row map

- [x] 5.1 The row map (`left_row_map`) has `None` entries for
      the hero rows. A click on a hero row hits the hero (not
      an item).
- [x] 5.2 The row map's item entries are at the display rows
      where they actually paint (top section: 0..N; bottom
      section: N+hero_height..).
- [x] 5.3 The `left_row_targets` (the per-row click targets)
      is updated to match.

## 6. Update auto-scroll

- [x] 6.1 The list's auto-scroll must account for the hero
      height. When the cursor + hero don't fit, scroll so
      both are visible.
- [x] 6.2 The minimum visible height for the cursor is the
      cursor's row + 1 + hero_height (the cursor's row, plus
      one row gap, plus the hero). The auto-scroll should
      bring all of these into view.

## 7. Drop the top-hero logic

- [x] 7.1 Remove the hero-at-top code from `render_power_list`.
      The hero is no longer at the top of the content area.
- [x] 7.2 Remove the `HERO_AT_TOP` / `hero_area_at_top` style
      branches. The hero is always inline now.
- [x] 7.3 The `list_area` is no longer split into hero + list;
      the entire content area is used (with the hero inserted
      in the middle).

## 8. Update mouse handling

- [x] 8.1 Verify that `click_set_cursor` in `input_mouse.rs`
      handles the new hero position correctly. The existing
      check `layout.main.hero_area.contains((col, row))`
      should still work if `hero_area` is set to the inline
      hero's rect.
- [x] 8.2 No code change expected if the check is position-
      agnostic (it just checks if the click is inside the
      stored `hero_area` rect).

## 9. Update tests

- [x] 9.1 In `src/app/render/list_tests.rs`, update the
      hero-on-top tests to assert the inline position:
      - `hero_paints_above_list_area_in_two_column_mode`
        → renamed to `hero_paints_below_selected_row_in_two_column_mode`,
        updated assertions (hero is below the selected row,
        list wraps above and below).
      - `hero_height_scales_with_content_width` → kept, but
        updated assertions (the hero position varies with the
        cursor, not with the content area).
      - `selected_cell_uses_carat_and_double_hash_in_two_column_mode`
        → kept, unchanged.
- [x] 9.2 Add a new test:
      `hero_follows_cursor_when_cursor_moves`. Render the list
      with cursor=0, verify the hero is below row 0. Render
      with cursor=5, verify the hero is below the row
      containing item 5. The list above and below the hero
      should reflect the cursor position.
- [x] 9.3 Add a new test:
      `row_map_has_none_entries_for_hero_rows`. Render the
      list, inspect the row map. Verify that:
      - The display rows of the top section have Some(item_idx)
      - The hero rows have None
      - The display rows of the bottom section have Some(item_idx)
      The hero rows should be exactly `top_section_height` to
      `top_section_height + hero_height - 1`.
- [x] 9.4 Add a new test:
      `auto_scroll_keeps_cursor_and_hero_visible`. Set the
      cursor near the bottom of a long list, render, verify
      the cursor and hero are both visible (not scrolled
      off).
- [x] 9.5 The maintenance invariant test
      (`one_and_two_column_render_the_same_per_cell_content`)
      still applies; updated for the new layout.

## 10. Verify

- [x] 10.1 Run `cargo fmt --all -- --check`.
- [x] 10.2 Run `cargo check --workspace --all-targets`.
- [x] 10.3 Run `cargo test --workspace`.
- [ ] 10.4 Visual verification in a real terminal at several
      widths (60, 82, 100, 150). Confirm the hero appears
      below the selected row, the list wraps around it,
      and the cursor + hero stay visible when scrolling.
- [ ] 10.5 Visual verification with the queue column collapsed
      and expanded. The hero's inline position should not
      depend on queue state.
- [ ] 10.6 Verify the hero follows the cursor smoothly as
      the user moves up/down through the list.

## Out of scope

- Home view refactor
- Music group view
- Feed home video group view
- 2-col packing math (unchanged)
- hjkl nav (unchanged)
- The 82-col threshold (unchanged)
- The maintenance rule and invariant test framework (unchanged)
- Smooth scrolling of the hero with the cursor (jump only)
- The top-hero branch (kept alongside for comparison)

# Tasks: hero on top of the library list

## 1. Split content_area into hero_area and list_area

- [x] 1.1 In `render_power_list` (src/app/render/list.rs), after
      computing `content_area`, split it vertically into `hero_area`
      and `list_area` based on the selected item's banner height.
- [x] 1.2 Compute the hero height using the formula in design.md
      decision 2: `image_height = max(1, (hero_width * 9 + 31) / 32)`,
      plus a meta block of 5 rows, plus a 1-row gap. Apply the cap
      (decision 3): clamp `image_height` to 12 rows max.
- [x] 1.3 Reserve `hero_area` and pass `list_area` (not the full
      `content_area`) to the row renderer and to the `ListRenderCtx`.
- [x] 1.4 Update `layout.left_area` to be `list_area` (the row renderer
      no longer paints the hero, so the layout's "left" area is the
      list, not the entire content area).

## 2. Render the hero above the list

- [x] 2.1 In `render_power_list`, after the hero area is reserved, call
      `render_power_compact_detail` (or equivalent) with the selected
      item and the `hero_area` rect. The existing function takes a
      `panel_width` parameter; passing the hero's width (the full
      `content_area.width`) is sufficient — no function change needed.
- [x] 2.2 Verify the hero paints at the top of the content area,
      full content width, with the image at 16:9 aspect ratio and the
      meta block below it.
- [x] 2.3 Confirm the hero is hidden (or shows a placeholder) when
      `power_selected_movie_item` returns None (e.g. an empty list).

## 3. Strip the inline selected-block from the row renderers

- [x] 3.1 In `render_power_plain_rows` (src/app/render/list_plain.rs),
      remove the `selected_block_bounds` parameter and the
      `render_selected_block_background` / `render_selected_block_borders`
      calls.
- [x] 3.2 In `render_power_letter_grouped_rows`
      (src/app/render/list_letter_groups.rs), do the same.
- [x] 3.3 Replace the selected-cell tab visual with: `▌` mark on the
      left edge + `##` prefix in the title. The `MEDIA_SELECTED_BG`
      color is no longer used for the selected cell — the cell uses
      the ordinary list bg.
- [x] 3.4 Update the `ListRenderCtx` struct in
      `src/app/render/list_rows.rs` to remove the banner-related
      fields (`banner_rows`, `banner_content_rows`,
      `series_detail_rows`, `selected_block_bounds`).

## 4. Drop the inline series detail

- [x] 4.1 In `render_power_list`, remove the `series_detail_rows`
      computation and reservation. The series detail is no longer
      inline; it's in the top hero.
- [x] 4.2 Confirm the top hero handles Series items (it should —
      `compact_banner_layout_with_overview` already supports
      `collection_type != "movies"`).

## 5. Update mouse handling

- [x] 5.1 In `src/app/input_mouse.rs`, add a hit test: a click inside
      `hero_area` (or any click in the list pane that is above the
      list area) is an Enter equivalent — fire the same action as
      Enter on the selected item.
- [x] 5.2 Keep the existing list-cell click behavior unchanged.

## 6. Update tests

- [x] 6.1 In `src/app/render/list_tests.rs`, drop the notched-block
      tests that are now invalid:
      - `selected_block_notches_left_and_right_column_selections`
      - `one_column_selected_block_remains_a_single_rectangle`
      - `unfocused_selected_block_uses_the_unfocused_background_on_tab_and_panel`
      Keep the per-cell tests (truncation, packing, cursor math) and
      the invariant test.
- [x] 6.2 The invariant test
      (`one_and_two_column_render_the_same_per_cell_content`) still
      applies; update it to compare the list_area at width 81 and 82.
- [x] 6.3 Add a new test: `hero_paints_above_list_area_in_two_column_mode`.
      Render a library at width 82, verify the hero area has the
      selected item's content (image + meta), the list area below it
      has the 2-col packed rows, and the row at `hero_y + hero_height`
      is the first list row.
- [x] 6.4 Add a new test: `hero_height_scales_with_content_width`.
      Render a library at widths 60, 82, 100, 150. Verify the hero
      height is bounded (≤ 18 rows with the cap) and the list area
      has at least 1 row in each case.
- [x] 6.5 Add a new test: `selected_cell_uses_carat_and_double_hash_in_two_column_mode`.
      Render a library with cursor=0 at width 82. Verify the left
      cell's title starts with `##` and has a `▌` mark on the left
      edge of the cell.

## 7. Verify

- [x] 7.1 Run `cargo fmt --all -- --check`.
- [x] 7.2 Run `cargo check --workspace --all-targets`.
- [x] 7.3 Run the library list renderer tests:
      `cargo test -p mbv --bin mbv library_column_width list_tests panel_tests`
- [ ] 7.4 Visual verification in a real terminal at several widths:
      60, 82, 100, 150. Confirm the hero reads as "above the list"
      and the list below reads as a clean 1-or-2-col grid.
- [ ] 7.5 Visual verification with the queue column collapsed and
      expanded, at default width. Confirm the hero position doesn't
      depend on queue state.
- [ ] 7.6 If the image cap or hero meta line don't read well, iterate
      on the design decisions 2 and 3.

## Out of scope

- Home view refactor
- Music group view
- Feed home video group view
- 2-col packing math (unchanged)
- hjkl nav (unchanged)
- The 82-col threshold (unchanged)
- The maintenance rule and invariant test framework (unchanged)

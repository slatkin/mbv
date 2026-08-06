# Tasks: hero on the bottom of the library list

## 1. Split content_area into list_area and hero_area

- [ ] 1.1 In `render_power_list` (src/app/render/list.rs), after
      computing `content_area`, split it vertically into `list_area`
      (top) and `hero_area` (bottom) based on the selected item's
      banner height.
- [ ] 1.2 Compute the hero height using the formula in design.md
      decision 2: `image_height = max(1, (hero_width * 9 + 31) / 32)`,
      plus a meta block of 5 rows, plus a 1-row gap. Apply the cap
      (decision 3): clamp `image_height` to 12 rows max.
- [ ] 1.3 Reserve `hero_area` at the bottom of `content_area` and pass
      `list_area` (not the full `content_area`) to the row renderer
      and to the `ListRenderCtx`.
- [ ] 1.4 Update `layout.left_area` to be `list_area` (the row
      renderer no longer paints the hero, so the layout's "left" area
      is the list, not the entire content area).

## 2. Render the hero below the list

- [ ] 2.1 In `render_power_list`, after the list has rendered, call
      `render_power_compact_detail` (or equivalent) with the selected
      item and the `hero_area` rect. The existing function takes a
      `panel_width` parameter; passing the hero's width (the full
      `content_area.width`) is sufficient — no function change needed.
- [ ] 2.2 Verify the hero paints at the bottom of the content area,
      full content width, with the image at 16:9 aspect ratio and the
      meta block below it.
- [ ] 2.3 Confirm the hero is hidden (or shows a placeholder) when
      `power_selected_movie_item` returns None (e.g. an empty list).
- [ ] 2.4 Confirm the hero always shows the cursor's item regardless
      of `list_area`'s scroll offset (design.md decision 5) — scroll
      the list so the selected row is off-screen and check the hero
      content doesn't change or disappear.

## 3. Strip the inline selected-block from the row renderers

- [ ] 3.1 In `render_power_plain_rows` (src/app/render/list_plain.rs),
      remove the `selected_block_bounds` parameter and the
      `render_selected_block_background` / `render_selected_block_borders`
      calls.
- [ ] 3.2 In `render_power_letter_grouped_rows`
      (src/app/render/list_letter_groups.rs), do the same.
- [ ] 3.3 Replace the selected-cell tab visual with: `▌` mark on the
      left edge + `##` prefix in the title. The `MEDIA_SELECTED_BG`
      color is no longer used for the selected cell — the cell uses
      the ordinary list bg.
- [ ] 3.4 Update the `ListRenderCtx` struct in
      `src/app/render/list_rows.rs` to remove the banner-related
      fields (`hero_rows`, `selected_block_bounds`, and any
      `DisplayRow::Hero` handling — the hero is no longer inline).

## 4. Drop the inline series detail

- [ ] 4.1 In `render_power_list`, remove the `series_detail_rows`
      computation and reservation. The series detail is no longer
      inline; it's in the bottom hero.
- [ ] 4.2 Confirm the bottom hero handles Series items (it should —
      `compact_banner_layout_with_overview` already supports
      `collection_type != "movies"`).

## 5. Update mouse handling

- [ ] 5.1 In `src/app/input_mouse.rs`, add a hit test: a click inside
      `hero_area` is an Enter equivalent — fire the same action as
      Enter on the selected item.
- [ ] 5.2 Keep the existing list-cell click behavior unchanged.

## 6. Update tests

- [ ] 6.1 In `src/app/render/list_tests.rs`, drop the notched-block
      tests that are now invalid (same set `hero-on-top` dropped):
      - `selected_block_notches_left_and_right_column_selections`
      - `one_column_selected_block_remains_a_single_rectangle`
      - `unfocused_selected_block_uses_the_unfocused_background_on_tab_and_panel`
      Keep the per-cell tests (truncation, packing, cursor math) and
      the invariant test.
- [ ] 6.2 The invariant test
      (`one_and_two_column_render_the_same_per_cell_content`) still
      applies; update it to compare the `list_area` at width 81 and 82.
- [ ] 6.3 Add a new test: `hero_paints_below_list_area_in_two_column_mode`.
      Render a library at width 82, verify the list area has the
      2-col packed rows starting at the top of `content_area`, the
      hero area sits at `content_area.y + content_area.height -
      hero_height` with the selected item's content (image + meta),
      and the hero is the last thing painted (bottom edge).
- [ ] 6.4 Add a new test: `hero_height_scales_with_content_width`.
      Render a library at widths 60, 82, 100, 150. Verify the hero
      height is bounded (≤ 18 rows with the cap) and the list area
      has at least 1 row in each case.
- [ ] 6.5 Add a new test: `selected_cell_uses_carat_and_double_hash_in_two_column_mode`.
      Render a library with cursor=0 at width 82. Verify the left
      cell's title starts with `##` and has a `▌` mark on the left
      edge of the cell.
- [ ] 6.6 Add a new test: `hero_content_tracks_cursor_when_selection_scrolled_offscreen`.
      Render a long library, move the cursor past the visible window
      so the selected row scrolls out of `list_area`, and verify the
      hero still shows that item's content unchanged.

## 7. Verify

- [ ] 7.1 Run `cargo fmt --all -- --check`.
- [ ] 7.2 Run `cargo check --workspace --all-targets`.
- [ ] 7.3 Run the library list renderer tests:
      `cargo test -p mbv --bin mbv library_column_width list_tests panel_tests`
- [ ] 7.4 Visual verification in a real terminal at several widths:
      60, 82, 100, 150. Confirm the hero reads as "below the list" and
      the list above reads as a clean 1-or-2-col grid with no gap
      relative to the tab bar above it.
- [ ] 7.5 Visual verification with the queue column collapsed and
      expanded, at default width. Confirm the hero position doesn't
      depend on queue state.
- [ ] 7.6 If the image cap or hero meta line don't read well, iterate
      on design decisions 2 and 3.

## Out of scope

- Home view refactor
- Music group view
- Feed home video group view
- 2-col packing math (unchanged)
- hjkl nav (unchanged)
- The 82-col threshold (unchanged)
- The maintenance rule and invariant test framework (unchanged)

## Housekeeping

- [ ] 8.1 Once this direction is confirmed, archive or delete
      `openspec/changes/hero-on-top/` on `main` — its tasks.md reads
      as complete but the code was only ever merged to
      `try/hero-on-top`, not `main`; left as-is it misrepresents
      current intent (AGENTS.md: "Shipped plans get deleted — stale
      ones read as current intent").

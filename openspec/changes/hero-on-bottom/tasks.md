# Tasks: hero on the bottom of the library list

## 1. Split content_area into list_area and hero_area

- [ ] 1.1 In `render_power_list` (src/app/render/list.rs), after
      computing `content_area`, split it vertically into `list_area`
      (top) and `hero_area` (bottom) based on the selected item's
      banner height. Delete the `hero_rows` calc block that currently
      does this inline (list.rs:244-263) and the `▁`/`▔` border-paint
      block that currently paints into blank filler rows (list.rs:311-403)
      — both are replaced by this split.
- [ ] 1.2 Reuse the existing `hero_height_for_width` function and its
      constants (`HERO_IMAGE_CAP_ROWS = 12`, `HERO_GAP_ROWS = 1`,
      `HERO_META_ROWS = 5`, `HERO_TITLE_ROWS = 1`,
      `HERO_BLOCK_EXTRA_ROWS = 4`) unchanged — the formula and the cap
      are already shipped; this task is wiring, not a new calculation.
      For a selected Series, use `series_inline_detail_rows` instead,
      same branch `render_power_list` already has today.
- [ ] 1.3 Reserve `hero_area` at the bottom of `content_area` and pass
      `list_area` (not the full `content_area`) to the row renderer
      and to the `ListRenderCtx`. Remove the `hero_rows` field from
      `ListRenderCtx` (list_rows.rs) entirely — the row renderer no
      longer needs to know about the hero at all.
- [ ] 1.4 Update `layout.left_area` to be `list_area` (the row
      renderer no longer paints the hero, so the layout's "left" area
      is the list, not the entire content area).

## 2. Render the hero below the list

- [ ] 2.1 In `render_power_list`, after the list has rendered, call
      `render_power_compact_detail` for a selected Movie or
      `render_series_inline_detail` for a selected Series (same branch
      as today) with the `hero_area` rect. Both existing functions take
      a `panel_width`/area parameter already; passing the hero's width
      (the full `content_area.width`) is sufficient — no function
      change needed.
- [ ] 2.2 Verify the hero paints at the bottom of the content area,
      full content width, with the image at 16:9 aspect ratio and the
      meta block below it.
- [ ] 2.3 Confirm the hero is hidden (or shows a placeholder) when
      `power_selected_movie_item` returns None (e.g. an empty list).
- [ ] 2.4 Confirm the hero always shows the cursor's item regardless
      of `list_area`'s scroll offset (design.md decision 5) — scroll
      the list so the selected row is off-screen and check the hero
      content doesn't change or disappear.

## 3. Strip the inline hero filler rows from the row renderers

- [ ] 3.1 In `render_power_plain_rows` (src/app/render/list_plain.rs),
      remove the `hero_rows` parameter and the `DisplayRow::Hero`
      filler-row insertion (list_plain.rs:58-71) and its use in the
      auto-scroll lower-bound calc and hero-rect publish
      (list_plain.rs:171-176).
- [ ] 3.2 In `render_power_letter_grouped_rows`
      (src/app/render/list_letter_groups.rs), do the same
      (list_letter_groups.rs:104-117, 237-242).
- [ ] 3.3 Remove the `DisplayRow::Hero` variant from
      `src/app/render/list_rows.rs` (no longer constructed anywhere).
      In `build_list_row_spans`, replace the selected-cell `▍` grabber
      + `PLAYBACK_PANEL_BG` background with: a `▌` mark on the left
      edge + `##` prefix in the title, on the ordinary list background
      (no `on_block` bg override). `MEDIA_SELECTED_BG` stops being
      referenced from the row renderers entirely — it's only used by
      the hero fill in `render_power_list` now.
- [ ] 3.4 Update the `ListRenderCtx` struct in
      `src/app/render/list_rows.rs` to remove the `hero_rows` field —
      the row renderer no longer reserves space for the hero at all.

## 4. Hero sizing for a selected Series

- [ ] 4.1 In `render_power_list`, the `hero_area` height calc (task
      1.2) already branches on selected-movie vs. selected-series, same
      as today's `hero_rows` calc did. No separate "series_detail_rows"
      step exists to remove — confirm there isn't dead code left behind
      from the old `hero_rows` branch after 1.1-1.3 land.
- [ ] 4.2 Confirm the bottom hero handles Series items via
      `render_series_inline_detail` (season pills + episode table),
      not the generic movie compact banner — these are different
      functions today (`detail_series_view.rs` vs. `detail.rs`), unlike
      what an earlier draft of this doc assumed.

## 5. Update mouse handling

- [ ] 5.1 In `src/app/input_mouse.rs`, add a hit test: a click inside
      `hero_area` is an Enter equivalent — fire the same action as
      Enter on the selected item.
- [ ] 5.2 Keep the existing list-cell click behavior unchanged.

## 6. Update tests

- [ ] 6.1 `src/app/render/list_tests.rs` has 4 tests today (item
      packing, letter buckets, cursor wrap/clamp, mouse-click-selects-
      cell); none of them assert on a notched block or an invariant
      test — both were already removed in #448 as brittle
      layout-internals tests. Check whether any of the 4 existing tests
      assert on `hero_rows` / `DisplayRow::Hero` filler-row counts
      (the cursor wrap/clamp math in list_plain.rs currently accounts
      for `hero_rows` in its lower-bound calc) and update those, rather
      than looking for tests that don't exist.
- [ ] 6.2 Add a new test enforcing the 1-col/2-col parity invariant:
      render the same library at width 81 and 82, compare `list_area`'s
      per-cell content (title, duration, marker), modulo cell-width
      truncation and the right cell's trailing-column absorption. This
      restores the guarantee `one_and_two_column_render_the_same_per_cell_content`
      gave before it was deleted — write it against `list_area`, not
      `content_area`, since the hero now lives outside the list rect.
- [ ] 6.3 Add a new test: `hero_paints_below_list_area_in_two_column_mode`.
      Render a library at width 82, verify the list area has the
      2-col packed rows starting at the top of `content_area`, the
      hero area sits at `content_area.y + content_area.height -
      hero_height` with the selected item's content (image + meta),
      and the hero is the last thing painted (bottom edge).
- [ ] 6.4 Add a new test: `hero_height_is_constant_above_the_image_cap`.
      Render a library at widths 60, 82, 100, 150. Verify the hero
      height is bounded (≤ ~23 rows given `HERO_IMAGE_CAP_ROWS = 12` +
      title + gap + meta + border rows) and, per decision 2, is the
      *same* height at 82/100/150 since the image cap already kicks in
      well below 82 columns — don't assert it grows with width. Verify
      the list area has at least 1 row in each case.
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
- The maintenance rule itself (list = one renderer parameterized by
  `cols`, unchanged) — though its regression test needs adding fresh,
  see task 6.2

## Housekeeping

- [ ] 8.1 Once this direction is confirmed, archive or delete
      `openspec/changes/hero-on-top/` on `main` — its tasks.md reads
      as complete but the code was only ever merged to
      `try/hero-on-top`, not `main`; left as-is it misrepresents
      current intent (AGENTS.md: "Shipped plans get deleted — stale
      ones read as current intent").
- [ ] 8.2 Once this change ships, archive
      `openspec/changes/hero-inline-below-selected/` too — it documents
      the design this change replaces (#448's inline-below-cursor
      hero), so leaving it as-is on `main` after the bottom hero lands
      would misrepresent current intent the same way.

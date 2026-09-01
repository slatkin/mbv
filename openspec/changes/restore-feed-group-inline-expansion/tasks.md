## 1. Pin the defect before fixing it

- [ ] 1.1 In `src/app/tests_narrow_browse_migration.rs`, extend
      `feed_home_video_group_app()` with a metadata-bearing variant: give the
      selected video `runtime_ticks`, `genre`, and an `overview` long enough to
      wrap at 60x20. Verify the new fixture produces a
      `feed_snapshot`-style full-frame capture and that the existing
      `FEED_NARROW_BASELINE` assertion now FAILS on it (proves the old
      baselines were vacuous). Commit the failing expectation as a
      characterization of the bug, not as a passing test.
- [ ] 1.2 Add `feed_home_video_group_expands_selected_row_narrow` asserting,
      on the metadata-bearing fixture at 60x20: the selected title appears
      exactly once as a plain row, the framed block's `▁`/`▔` rows exist, and
      the banner meta line (`Family` + duration fragment) is present. Verify
      it fails with the current `selected_h = 1`.

## 2. Publish the expansion height shell-side (D1)

- [ ] 2.1 Add `feed_selected_height: u16` to `NarrowBrowseExtras`
      (`src/app/components/browser_narrow.rs`) with a doc comment naming the
      formula `banner.content_rows() + 5` and why it is computed shell-side
      (image-cache access; components issue no effects while painting).
      Verify: `rtk cargo check -p mbv --all-targets` clean apart from the
      field's unused-warning, resolved by 2.2.
- [ ] 2.2 In `App::narrow_browse_extras`
      (`src/app/render/components/list_narrow.rs`), compute
      `feed_selected_height` for the `is_feed_home_video_group_view` branch
      from the already-built `CompactBannerLayout`, using the picker's panel
      width (`text_w - 2 * SELECTED_BLOCK_SIDE_PADDING`), and leave it `0`
      otherwise. Verify: a unit test asserting the published height equals
      `content_rows() + 5` for the fixture item (fails if it reads the
      generic path's `content_rows_with_title` value instead).

## 3. Fix the Normal painter

- [ ] 3.1 Replace `let selected_h = 1;` in `render_feed_group_picker_content`
      with `extras.feed_selected_height.max(1)`, keeping the existing
      `render_compact_detail_with_ctx` call untouched so its
      `h.saturating_sub(5)` becomes live again. Verify: test 1.2's row-count
      and meta-line assertions pass; `Video Two` appears at most as often as
      legacy geometry allows and the frame contains the overview fragment.
- [ ] 3.2 Port the legacy scroll clamp (`design.md` D3): replace
      `if selected_h > list_area.height { offset = selected; }` with the
      accumulated-height loop from
      `fbc6888e:src/app/render/components/home_feed.rs:131-140`, using
      `1`/`feed_selected_height` inline instead of an `item_heights` vec.
      Return the landed offset. Verify: new test
      `feed_group_tall_selected_row_scrolls_fully_into_view` — long list
      (≥ 8 items), metadata-bearing selected row near the bottom, assert the
      selected row's last expanded line is above `list_area.bottom()` and the
      returned offset is greater than `ctx.scroll`.
- [ ] 3.3 Confirm the landed offset reaches `FeedHomeVideoState::video_scroll`
      through the component's own scroll (`BrowserComponent` records it;
      `types_library_tab.rs` projects it). Verify by asserting in test 3.2's
      scenario that a second `draw` with unchanged input yields an identical
      frame (clamp sticky, not recomputed).
- [ ] 3.4 Delete the unconditional trailer block (`▔`-row + last-item
      `Paragraph`) at the end of `render_feed_group_picker_content` (D4).
      Verify: the metadata-free fixture shows no trailing border echo and no
      duplicated last title; the framed expanded row still has its own bottom
      border row.

## 4. Fix the Wide presentation (D2)

- [ ] 4.1 Add a failing wide test
      `feed_group_wide_paints_each_row_once_with_banner` at 140x40 on the
      metadata-bearing fixture: each video title appears once as a row and the
      rail's expanded block contains the banner meta line. Verify it fails
      today (doubled rows, no banner).
- [ ] 4.2 Change `render_wide_feed_layer` to take the rail list rect and
      `extras.feed_selected_height`, paint the selected row expanded / others
      as 1 row with no stride-2 skip, and call
      `render_compact_detail_with_ctx` with `show_title = false`, inset
      `SELECTED_BLOCK_SIDE_PADDING`, `y: row + 3`. Verify: test 4.1 passes;
      `layout.left_area` and `layout.selected_item_rect` describe the rail.
- [ ] 4.3 In `BrowserComponent::render_wide_movies`, skip the
      `render_generic_movies_home_video_rows_with_ctx` leg when
      `narrow_extras.feed_items.is_some()` (the feed layer owns the rail
      rows), leaving the left hero card, separator, and rail border
      untouched. Verify: full-frame 140x40 capture shows one set of rows,
      group pills in the rail, hero card on the left;
      `rtk cargo nextest run -p mbv emby_browser` green.

## 5. Re-pin baselines and gate

- [ ] 5.1 Regenerate `FEED_NARROW_BASELINE` and `FEED_WIDE_BASELINE` from the
      fixed implementation with the metadata-bearing fixture, in the same
      commit as the code change; keep the metadata-free frames as
      `FEED_*_DEGENERATE_BASELINE`. Verify the doc comment above each
      constant states the fixture, commit, and capture method (per the
      existing `6394f762` convention). Verify too that the only structural
      differences from a legacy narrow capture are the count/`▁` header rows
      (accepted, `b029fec3`), the expanded selected row, and its banner text —
      no border echo, no duplicated title.
- [ ] 5.2 Update the feed-picker rows of
      `docs/architecture/interactive-surface-ledger.md` to record the Wide
      rail's single painter and reference #634. Verify: no row claims both
      the feed layer and the generic rail path paint the same rect.
- [ ] 5.3 Run the full gate and record the output in this change:
      `rtk cargo fmt`, `rtk cargo clippy --workspace --all-targets`,
      `rtk cargo nextest run -p mbv`, `rtk make check-code-file-lines`.
      Verify: zero new failures against the recorded branch baseline; if
      `src/app/shell_tv_workspace_tests.rs` exceeds 800 lines it is the known
      pre-existing violation and must NOT be folded into this change.
- [ ] 5.4 Manual check against a real feed-view home-video library (or Emby
      podcast channel list) at 60x20, 100x30, and 140x40: expansion, banner
      content, group pill clicks, and bottom-edge scrolling behave as the
      spec scenarios state. Verify: note the observed behavior here; if it
      diverges, fix before merging rather than re-pinning the baselines.

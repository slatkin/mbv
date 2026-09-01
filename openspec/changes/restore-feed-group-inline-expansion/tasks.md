## 1. Pin the defect before fixing it

- [x] 1.1 In `src/app/tests_narrow_browse_migration.rs`, extend
      `feed_home_video_group_app()` with a metadata-bearing variant: give the
      selected video `runtime_ticks`, `genre`, and an `overview` long enough to
      wrap at 60x20. Verify the new fixture produces a
      `feed_snapshot`-style full-frame capture and that the existing
      `FEED_NARROW_BASELINE` assertion now FAILS on it (proves the old
      baselines were vacuous). Commit the failing expectation as a
      characterization of the bug, not as a passing test.
- [x] 1.2 Add `feed_home_video_group_expands_selected_row_narrow` asserting,
      on the metadata-bearing fixture at 60x20: the selected title appears
      exactly once as a plain row, the framed block's `▁`/`▔` rows exist, and
      the banner meta line (`Family` + duration fragment) is present. Verify
      it fails with the current `selected_h = 1`.

## 2. Publish the expansion height shell-side (D1)

- [x] 2.1 Add `feed_selected_height: u16` to `NarrowBrowseExtras`
      (`src/app/components/browser_narrow.rs`) with a doc comment naming the
      formula `banner.content_rows() + 5` and why it is computed shell-side
      (image-cache access; components issue no effects while painting).
      Verify: `rtk cargo check -p mbv --all-targets` clean apart from the
      field's unused-warning, resolved by 2.2.
- [x] 2.2 In `App::narrow_browse_extras`
      (`src/app/render/components/list_narrow.rs`), compute
      `feed_selected_height` for the `is_feed_home_video_group_view` branch
      from the already-built `CompactBannerLayout`, using the picker's panel
      width (`text_w - 2 * SELECTED_BLOCK_SIDE_PADDING`), and leave it `0`
      otherwise. Verify: a unit test asserting the published height equals
      `content_rows() + 5` for the fixture item (fails if it reads the
      generic path's `content_rows_with_title` value instead).

## 3. Fix the Normal painter

- [x] 3.1 Replace `let selected_h = 1;` in `render_feed_group_picker_content`
      with `extras.feed_selected_height.max(1)`, keeping the existing
      `render_compact_detail_with_ctx` call untouched so its
      `h.saturating_sub(5)` becomes live again. Verify: test 1.2's row-count
      and meta-line assertions pass; `Video Two` appears at most as often as
      legacy geometry allows and the frame contains the overview fragment.
- [x] 3.2 Port the legacy scroll clamp (`design.md` D3): replace
      `if selected_h > list_area.height { offset = selected; }` with the
      accumulated-height loop from
      `fbc6888e:src/app/render/components/home_feed.rs:131-140`, using
      `1`/`feed_selected_height` inline instead of an `item_heights` vec.
      Return the landed offset. Verify: new test
      `feed_group_tall_selected_row_scrolls_fully_into_view` — long list
      (≥ 8 items), metadata-bearing selected row near the bottom, assert the
      selected row's last expanded line is above `list_area.bottom()` and the
      returned offset is greater than `ctx.scroll`.
- [x] 3.3 Confirm the landed offset reaches `FeedHomeVideoState::video_scroll`
      through the component's own scroll (`BrowserComponent` records it;
      `types_library_tab.rs` projects it). Verify by asserting in test 3.2's
      scenario that a second `draw` with unchanged input yields an identical
      frame (clamp sticky, not recomputed).
- [x] 3.4 Delete the unconditional trailer block (`▔`-row + last-item
      `Paragraph`) at the end of `render_feed_group_picker_content` (D4).
      Verify: the metadata-free fixture shows no trailing border echo and no
      duplicated last title; the framed expanded row still has its own bottom
      border row.

## 4. Restore picker-row interaction (proposal "What Changes")

- [x] 4.1 Restore click hit-testing for the picker's video rows: the picker's
      row painter populates `layout.left_row_map` / `layout.left_item_rows` so
      `BrowserComponent::resolve_left_cursor` maps a click to the row under it
      (legacy `render_feed_home_video_group_view` built a `row_map` plus an
      offset + `click_y` fallback). Verify: a left click on a picker row moves
      the selection to that row — delivered by routing the picker through the
      shared narrow row path (commit `051bf75a`), which populates the shared
      hit maps `resolve_left_cursor` reads; the shared `left_item_rows`
      population is exercised by
      `feed_home_video_group_browser_scroll_updates_video_scroll`.
- [x] 4.2 Drop the one-column right shift of the row at index `selected + 1`
      (`x + 1`, `width - 1`) in the picker painter. Verify: no non-selected row
      is indented and the selected row's framed expansion owns its own border
      — the `idx == selected + 1` rect branch was removed in commit `27b87423`
      and the whole bespoke painter deleted in `051bf75a`.
- [x] 4.3 Remove the dead `feed_items` early return in
      `render_narrow_browse_with_ctx` that computed `inline_hero_rows` /
      `feed_ctx` / pills branches and then delegated to
      `render_feed_group_picker_content`, leaving the later
      `feed_items.is_some()` arms unreachable. Verify: the picker flows through
      the shared pill-bar + row + inline-hero path with no early return, and
      the `feed_items.is_some()` arms in `render_narrow_browse_with_ctx` are
      the live path — delivered in commit `051bf75a`.

## 5. Re-pin baselines and gate

- [x] 5.1 Regenerate `FEED_NARROW_BASELINE` from the fixed implementation
      with the metadata-bearing fixture, in the same commit as the code
      change; keep the metadata-free frame as
      `FEED_NARROW_DEGENERATE_BASELINE`. Verify the doc comment above each
      constant states the fixture, commit, and capture method (per the
      existing `6394f762` convention). Verify too that the only structural
      differences from a legacy narrow capture are the count/`▁` header rows
      (accepted, `b029fec3`), the expanded selected row, and its banner text —
      no border echo, no duplicated title.
- [x] 5.3 Run the full gate and record the output in this change:
      `rtk cargo fmt`, `rtk cargo clippy --workspace --all-targets`,
      `rtk cargo nextest run -p mbv`, `rtk make check-code-file-lines`.
      Verify: zero new failures against the recorded branch baseline; if
      `src/app/shell_tv_workspace_tests.rs` exceeds 800 lines it is the known
      pre-existing violation and must NOT be folded into this change.
- [x] 5.4 Manual check against a real feed-view home-video library (or Emby
      podcast channel list) at 60x20, 100x30, and 140x40: expansion, banner
      content, group pill clicks, and bottom-edge scrolling behave as the
      spec scenarios state. Verify: note the observed behavior here; if it
      diverges, fix before merging rather than re-pinning the baselines.

> Acceptance note (2026-09-01): user-authorized exception. Automated metadata/framing/scroll coverage and live YouTube homevideos size/Wide checks passed; live metadata-rich expansion and group-pill interaction were unavailable in the configured data/tmux path.

> Historical prerequisite note (do not copy): this change is a landed
> historical prerequisite for the canonical media-list campaign
> (`compose-canonical-media-lists`, umbrella task 1.2). It was delivered
> test-first — tasks 1.1/1.2 commit a failing characterization test, then
> tasks 2.x/3.x make it pass — and that ordering is preserved AS HISTORICAL
> FACT. It MUST NOT be cited as precedent for new canonical UI work, which
> follows visual-first ordering: implement, obtain explicit user live visual
> approval, then add or change UI buffer/geometry tests. Do not rewrite the
> completed task history above to claim visual-first was followed; the
> user-authorized-exception acceptance note stands as written.

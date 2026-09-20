# Tasks

Before starting any task, read `.agents/skills/mbv-frontend/SKILL.md` (required
before any TUI/render change per AGENTS.md).

## 1. Reserve the full-width band and split below it

- [ ] 1.1 In `wide_library_panes` (`render/arrangements/library.rs`), carve the top `WIDE_HERO_PILLS_ROW_HEIGHT + WIDE_HERO_PILLS_GAP_ROWS` rows off `area` as a full-width band, split the reduced `content_area` via `wide_hero_presentation`, and expose the band rect(s) and the `content_area` on `WideLibraryPanes`. Verify with a rect test: hero/browser `y` == band bottom, band width == `area.width`.
- [ ] 1.2 Thread the reduced `content_area` from `render_wide_skeleton` into `wide_hero_hero_pane` (replacing the raw `area`) so the hero pane paints below the band. Verify with a buffer test: the hero pane's top row is below the pill/spacer rows.

## 2. Paint the full-width Selector band; drop the Wide internal pill reserve

- [ ] 2.1 In the Wide path (`render_wide_skeleton`/`paint_browser_pane`, `library_panel/wide.rs`), feed the full-width band's `pills_area`/`spacer_area` (aligned to `PANE_PAD_X`) to the pill painter and give the Browser pane its full rect as `list_panel` with no internal pill reserve. Verify with a buffer test: pills span from the panel's left inset across both panes; the list starts at the Browser pane's top.
- [ ] 2.2 Confirm Narrow is untouched: `render_narrow_skeleton` still calls `wide_hero_browser_pane(area, area)` and reserves its own full-width band. Verify existing narrow buffer tests still pass.

## 3. Make the split drag independent of the band

- [ ] 3.1 In `panel_view.rs`, change the split-gap rect to the content band's vertical extent (`y: geometry.hero.y`, `height: geometry.hero.height`) instead of `area.y`/`area.height`. Verify with a `tests_tick_integration` case: a click/drag in the gutter columns within the pill/spacer rows does not start a split drag; a drag in the content band does.

## 4. Excise the List controls row

- [ ] 4.1 Remove `ListControls` and the `controls` field on `LibraryPanelContent` (`library_panel/content.rs`) and `paint_list_controls_row` (`library_panel/slots.rs`). Verify `cargo check -p mbv` after downstream edits.
- [ ] 4.2 Remove `controls` from `SkeletonHits`, `BrowserPaneGeometry`, `WideSkeletonGeometry`, and the `list_panel`/`controls_area` split in `paint_browser_pane` (`library_panel/wide.rs`); drop `controls` from `narrow.rs`'s geometry build. Verify `cargo check -p mbv`.
- [ ] 4.3 Unwind `home_video`: remove the field/push/use in `emby_library_content.rs` and its computation/push in `shell_emby_library_content.rs`. Do NOT touch `feed_home_video`/`is_feed_home_video_group_view`, and KEEP `is_home_video_view` (still used by `music_actions.rs`, `tests_non_music.rs`). Verify `cargo check -p mbv` and `rg "home_video" src/` shows only the retained feed-grouping and `is_home_video_view` symbols.

## 5. Tests and gates

- [ ] 5.1 Delete the stale controls-row tests (`slots.rs::list_controls_row_paints_its_label` and any controls assertions in `wide_tests.rs`/`panel_tests.rs`/`tests_library_characterization.rs`); write fresh buffer/tick tests for the new behavior per the spec's scenarios (full-width band, hero pushed down, drag independence, no home-video count). Verify `cargo nextest run -p mbv` green.
- [ ] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`; verify no dead-code or unused-import warnings from the removals.
- [ ] 5.3 Manually verify against the legacy Wide reference (Home, Music, TV, Feeds, Books, Podcast): pills full-width, hero two rows down, drag gap below the band, home-video count gone, Inline Search bar full-width.

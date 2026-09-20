# Tasks

Before starting any task, read `.agents/skills/mbv-frontend/SKILL.md` (required
before any TUI/render change per AGENTS.md). Decision IDs (D1–D7) reference
`design.md`.

## 1. Reserve the full-width band and split below it (D1, D2, D7)

- [ ] 1.1 Expose the pill-band height from `wide_hero.rs` (D7): add a `pub(in crate::app)` accessor (const or `pill_band_height()` fn) equal to `WIDE_HERO_PILLS_ROW_HEIGHT + WIDE_HERO_PILLS_GAP_ROWS`; do not duplicate the literal. Verify `cargo check -p mbv`.
- [ ] 1.2 In `wide_library_panes` (`render/arrangements/library.rs`) implement fit-first-then-carve (D1): check the breakpoint on the **uncarved** `area`, then carve the band off the top (full width), then split the reduced `content_area`; expose the band rect(s) and `content_area` on `WideLibraryPanes`. Verify with a rect test at raw heights 7, 8, 9: Wide is chosen at exactly the same heights as before this change (no 2-row shift), and when chosen, hero/browser `y` == band bottom and band width == `area.width`.
- [ ] 1.3 Thread the reduced `content_area` from `render_wide_skeleton` into `wide_hero_hero_pane` (D2), replacing the raw `area`. Verify with a buffer test: the hero pane's top row is below the pill/spacer rows.

## 2. Paint the full-width Selector band; drop the Wide internal pill reserve (D3, D5)

- [ ] 2.1 In the Wide path (`render_wide_skeleton`/`paint_browser_pane`, `library_panel/wide.rs`), feed the full-width band's `pills_area`/`spacer_area` (aligned to `PANE_PAD_X`, D5) to the pill painter and pass the **`browser_panel`** rect (the full un-inset pane, D3) as `list_panel`, with no internal pill reserve. Verify with a buffer test: pills span from the panel's left inset across both panes; the list-box fill starts at the Browser pane's top and reaches its border.
- [ ] 2.2 Confirm Narrow is untouched: `render_narrow_skeleton` still calls `wide_hero_browser_pane(area, area)` and reserves its own full-width band. Verify existing narrow buffer tests still pass.
- [ ] 2.3 Migrate the `WideLibraryPanes` consumers to the new shape: `wide_tests.rs:110-111`, `tests_wide_hero_split_override.rs:88-89,112-113`, `tests_wide_hero_pane_characterization.rs:69`. Verify `cargo nextest run -p mbv` compiles these targets.

## 3. Make the split drag independent of the band (D4)

- [ ] 3.1 In `panel_view.rs`, change the split-gap rect to the content band's vertical extent (`y: geometry.hero.y`, `height: geometry.hero.height`) instead of `area.y`/`area.height` (D4). Verify with a `tests_tick_integration` case: a press in the gutter columns within the pill/spacer rows does not start a split drag; a drag in the content band does.

## 4. Excise the List controls row and its event (D6)

Note: the per-subtask edits below do not each compile in isolation; the compile
gate is task 4.5 (a whole change under lib-only `cargo check` cannot verify test
targets, so 4.5 uses `--all-targets`).

- [ ] 4.1 Remove `ListControls` and the `controls` field on `LibraryPanelContent` (`content.rs`) and `paint_list_controls_row` (`slots.rs`).
- [ ] 4.2 Remove `controls` from `SkeletonHits`, `BrowserPaneGeometry`, `WideSkeletonGeometry`, and the `list_panel`/`controls_area` split in `paint_browser_pane` (`wide.rs`); drop `controls` from `narrow.rs`'s geometry build.
- [ ] 4.3 Remove the `LibrarySlotEvent::ControlPicked` variant (`owner.rs:107`), its resolution arm (`panel.rs:655-657`), and its dispatch arms in `home_content.rs:581`, `emby_library_content.rs:685`, `podcast_content.rs:600`, `feeds_content.rs:523`, `tv_content/interaction.rs:65`, `book_content.rs:483`, `music_interaction.rs:150` (all are `=> None` no-ops). No wildcard arm may hide the removal.
- [ ] 4.4 Unwind `home_video`: remove the field/push/use in `emby_library_content.rs` and its computation/push in `shell_emby_library_content.rs`. Do NOT touch `feed_home_video`/`is_feed_home_video_group_view`, and KEEP `is_home_video_view` (still used by `music_actions.rs:164`, `tests_non_music.rs:13`).
- [ ] 4.5 Verify the excision compiles and is clean: `cargo check --all-targets -p mbv` passes and `rg "ControlPicked|ListControls|paint_list_controls_row|\bhome_video\b" src/` shows only the retained feed-grouping symbols (`feed_home_video`, `is_home_video_view`).

## 5. Tests, vocabulary, and gates

- [ ] 5.1 Update the controls-row tests per the frontend skill's rule (each removed assertion names its surviving owner test): delete `slots.rs::list_controls_row_paints_its_label` and the controls assertions in `wide_tests.rs:174,196`, `panel_tests.rs`, `narrow_tests.rs:66`, `tests_library_characterization.rs`, `book_content.rs:733`, `feeds_content.rs:682,702`, `podcast_content.rs:849`, `tests_feeds.rs:111,186`, `tests_podcast_panel.rs:149`. Write fresh buffer/tick tests for the delta's scenarios whose owners disappear: full-width band spanning both panes, hero pushed below the band, split-drag independence (task 3.1), no home-video count, full-width Inline Search box, Selector pill hit geometry after the move, and an undersized-area case (skill §278) exercising the band's saturating arithmetic. Verify `cargo nextest run -p mbv` green.
- [ ] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`; verify no dead-code or unused-import warnings from the removals.
- [ ] 5.3 Update `CONTEXT.md` (D6): remove the "List controls row" definition and strike it from the "Library panel" definition's slot list.
- [ ] 5.4 Manually verify against the pre-change build (git stash or commit `2dce938c^`) for Home, Music, TV, Feeds, Books, Podcast: pills full-width, hero two rows down, drag gap below the band, home-video count gone, Inline Search bar full-width.

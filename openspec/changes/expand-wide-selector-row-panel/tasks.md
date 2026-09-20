# Tasks

Before starting any task, read `.agents/skills/mbv-frontend/SKILL.md` (required
before any TUI/render change per AGENTS.md). Decision IDs (D1–D7) reference
`design.md`.

## 1. Reserve the full-width band and split below it (D1, D2, D7)

- [x] 1.1 Expose the pill-band height from `wide_hero.rs` (D7): add a `pub(in crate::app)` accessor (const or `pill_band_height()` fn) equal to `WIDE_HERO_PILLS_ROW_HEIGHT + WIDE_HERO_PILLS_GAP_ROWS`; do not duplicate the literal. Verify `cargo check -p mbv`.
- [x] 1.2 In `wide_library_panes` (`render/arrangements/library.rs`) implement fit-first-then-carve (D1): check the breakpoint on the **uncarved** `area`, then carve the band off the top (full width), then split the reduced `content_area`; expose the band rect(s) and `content_area` on `WideLibraryPanes`. Verify with a rect test at raw heights 7, 8, 9: Wide is chosen at exactly the same heights as before this change (no 2-row shift), and when chosen, hero/browser `y` == band bottom and band width == `area.width`.
- [x] 1.3 Thread the reduced `content_area` from `render_wide_skeleton` into `wide_hero_hero_pane` (D2), replacing the raw `area`. Verify with a buffer test: the hero pane's top row is below the pill/spacer rows.

## 2. Paint the full-width Selector band; drop the Wide internal pill reserve (D3, D5)

- [x] 2.1 In the Wide path (`render_wide_skeleton`/`paint_browser_pane`, `library_panel/wide.rs`), feed the full-width band's `pills_area`/`spacer_area` (aligned to `PANE_PAD_X`, D5) to the pill painter and pass the **`browser_panel`** rect (the full un-inset pane, D3) as `list_panel`, with no internal pill reserve. Verify with a buffer test: pills span from the panel's left inset across both panes; the list-box fill starts at the Browser pane's top and reaches its border.
- [x] 2.2 Confirm Narrow is untouched: `render_narrow_skeleton` still calls `wide_hero_browser_pane(area, area)` and reserves its own full-width band. Verify existing narrow buffer tests still pass.
- [x] 2.3 Migrate the `WideLibraryPanes` consumers to the new shape: `library_panel/wide_tests.rs:110-111`, `src/app/render/tests_wide_hero_split_override.rs:88-89,112-113`, `src/app/render/tests_wide_hero_pane_characterization.rs:69`. Verify `cargo nextest run -p mbv` compiles these targets.

## 3. Make the split drag independent of the band (D4)

- [x] 3.1 In `panel_view.rs`, change the split-gap rect to the content band's vertical extent (`y: geometry.hero.y`, `height: geometry.hero.height`) instead of `area.y`/`area.height` (D4). Verify with a `tests_tick_integration` case: a press in the gutter columns within the pill/spacer rows does not start a split drag; a drag in the content band does.

## 4. Excise the List controls row and its event (D6)

Note: the per-subtask edits below do not each compile in isolation. Task 4.5 is
a **lib-only** `cargo check -p mbv` gate (test modules still reference the removed
items until 5.1, and `--all-targets` compiles them — so the `--all-targets` gate
lives at the end of 5.1, not here).

- [x] 4.1 Remove `ListControls` and the `controls` field on `LibraryPanelContent` (`content.rs`), `paint_list_controls_row` (`slots.rs`), and their re-exports in `library_panel/mod.rs` (the `ListControls` import at `mod.rs:29`, the `paint_list_controls_row` re-export at `mod.rs:47`).
- [x] 4.2 Remove `controls` from `SkeletonHits`, `BrowserPaneGeometry`, `WideSkeletonGeometry`, and the `list_panel`/`controls_area` split in `paint_browser_pane` (`wide.rs`); drop `controls` from `narrow.rs`'s geometry build.
- [x] 4.3 Remove the `LibrarySlotEvent::ControlPicked` variant (`owner.rs:107`), its resolution arm (`panel.rs:655-657`), and its dispatch arms in `home_content.rs:581`, `emby_library_content.rs:685`, `podcast_content.rs:600`, `feeds_content.rs:523`, `tv_content/interaction.rs:65`, `book_content.rs:483`, `music_interaction.rs:150` (all are `=> None` no-ops). No wildcard arm may hide the removal.
- [x] 4.4 Unwind `home_video`: remove the field/push/use in `emby_library_content.rs` and its computation/push in `shell_emby_library_content.rs`. Do NOT touch `feed_home_video`/`is_feed_home_video_group_view`, and KEEP `is_home_video_view` (still used by `music_actions.rs:164`, `tests_non_music.rs:13`).
- [x] 4.5 Verify the production excision compiles (lib only): `cargo check -p mbv` passes. Then `rg -n "ControlPicked|ListControls|paint_list_controls_row" src/` shows no hits, and `rg -n "home_video" src/` shows only the retained `feed_home_video`/`is_feed_home_video_group_view` and `is_home_video_view` symbols (the standalone `home_video` binding is gone). Do NOT run `--all-targets` here — test modules still reference the removed items until 5.1.

## 5. Tests, vocabulary, and gates

- [x] 5.1 Remove **all** controls-related test code — the `controls:` struct-literal sites (`controls: None`/`Some(ListControls { .. })`, compiler-enumerated: ~22 in `library_panel/wide_tests.rs`, `narrow_tests.rs:37,124`, `panel_tests.rs`), the `ListControls` test imports (`narrow_tests.rs:5`, `wide_tests.rs:7`), and the assertions — including but not limited to `slots.rs::list_controls_row_paints_its_label`, `library_panel/wide_tests.rs:174,196`, `panel_tests.rs`, `narrow_tests.rs:66`, `src/app/render/tests_library_characterization.rs`, `components/book_content.rs:733`, `components/feeds_content.rs:682,702`, `components/podcast_content.rs:849`, `src/app/render/tests_feeds.rs:111,186`, `src/app/render/tests_podcast_panel.rs:149`. Per the frontend skill's rule, each removed assertion names its surviving owner test. Write fresh buffer/tick tests for the delta's scenarios whose owners disappear: full-width band spanning both panes, hero pushed below the band, split-drag independence (task 3.1), no home-video count, full-width Inline Search box, Selector pill hit geometry after the move, and an undersized-area case (skill §278) exercising the band's saturating arithmetic. Verify `cargo nextest run -p mbv` green, then `cargo check --all-targets -p mbv` passes (the full compile gate, now that test modules no longer reference the removed items).
- [x] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`; verify no dead-code or unused-import warnings from the removals.
- [x] 5.3 Update `CONTEXT.md` (D6): remove the "List controls row" definition and strike it from the "Library panel" definition's slot list.
- [ ] 5.4 Manually verify against the pre-change build (git stash or commit `2dce938c^`) for Home, Music, TV, Feeds, Books, Podcast: pills full-width, hero two rows down, drag gap below the band, home-video count gone, Inline Search bar full-width.

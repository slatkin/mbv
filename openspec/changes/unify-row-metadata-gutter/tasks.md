## 1. Row painter and palette

- [ ] 1.1 In `src/app/render/components/media_list/row.rs`, add `const YEAR_GUTTER_W: usize = 4` beside the existing `DATE_GUTTER_W: usize = 6`, and update `DATE_GUTTER_W`'s doc comment so it describes the date gutter specifically rather than "the" gutter. Verify: `cargo check -p mbv` compiles clean.
- [ ] 1.2 In the same file, replace the `MediaListTrailing::Year` arm's `trailing_pieces.push(...)` with a capture that feeds the right-aligned gutter path alongside `Published`, carrying the arm's gutter width (4 for `Year`, 6 for `Published`). The reserve currently computed as `date_reserve` becomes `QUIET_GAP + <that width>`, and the paint block's `trunc_str`/`{:>width$}` use the same width. Verify: `cargo check -p mbv` compiles clean and `trailing_pieces` now receives only the progress percentage.
- [ ] 1.3 In the same paint block, change the gutter's style from `palette::ROW_DATE_FG` to `palette::STATUS_AVAILABLE`. Verify: `rg -n "ROW_DATE_FG" src/app/render/components/` returns nothing.
- [ ] 1.4 Re-read `row.rs`'s module doc comment (lines ~12-36) and the comments above `trailing_pieces`/`DATE_GUTTER_W` that describe a "left-aligned green year" as distinct from a "right-gutter date" — update them to the unified model with two widths. Verify: no comment in the file still claims a year renders inline after the title.
- [ ] 1.5 In `src/app/render/theme/mod.rs`, delete `ROW_DATE_FG`, and remove it from the re-export lists in `src/app/render/mod.rs` and `src/app/palette.rs`. Verify: `cargo check -p mbv` names every remaining reference (task 2.1 clears them); afterwards `rg -n "ROW_DATE_FG" src` returns nothing.
- [ ] 1.6 In `src/app/components/media_list/mod.rs`, update the `MediaListTrailing::Year` and `::Published` doc comments (the `Published` one at ~line 433 names `ROW_DATE_FG`) to describe both as right-aligned green gutters differing only in width. Verify: neither comment names `ROW_DATE_FG` or an inline placement.

## 2. Music tree browser

- [ ] 2.1 In `src/app/components/music_tree.rs`, change `YEAR_GUTTER_WIDTH` from `6` to `4`, and in `src/app/components/music_tree_label.rs` change the year span's style from `palette::ROW_DATE_FG` to `palette::STATUS_AVAILABLE`. Verify: `cargo check -p mbv` compiles clean; the reserve math in `music_tree_view.rs:254` and `music_tree_label.rs:38` reads the constant and needs no edit.
- [ ] 2.2 Update the comment block at `music_tree_label.rs:97-102` that describes a "fixed six-column gutter … in the `ROW_DATE_FG` role". Verify: the comment names four columns and the green role.

## 3. Canonical-list call sites

- [ ] 3.1 Confirm the `MediaListTrailing::Year` construction sites are exactly `src/app/components/emby_library_content.rs`, `src/app/components/tv_content/mod.rs`, `src/app/components/inline_search.rs`, and the `::Published` site is `src/app/components/podcast_content.rs`. No variant rename is needed, so no construction changes — but check each site's surrounding comment for a claim about inline year placement and fix any that is now wrong. Verify: `rg -n "MediaListTrailing::(Year|Published)" src` lists only those sites plus test fixtures.

## 4. Hero Workspace durations

- [ ] 4.1 In `src/app/components/tv_content/mod.rs`, `build_episode_rows` supplies `duration: Some(fmt_duration_short(seconds))` from the episode's `runtime_ticks` (ticks are 100ns units), `None` when the runtime is absent or zero. Replace the "Library lists carry no time column" comment on that builder with the Workspace carve-out. Verify: `cargo check -p mbv` compiles clean.
- [ ] 4.2 Same change in `src/app/components/music_content_workspace.rs` (`track_row`, which serves both the album and artist Workspaces). Verify: `cargo check -p mbv` compiles clean.
- [ ] 4.3 Same change in `src/app/components/book_content.rs` (`chapter_rows`), deriving seconds from `BookRow::Chapter { start, end }` and `BookRow::AudioFile { duration }`. Verify: `cargo check -p mbv` compiles clean.
- [ ] 4.4 Confirm no ordinary browse list gained a duration as a side effect. Verify: `rg -n "duration: Some" src/app/components/` lists only the three Workspace builders, the Queue, and the sessions modal.

## 5. Tests

- [ ] 5.1 In `src/app/render/components/media_list.rs`, update the buffer test asserting `STATUS_AVAILABLE` at the old inline year column (around line 398) to assert the year right-aligned in a four-column gutter, still `STATUS_AVAILABLE`. Verify: `cargo nextest run -p mbv <test name>` passes.
- [ ] 5.2 In the same file, update the tests asserting `ROW_DATE_FG` at the gutter column (around line 1084) to assert `STATUS_AVAILABLE`, and the surrounding date fixtures at lines ~1053, ~1102, ~1192. Verify: `rg -n "ROW_DATE_FG" src` returns nothing.
- [ ] 5.3 Add one buffer test proving the two widths: a row carrying a four-character year and a row carrying a six-character date both paint their gutter flush to the same right edge, in the same green role, with the year's title slot two columns wider. Verify: the new test passes.
- [ ] 5.4 Update `src/app/render/tests_music_characterization.rs` (the `ROW_DATE_FG` assertions and prose at ~819, ~876, ~910, and the gutter-column arithmetic at ~367) and `src/app/render/tests_music_groups.rs` (~287) for the four-column green gutter. Verify: `cargo nextest run -p mbv` passes for both files.
- [ ] 5.5 Add or extend one test per Workspace (TV episodes, music tracks, book chapters) proving the row now carries a `M:SS`/`H:MM:SS` duration, and one proving the corresponding browse list still carries none. Verify: the new tests pass.

## 6. Whole-crate verification

- [ ] 6.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv`. Verify: all three succeed with no warnings/failures.
- [ ] 6.2 Manually run the app (`run` skill) and check: a Movies/TV/Generic list (year right-aligned, green, four columns), the Podcast episode list (date right-aligned, green, six columns), the music tree (album year right-aligned, green, four columns), and a TV season / music album / book Hero Workspace (right-aligned duration on every row). Verify: group headings show no gutter and no duration, and no browse list shows a duration.

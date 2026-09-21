## 1. Type and palette

- [x] 1.1 In `src/app/components/media_list/mod.rs`, replace `MediaListTrailing::Year(String)` and `MediaListTrailing::Published(String)` with a single `MediaListTrailing::Gutter(String)` variant; update its doc comment to describe the merged right-gutter/green behaviour. Verify: `cargo check -p mbv` fails at every call site that still names `Year`/`Published` (confirms the type change is exhaustive-checked, not silently ignored).
- [x] 1.2 In `src/app/render/theme/mod.rs`, delete `ROW_DATE_FG`. Verify: `cargo check -p mbv` fails at every remaining reference (there should be none left after task 2.1, so this task's own build error is the checklist for task 2.1's completeness).

## 2. Row painter

- [x] 2.1 In `src/app/render/components/media_list/row.rs`, replace the `Year`/`Published` match arms with one arm matching `MediaListTrailing::Gutter(text)`, feeding the existing `date_reserve`/`DATE_GUTTER_W` right-aligned paint path (the current `published` block), recoloured to `palette::STATUS_AVAILABLE`. Remove the now-dead inline `trailing_pieces` push for years (the FOAM progress-percentage push stays). Verify: `cargo check -p mbv` compiles clean.
- [x] 2.2 Re-read the row's module doc comment (`row.rs` lines ~12-36) and the comments above `trailing_pieces`/`DATE_GUTTER_W`/`MediaListTrailing` describing "left-aligned green year" vs "right-gutter date" as distinct — update or remove any that now describe the merged model. Verify: no comment in the file still claims a year renders inline.

## 3. Call sites

- [x] 3.1 Update `MediaListTrailing::Year(...)` construction to `MediaListTrailing::Gutter(...)` in `src/app/components/emby_library_content.rs`, `src/app/components/tv_content/mod.rs`, `src/app/render/components/music_wide.rs`, `src/app/components/inline_search.rs`. Verify: `cargo check -p mbv` compiles clean and each site's surrounding comment (if any) referencing "year" placement is still accurate.
- [x] 3.2 Update `MediaListTrailing::Published(...)` construction to `MediaListTrailing::Gutter(...)` in `src/app/components/podcast_content.rs` (both the live construction and the test fixture literal). Verify: `cargo check -p mbv` compiles clean.

## 4. Tests

- [x] 4.1 In `src/app/render/components/media_list.rs`, update the buffer test asserting `STATUS_AVAILABLE` at the old inline year column (around line 398) to assert the year now paints right-aligned in the gutter column, still `STATUS_AVAILABLE`. Verify: `cargo nextest run -p mbv <test name>` passes.
- [x] 4.2 In the same file, update the buffer test asserting `ROW_DATE_FG` at the gutter column (around line 1080) to assert `STATUS_AVAILABLE` instead. Verify: `cargo nextest run -p mbv <test name>` passes.
- [x] 4.3 Grep the whole crate for any other test literal referencing `MediaListTrailing::Year`, `MediaListTrailing::Published`, or `ROW_DATE_FG` (including `media_list.rs` lines ~369, ~1049, ~1098, ~1192) and update each to `Gutter`/`STATUS_AVAILABLE`. Verify: `rg -n "MediaListTrailing::Year|MediaListTrailing::Published|ROW_DATE_FG" src` returns no results.
- [x] 4.4 Add or extend one buffer test proving a row carrying a release year and a row carrying a publish date paint identically (same column, same colour) given the same string length, so the merge itself is covered, not just each old assertion ported forward. Verify: `cargo nextest run -p mbv` new test passes.

## 5. Whole-crate verification

- [ ] 5.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv` (and any other package touched). Verify: all three succeed with no warnings/failures.
- [ ] 5.2 Manually run the app (`run` skill) and check a Movies/TV/Music/Generic list (year in gutter, green) and the Podcast episode list (date in gutter, green) side by side. Verify: both show a right-aligned green date/year column at the same row position, and group headings show no gutter.

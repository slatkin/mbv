## 1. Extract the shared marquee primitive

- [x] 1.1 Move `marquee_col` and the width-windowing function out of `src/app/render/components/chrome_player.rs` into a new small module (e.g. `src/app/render/components/marquee.rs`), generalized to take `&mut String` + `&mut Instant` directly (no `PlaybackRenderContext` dependency). Keep `marquee_col`'s existing unit test (`marquee_advances_five_columns_per_second`) passing unchanged after the move. Verify: `cargo check -p mbv` and `cargo nextest run -p mbv chrome_player` (or the moved test's new path) both pass.
- [x] 1.2 Update `chrome_player.rs::marquee_spans` to call the extracted primitive with its own `ctx.marquee_text`/`ctx.marquee_started_at`, with no behavior change. Verify: the existing `standard_title_row_showcases_instead_of_truncating_a_long_title` and `idle_feed_title_marquees_instead_of_truncating` tests in `src/app/render/tests.rs` still pass unmodified.

## 2. Give `MediaList<Target>` its own marquee clock

- [x] 2.1 Add private `marquee_text: String` and `marquee_started_at: Instant` fields to `MediaList<Target>` in `src/app/components/media_list/mod.rs`, initialized in `MediaList::new()` alongside `cursor`/`scroll`. Add a method that, given the currently-marqueed row's text, returns `(&mut String, &mut Instant)`-style access for the row painter, resetting `marquee_started_at` to `Instant::now()` whenever the stored text doesn't match. Verify: `cargo check -p mbv`.
- [x] 2.2 Expose that access through `WideMediaList<Target>` and `InlineMediaBrowser<Target>` (whichever accessor pattern matches their existing forwarding methods, e.g. next to `set_scroll`/`row_geometry`). Verify: `cargo check -p mbv`.

## 3. Marquee the selected+focused row in the shared row painter

- [x] 3.1 In the shared row painter (`wide_row.rs::wide_media_row`, or its post-rename equivalent `media_list_row` if that rename has already landed — check `src/app/render/components/media_list/` for the current name before starting), replace the unconditional `trunc_str(primary, ...)` call for the title with: use the extracted marquee primitive when this row is the one being marqueed and its title overflows the available width; otherwise keep `trunc_str` exactly as today. The painter needs a way to know "is this the row to marquee" and mutable access to the clock — thread an `Option<&mut (String, Instant)>`-shaped parameter (or equivalent) in, passed as `Some` only for the row satisfying `selected && focused` from the caller. Verify: `cargo check -p mbv`.
- [x] 3.2 Update `render_wide_media_list` in `src/app/render/components/media_list/wide.rs` to pass the marquee clock only for the row where `Some(row) == selected_row` (it already computes this), sourced from `list`'s (already-`&mut`) marquee accessor from task 2.2. Verify: `cargo check -p mbv`.
- [x] 3.3 Change `render_inline_media_browser_with_geometry`'s `list` parameter from `&InlineMediaBrowser<Target>` to `&mut InlineMediaBrowser<Target>` and pass the marquee clock only for the row where `Some(display_row) == selected_row && layout.detail_rows == 0` (it already computes this). Its caller `render_inline_media_browser_component` already holds `&mut`, so no further ripple. Verify: `cargo check -p mbv`.

## 4. Tests

- [x] 4.1 Add a narrow buffer test (following the existing pattern in `src/app/render/components/media_list.rs`'s `wide_row_regression_tests`) asserting: a selected+focused row with an overflowing title contains no ellipsis and shows the title's start at rest, then shows a different window after advancing the row's own marquee clock past the initial hold — mirroring `idle_feed_title_marquees_instead_of_truncating`'s two-render-and-compare shape. Verify: `cargo nextest run -p mbv wide_row_regression_tests` passes.
- [x] 4.2 Add a narrow test asserting an unfocused list's selected row, and a non-selected row in a focused list, both still ellipsis-truncate an overflowing title unchanged. Verify: same test run as 4.1 passes.
- [x] 4.3 Add a narrow test asserting the marquee clock restarts (renders the title's start again) after the selected row's target/content changes to a new overflowing title. Verify: same test run as 4.1 passes.

## 5. Whole-change verification

- [x] 5.1 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`, fix any findings introduced by this change. Verify: both commands exit clean.
- [x] 5.2 Run `cargo nextest run -p mbv` for the full crate and confirm no regressions outside the tests touched above. Verify: full run is green.
- [x] 5.4 Shorten the shared marquee cadence in `src/app/render/components/marquee.rs` (user-approved 2026-09-14): `STEP_MS` 200→150, `HOLD_MS` 1200→600, applying to every marquee call site (media-list rows and the chrome player strip, which share the primitive). Update `marquee_advances_five_columns_per_second` to the new constants. Verify: `cargo nextest run -p mbv marquee` passes and `cargo check --workspace --all-targets` is clean.
- [x] 5.3 Manually exercise a list with a title long enough to overflow (e.g. a long album/series title) in the running app: confirm the selected+focused row marquees, moving selection away truncates it normally, and an unfocused list's selection does not marquee. Verify: observed directly in the terminal.

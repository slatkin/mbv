# Tasks

Each unit may leave dependent callers or the build broken for the next unit. A unit records that state in its handoff. All work is in `crates/mbv-gui`. Every verification names `-p mbv-gui`. Tests follow `crates/mbv-gui/AGENTS.md` and `docs/invariants/14-test-ownership.md`: headless painted-structure assertions only, one contract per test, extend an existing table before you add a test. No lint suppression, no `allow` or `expect` attribute, no sleeps. Run `cargo fmt` after each Rust edit. Run cargo commands in the foreground and do not pipe them.

## 1. The shared play look (design D1)

- [x] 1.1 Add `crates/mbv-gui/src/window/components/play_mark.rs` (registered in `crates/mbv-gui/src/window/components.rs`) with `RowPlay`, `RowPlay::of_row`, `RowPlay::of_queue`, the five look methods, the named constants, and the `PlayMark { play: Option<RowPlay> }` component, exactly as design D1 states. The unplayed dot is a rounded span. The played check is `crate::window::icons::Check` at size 12 with `aria_hidden: true`. The slot carries `data-play-mark`, and `role="img"` plus `aria-label` on the two marks. `RowPlay::of_row` calls `library::episode::is_played_episode`. Verify: `cargo check -p mbv-gui` passes.
- [x] 1.2 Add `crates/mbv-gui/src/window/components/play_mark/tests.rs`. Write one `#[case]` table for `RowPlay::of_row` over `Ordinary`, `Played`, and `Active { progress: Some(_) }`, and one for `RowPlay::of_queue` over `Ordinary`, `Played`, `Active`, and `NowPlaying` (to `None`). Write one `#[case]` table that renders `PlayMark` through `DioxusDocument` for `Some(Unplayed)`, `Some(Played)`, and `None`, and asserts the `data-play-mark` value, the `role`, and the `aria-label` (spec "Rows mark their played state"). Verify: `cargo nextest run -p mbv-gui play_mark` passes.

## 2. Flat row and thumb (design D2, D3, D4)

- [x] 2.1 In `crates/mbv-gui/src/window/components/list_row/thumb.rs`, give `paint_thumb` an `opacity: Option<f32>` argument. When it is `Some`, add `opacity: <value>;` to the box style. Write no `filter` (design D2). Update its callers to pass `None` for now. Verify: `cargo check -p mbv-gui` passes.
- [x] 2.2 In `crates/mbv-gui/src/window/components/list_row.rs`, change `RowLevel::title_tone` to return `RowPlay::of_row(play).title_tone()` for `Some`. Add `RowLevel::title_weight(play)`, which returns the level's present weight for `None` (Top `LIBRARY_ROW_TITLE_WEIGHT`, Parent 400, Leaf the leaf weight) and `title_weight()` for `Some`. Delete `played_title_tone` and update the module doc. The Workspace caller breaks until task 4.1. Record that in the handoff. Verify: `cargo check -p mbv-gui --lib` reports only the `library_workspace_table.rs` error.
- [x] 2.3 In `crates/mbv-gui/src/window/components/list_row/flat.rs`, apply design D4 "Flat row": the title tone and weight (CSS and the `Truncate` `TextStyle`), the subtitle's `secondary_tone()`, the trail's `trail_tone()`, the thumb's `thumb_opacity()`, and the D3 trailing cluster (`PlayMark` then the trail text, the cluster carrying `hover_trail_class`). A `play: None` row keeps its present markup. Update the module doc. Verify: `cargo check -p mbv-gui` reports no error in `flat.rs`.
- [x] 2.4 In `crates/mbv-gui/src/window/components/list_row/tests.rs`, update every expected tone string that the change moves from the `--canvas-*` tokens to `--ef-*` tokens. Add one `#[case]` table on the flat row over `play: None`, `Some(Ordinary)`, and `Some(Played)`. It asserts the `data-play-mark` value (absent for `None`), the thumb box `opacity: 0.35` only on the played row, and that the mark and the trail text share the one `row-hover-trail` cluster (spec "A played row", "The hover controls cover the trailing end"). Verify: `cargo nextest run -p mbv-gui list_row` passes.

## 3. Tree row (design D3, D4)

- [x] 3.1 In `crates/mbv-gui/src/window/components/list_row/tree.rs`, apply design D4 "Tree row" to the leaf and the parent body: the title tone and weight (a `font-weight` on the `Truncate` box, and the `TextStyle` at that weight), the trail's `trail_tone()`, and the D3 trailing cluster when `play` is `Some`. A `play: None` row keeps its present markup. Update the module doc. Verify: `cargo check -p mbv-gui` reports no error in `tree.rs`.
- [x] 3.2 Extend `a_nested_rows_title_paints_its_play_tone` in `crates/mbv-gui/src/window/components/list_row/tests.rs` so that each case also asserts the title weight and the `data-play-mark` value. `None` keeps the leaf tone and has no mark, `Ordinary` paints `var(--ef-fg)` at 600 with `unplayed`, and `Played` paints `var(--ef-grey1)` at 400 with `played`. Verify: `cargo nextest run -p mbv-gui list_row` passes.

## 4. Workspace table (design D4)

- [x] 4.1 In `crates/mbv-gui/src/window/components/library_workspace_table.rs`, apply design D4 "Workspace table". `title_tone` uses `RowPlay` (the playing row keeps `heading`), the title weight follows `title_weight()` for a carried state, and the trailing cell wraps the duration in the D3 cluster when `state` is `Some`. The playing row paints `PlayMark { play: None }`. A played row's duration paints as a plain text span in `Grey1`. `GRID_COLUMNS` does not change. Update the module doc. Verify: `cargo check -p mbv-gui` passes.
- [x] 4.2 Extend `a_rows_play_state_decides_its_title_tone_and_its_resume_run` in `crates/mbv-gui/src/window/components/library_workspace_table/tests.rs` with the new tones and a `data-play-mark` column (`None` state has no mark, `Active` with and without a percent has `unplayed`, `Played` has `played`). No playing-row test exists. Add one test: a playing row that carries `Played` keeps the `heading` title tone and has mark `none` (spec "The playing Workspace row keeps its own mark"). Verify: `cargo nextest run -p mbv-gui library_workspace_table` passes.

## 5. Queue rows (design D4)

- [x] 5.1 In `crates/mbv-gui/src/window/components/queue_panel/row_cells.rs`, apply design D4 "Queue row cells". Drop `strike` from `RowLook` and `line-through` from the title class. Keep the now-playing branch. Take the other rows' title tone and weight from `RowPlay::of_queue`, and keep the hover rule. Paint `PlayMark` between the ring and the duration in `RowChrome::Row` only. `RIGHT_COL_PX` does not change. Update the module doc and the `RowLook` doc. Verify: `cargo check -p mbv-gui` passes.
- [x] 5.2 Rewrite `the_rows_paint_the_watched_and_now_playing_states` in `crates/mbv-gui/src/window/components/queue_panel/tests/rows.rs`. The watched row's title has no `line-through` and paints `Grey1` at 400 with mark `played`. Add an `Ordinary` row that paints `Fg` at 600 with mark `unplayed`. The now-playing row keeps its green fill and title and has mark `none` (spec "Rows show their state"). Verify: `cargo nextest run -p mbv-gui queue_panel` passes.

## 6. Docs and integration

- [ ] 6.1 Update `docs/architecture/gui-client.md` (the queue row looks paragraph that names the strike) and `docs/invariants/38-repeated-gui-shapes-have-one-mechanics-owner.md` (the `played_title_tone` bullet). Name `play_mark.rs` (`RowPlay`, `PlayMark`) as the one played-state derivation and mark that the flat row, the tree row, the Workspace table, and the queue use. Verify: `rg -n "played_title_tone|line-through|bg5. strike" docs` returns nothing.
- [ ] 6.2 Run `make gui-css` and commit any change to `crates/mbv-gui/assets/app.generated.css`. Then run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-gui`, each in the foreground, with no pipe. Verify: all four commands exit 0.

## Workflow follow-up

- The user does the live look check of the played and unplayed rows in the GUI window before the PR opens.
- Sync the `gui-client` delta into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.

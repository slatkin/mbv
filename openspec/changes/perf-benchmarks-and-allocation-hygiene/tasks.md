## 1. Benchmarks and baseline

- [x] 1.1 Add `criterion` to `[workspace.dependencies]` and as a dev-dependency of `mbv-render` and `mbv-components`. Add `[profile.bench] debug = 1` to the workspace `Cargo.toml`. → verify: `cargo check -p mbv-render -p mbv-components --benches`
- [x] 1.2 `crates/mbv-render/benches/`: bench `feed_display_rows` (entries spread across several age groups) and the status-pill title row through its public painter onto a `TestBackend` (see design.md for the fallback). → verify: `cargo bench -p mbv-render --no-run`
- [x] 1.3 `crates/mbv-components/benches/`: bench `TvContent::show_targets` with a catalog that includes duplicate ids. → verify: `cargo bench -p mbv-components --no-run`
- [x] 1.4 Write `docs/architecture/performance.md` (hot paths, bench commands). Commit 1.1–1.4. → verify: `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] 1.5 Ask the user to run the benches and paste the baseline medians (they run manual and live commands; do not run `cargo bench` unasked). Put the numbers in the PR description.

## 2. Capacity (M-INITIAL-CAPACITY)

- [ ] 2.1 Add `with_capacity` at the sites in design.md: `show_targets`, `append_episode_entries`, the season occurrence map in `tree_projection`, `feed_display_rows`, `music_content/tree_target.rs:193`, `tv_content/episode_rows.rs:102`. Skip a site where the bound is not known. → verify: `cargo nextest run -p mbv-render -p mbv-components`

## 3. Allocation reuse (M-MEM-REUSE)

- [ ] 3.1 Rewrite `status_pill_spans` in one pass with no `status_indicators` clone; output identical to the old spans. If no existing test pins content, fg, and bg of the pill, add one named for #896. → verify: `cargo nextest run -p mbv-render`
- [ ] 3.2 Rewrite `tree_row_title` and the padding span in `components/tree_browser.rs` to avoid per-row owned strings (change `TreeRowSpans` first if the signature must change). → verify: `cargo nextest run -p mbv-render -p mbv-components`

## 4. Fast hasher (M-FAST-HASHER)

- [ ] 4.1 Grep `.iter()`, `.keys()`, `.values()`, and `.drain()` on the arena and image-cache maps; confirm no ordering dependency. Add `foldhash` to `[workspace.dependencies]`, `mbv-components`, and `mbv-images`. → verify: `cargo check -p mbv-components -p mbv-images`
- [ ] 4.2 Switch the arena fields in `list/tree_browser.rs` and `card_image_states` / `card_image_loading` in `mbv-images/src/cache.rs` to `foldhash` maps. → verify: `cargo nextest run -p mbv-components -p mbv-images`

## 5. Finish

- [ ] 5.1 Ask the user to rerun the benches; compare with the baseline. Revert any fix from groups 2–4 that shows no gain beyond noise. List kept and dropped fixes in the PR description.
- [ ] 5.2 `cargo fmt`, then `cargo clippy --workspace --all-targets -- -D warnings` → verify: clean
- [ ] 5.3 Commit. Do not push.

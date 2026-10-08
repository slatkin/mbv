# Proposal

## Why

Issue #896: the performance rules are unmet in one coherent way. There are no benchmarks, so nothing justifies any performance claim (`M-HOTPATH`). The draw and projection paths allocate per row per frame (`M-MEM-REUSE`), build collections from known sizes with `new` (`M-INITIAL-CAPACITY`), and key internal maps with the default hasher (`M-FAST-HASHER`). The allocation and hasher findings are structural, not measured. A benchmark baseline comes first, so each fix is shown to help or is dropped.

## What Changes

- Add `criterion` benchmarks for three hot paths: `feed_display_rows` (`mbv-render`), `TvContent::show_targets` (`mbv-components`), and the status-pill title row in `mbv-render` (reached through `padded_status_pill`). Add `[profile.bench] debug = 1`. Add a short `docs/architecture/performance.md` that lists the hot paths and how to run the benches.
- Remove per-frame allocation in `status_pill_spans` (`chrome_player/title/transport.rs`): the `status_indicators` clone, the double `to_string()` re-wrap, and the repeated uppercase pass.
- Remove per-row `String` allocation in `components/tree_browser.rs`: `tree_row_title` and the `" ".repeat(..)` padding span.
- Use `with_capacity` where the size is known: `show_targets`, `append_episode_entries`, the season occurrence map in `tree_projection`, `feed_display_rows`, and the cited sites in `music_content/tree_target.rs` and `tv_content/episode_rows.rs`.
- Use a `foldhash` map for the tree-browser arena (`mbv-components`) and for the `mbv-images` card-image cache.
- Each fix is kept only if the baseline bench or a reasoned allocation count supports it. A fix with no measurable gain is reverted and noted in the PR.
- Rendered output must stay identical. **No BREAKING changes.**

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. No spec-level behavior changes, so the change sets `skip_specs: true`.

## Impact

- New dev-dependency `criterion` in `mbv-render` and `mbv-components`; `foldhash` becomes a direct dependency of `mbv-components` and `mbv-images` (it is already in `Cargo.lock`).
- Code: `crates/mbv-render/src/{components/chrome_player/title/transport.rs, components/tree_browser.rs, screens/feeds_model.rs}`, `crates/mbv-components/src/{tv_content/tree_projection.rs, tv_content/episode_rows.rs, music_content/tree_target.rs, list/tree_browser.rs}`, `crates/mbv-images/src/cache.rs`, new `benches/` directories, workspace `Cargo.toml`, `docs/architecture/performance.md`.
- Out of scope: caching `show_targets` across lookups (`resolve_show_target` re-runs it per call; revisit only if the bench shows it matters), CPU-time measurement across threads, and the rules the issue lists as clean (M-THROUGHPUT, M-SHRINK-TO-FIT, M-BOX-DST, M-AVOID-INDIRECTION, M-LOG-OVERHEAD).

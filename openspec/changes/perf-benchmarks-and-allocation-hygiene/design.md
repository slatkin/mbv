# Design

## Context

See proposal.md. Rules: `docs/standards/rules/M-HOTPATH.md`, `M-MEM-REUSE`, `M-INITIAL-CAPACITY`, `M-FAST-HASHER`. Observed in code:

- `status_pill_spans` clones `ctx.status_indicators`, uppercases each span into a new `String`, then rebuilds each span again with `to_string()` for the fg change and a third time for the pill bg. It runs once per painted frame through `padded_status_pill` (only caller: `title_row.rs`).
- `tree_row_title` returns `(String, String)` per visible row; `tree_row_spans` also builds a padding `String` with `" ".repeat`.
- `show_targets` sorts all shows and builds two `HashMap::new()` per call, and `resolve_show_target` calls it for every lookup. `tree_projection` already uses `with_capacity` for its output vec, but not for the occurrence maps.
- The arena in `list/tree_browser.rs` is `HashMap<usize, ArenaNode<_>>`, built with `with_capacity` at reconcile time (line ~360) and `HashMap::new()` at construction. The image cache is `HashMap<String, CachedImage>` in `mbv-images/src/cache.rs`.
- `foldhash` 0.1 and 0.2 are already in `Cargo.lock` as transitive dependencies. No manifest depends on it directly.
- AGENTS.md: no bespoke scripts, hermetic unit tests, no lint suppression, clippy `-D warnings` runs with `--all-targets`, so benches must pass pedantic clippy.

## Goals / Non-Goals

**Goals:**
- A reproducible baseline for the three hot paths, run with `cargo bench -p <crate>`.
- Fewer allocations in the cited draw and projection paths with identical output.
- Fast hashing for internal keys in the arena and the image cache.

**Non-Goals:**
- No allocation-counting harness, no CPU-time-over-threads measurement, no CI bench job. The benches are run by hand.
- No algorithmic change (`show_targets` caching, memoised projection).
- No change to `HashMap` uses whose keys come from the server or the user.

## Decisions

- **Benches first, as the first commit.** Record baseline medians in the PR description, not in the repo, since numbers depend on the machine. After each fix group, rerun and compare. A fix with no gain beyond noise is reverted. Rejected: fixing first, which is what M-HOTPATH warns against.
- **criterion, not divan.** criterion is the first option the rule names and has the more common tooling. It is a dev-dependency only, declared once in `[workspace.dependencies]`.
- **Bench locations.** `crates/mbv-render/benches/` for `feed_display_rows` and the status pill; `crates/mbv-components/benches/` for `show_targets`. The status pill is private (`pub(super)`), so the bench drives it through the public title-row painter onto a ratatui `TestBackend` buffer. The implementer finds the public entry that reaches `title_row.rs`; if none is public, benchmark `feed_display_rows` and `show_targets` only and record the gap in `docs/architecture/performance.md` rather than widening visibility. Fixtures are built in the bench with `mbv-emby-model`'s `test` feature; no real services or files (benches are manual runs, like live tests, and are never part of `cargo nextest`).
- **Status pill: one pass.** Build each output span once: uppercase and style in a single loop, applying the caption fg and the pill bg together. Borrow the `status_indicators` slice instead of cloning it. Output spans stay `Span<'static>`. The result must equal the old spans exactly (content, fg, bg, modifiers, the leading pad, and the trailing pad from `padded_status_pill`).
- **Tree rows: no per-row owned strings where a borrow works.** `tree_row_title` returns the prefix depth and a `Cow<'_, str>` (borrowed for `Node`, owned only for the uppercased `Heading`). The prefix and the right-pad become `Span`s built from a reused spaces slice when the width fits a static run, otherwise `" ".repeat`. If the borrow forces a signature change in `TreeRowSpans`, change the struct (the single source of truth) before the callers.
- **`with_capacity` bounds.** `show_targets`: both maps get `shows.len()`. `append_episode_entries`: the occurrence map and `entries` reserve `episodes.len()`. Season occurrence map: `detail.seasons.len()`. `feed_display_rows`: `entries.len()` (a loose upper bound; the headings and spacers add at most about 2 per age group, so use `entries.len() + 8`, as `tree_projection` does). `tree_target.rs:193` and `episode_rows.rs:102` get the input length where it is known at that line; skip any site where the bound is not known.
- **Hasher.** Use `foldhash::HashMap` / `foldhash::HashSet` (the `fast` or `quality` variant is the implementer's choice; default to `fast` for the `usize` arena ids and `quality` for `String` image keys). Construct with `default()` or `with_capacity_and_hasher(n, Default::default())`. Apply only to the arena fields (both struct fields at lines 124 and 142 of `list/tree_browser.rs`) and `card_image_states` / `card_image_loading` in `cache.rs`. Rejected: a workspace-wide find/replace of `HashMap`, which would also rehash keys that come from the server.
- **Iteration order.** `std::HashMap` iteration order is already unspecified, so the arena and the cache must not depend on it. The implementer greps `.iter()`, `.keys()`, `.values()`, and `.drain()` on these maps before swapping and confirms no ordering dependency.
- **Docs.** `docs/architecture/performance.md`: the hot paths, the bench commands, and the `profile.bench` note. Keep it under one page. Commit it with the code.
- **Tests.** No test inflation. The existing `mbv-render` and `mbv-components` suites are the regression gate. Add a test only if no existing test pins the status-pill output (content, fg, bg) or `tree_row_title` output; then add one test per function, named after #896.

## Risks / Trade-offs

- [Rewrite changes rendered output] → Existing render tests run before and after. Pin the status pill output with one test if no existing test covers it.
- [Micro-gains are below noise] → Revert that fix (see Decisions). The PR lists what was kept and dropped.
- [A new direct dependency] → `foldhash` is already in `Cargo.lock`, so there is no new crate to vet. `criterion` is dev-only.
- [`cargo clippy --all-targets` now lints benches] → Write benches to pass pedantic clippy, with no suppressions.
- [The status pill cannot be benched without widening visibility] → Skip it and document the gap, as described above.

# Performance baselines

Issue #896: the allocation and hasher fixes in this change are structural,
not measured. These benchmarks are the baseline each fix is judged against:
a fix with no gain beyond noise is reverted.

## Hot paths

- `feed_display_rows` (`mbv-render`): projects the "All" feed list into
  display rows. Bench fixture: 500 entries spread across every age group
  plus undated entries.
- Status-pill title row (`mbv-render`): the private `status_pill_spans`,
  driven through its public painter `render_title_row` onto a ratatui
  `TestBackend`. No visibility was widened to reach it.
- `TvContent::show_targets` (`mbv-components`): sorts the show-mode
  catalog and disambiguates duplicate Emby ids. Bench fixture: 200 shows
  sharing a pool of 20 ids, mixed with uniquely identified shows.
- GUI core step (`mbv-gui`, bench `benches/sections.rs`): one `CoreInput`, one
  `CoreThread::step`, one emitted frame, over seven fixtures built from the
  `mbv` `test`-feature stubs and the in-memory `MockHttp` — an idle 50-slot
  queue, a playing 50-slot queue, a Grouped Music library with 2,000 artists
  and 6,000 albums (one picked), the Shows All mode with 300 series, Home with
  50 Continue rows and 100 recent rows, Search with 200 results, and Settings
  on its F2 section. Each fixture also reports its emitted frame's
  `serde_json` size as criterion `Throughput::Bytes`.
- `LibraryList` row derivation (`crates/mbv-gui/ui`, bench
  `src/lib/library/rows.bench.ts`, vitest `bench`): 100 scroll steps of 50 rows
  each through `scrollLibraryList` over 5,000 closed artists under 26 letter
  headings. The content and the viewport are set once at module load, so the
  bench times only the derivation.

## Running the benches

```sh
cargo bench -p mbv-render
cargo bench -p mbv-components
cargo bench -p mbv-gui --bench sections
```

The front-end bench runs from `crates/mbv-gui/ui`:

```sh
pnpm vitest bench --run
```

Benches are manual runs, never part of `cargo nextest`. Baseline medians
live in the PR description, not here: the numbers depend on the machine.
`[profile.bench] debug = 1` keeps readable criterion profiles without
paying for full debug builds.

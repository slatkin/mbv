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
  `CoreThread::step`, one emitted batch, over seven fixtures built from the
  `mbv` `test`-feature stubs and the in-memory `MockHttp` — an idle 50-slot
  queue, a playing 50-slot queue, a Grouped Music library with 2,000 artists
  and 6,000 albums (one picked), the Shows All mode with 300 series, Home with
  50 Continue rows and 100 recent rows, Search with 200 results, and Settings
  on its F2 section. The batch crosses the window channel as a Rust value, so
  the bench reports no payload size.
- `LibraryList` row derivation: the TypeScript bench of the removed Svelte
  front end (100 scroll steps through `scrollLibraryList` over 5,000 closed
  artists). No Rust bench replaces it yet.

## Running the benches

```sh
cargo bench -p mbv-render
cargo bench -p mbv-components
cargo bench -p mbv-gui --bench sections
```

Benches are manual runs, never part of `cargo nextest`. Baseline medians
live in the PR description, not here: the numbers depend on the machine.
`[profile.bench] debug = 1` keeps readable criterion profiles without
paying for full debug builds.

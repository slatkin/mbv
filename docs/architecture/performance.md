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

## Running the benches

```sh
cargo bench -p mbv-render
cargo bench -p mbv-components
```

Benches are manual runs, never part of `cargo nextest`. Baseline medians
live in the PR description, not here: the numbers depend on the machine.
`[profile.bench] debug = 1` keeps readable criterion profiles without
paying for full debug builds.

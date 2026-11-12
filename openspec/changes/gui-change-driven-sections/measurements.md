# Measurements (design D12)

Machine: Intel Core i7-14700K. Criterion medians are point estimates from
`target/criterion` (`estimates.json`, `benchmark.json`). Criterion runs 100
samples after a 3 s warm-up per fixture.

## Baseline

Run before any task 2.x change lands. Each later task re-runs the bench that
covers its fix and adds its medians to this file.

### Rust: `cargo bench -p mbv-gui --bench sections`

One timed unit: one `CoreInput` (a key with no default binding), one
`CoreThread::step`, one emitted frame. The bytes are the frame's `serde_json`
size at steady state, measured outside the timed loop; a section rides a frame
only when its content changed, so the timed step after the seeding step carries
none of it.

| Fixture | Median step | Bytes per step |
| --- | --- | --- |
| `idle_queue` (50 slots, nothing playing) | 8.7 µs | 10,318 |
| `playing_queue` (50 slots, active playing) | 9.0 µs | 10,458 |
| `music_grouped` (2,000 artists, 6,000 albums, one picked) | 344.1 µs | 1,253 |
| `shows_all` (300 series, All mode) | 6.4 µs | 1,253 |
| `home_rows` (50 Continue rows, 100 recent rows) | 44.6 µs | 1,445 |
| `search_open` (query `bench`, 200 results) | 71.9 µs | 17,109 |
| `settings_f2` (Settings on its F2 section) | 6.9 µs | 1,904 |

### Front end: `pnpm vitest bench --run` (in `crates/mbv-gui/ui`)

`LibraryList` row derivation under scrolling: 100 scroll steps of 50 rows each
through `scrollLibraryList` over 5,000 closed artists under 26 letter
headings. Vitest reports a mean, not a median; this is the bench's only
statistic.

| Bench | Mean per pass | Per scroll step |
| --- | --- | --- |
| 100 scroll steps over 5,000 rows | 39.8 ms | ≈ 0.40 ms |

64 samples, relative margin of error ±3.10%.

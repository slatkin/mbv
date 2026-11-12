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

## Task 5.5: artwork through the `mbv-art` scheme

Run at HEAD `bb3323426` (rows 5.1 to 5.4 landed, task 5.5 uncommitted), on the
same machine, with `timeout 900 cargo bench -p mbv-gui --bench sections`. The
table holds the two fixtures that rows 5.x touch: the playing queue (fixture 2)
and the Music step (fixture 3). The medians are `median.point_estimate` in
`target/criterion/sections/<fixture>/new/estimates.json`.

| Fixture | Baseline median step | Median step | Baseline bytes per step | Bytes per step |
| --- | --- | --- | --- | --- |
| `playing_queue` (50 slots, active playing) | 9.0 µs | 9.63 µs | 10,458 | 2 |
| `music_grouped` (2,000 artists, 6,000 albums, one picked) | 344.1 µs | 249.8 µs | 1,253 | 2 |

The bytes column no longer compares with the baseline. The baseline measured
a full frame. The sections contract sends a section only when it changes, so
the steady-state timed step carries an empty batch, which serializes to 2
bytes. A bench step that changes artwork is not timed here, so this run does
not measure the size of an artwork reference or the data URLs that rows 5.3 and
5.4 removed from the payload.

The Music step fell by 27% against the baseline. The playing-queue step rose
by about 7%, which is within the spread that the earlier rows left. The step
times include the ledger's compared sections from rows 2.x to 4.x, so the change
is not the effect of task 5.5 alone.

## Task 6.6: Library sections

Run at HEAD `a9e3b85f1` (rows 6.1 to 6.5 landed, task 6.6 uncommitted), on the
same machine, with `timeout 900 cargo bench -p mbv-gui --bench sections`. The
run covered all seven fixtures. The table holds fixtures 3, 4, and 5, the three
that read the Library sections. The medians are `median.point_estimate` in
`target/criterion/sections/<fixture>/new/estimates.json`.

| Fixture | Baseline median step | Median step | Baseline bytes per step | Bytes per step |
| --- | --- | --- | --- | --- |
| `music_grouped` (2,000 artists, 6,000 albums, one picked) | 344.1 µs | 242.3 µs | 1,253 | 2 |
| `shows_all` (300 series, All mode) | 6.4 µs | 6.68 µs | 1,253 | 2 |
| `home_rows` (50 Continue rows, 100 recent rows) | 44.6 µs | 40.7 µs | 1,445 | 2 |

The bytes column has the same meaning as in Task 5.5. The timed key marks every
gated section, so each timed step runs the one Library pass and compares its
three slots. Nothing changed, so the batch is empty and serializes to 2 bytes.
This run does not time a pick or a row-menu open, which each send one section.

Against the baseline, the Music step fell 30% and the Home step fell 9%. Against
the Task 5.5 run, the Music step fell 3% (249.8 µs). The Shows step rose 4%
over the baseline. Criterion's change line compares with the last saved run in
`target/criterion`, and it reports Music −3.3%, Home −2.7%, and Shows −0.7%, so
the Shows rise is not a regression between the last two runs. The step times
include the compared sections from rows 2.x to 6.x, so the change is not the
effect of task 6.6 alone.

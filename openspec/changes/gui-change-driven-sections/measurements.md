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

## Final: task 9.1, all eight benches

Run at HEAD `5e39ffb42` (tasks 1.1 to 8.3 landed), on the same machine, with
`timeout 1200 cargo bench -p mbv-gui --bench sections` and
`timeout 600 pnpm vitest bench --run --reporter=verbose` in
`crates/mbv-gui/ui`. The Rust medians are `median.point_estimate` in
`target/criterion/sections/<fixture>/new/estimates.json`. Vitest reports a mean,
not a median, so the front-end row has a mean.

| Fixture | Baseline median step | Final median step | Baseline bytes per step | Final bytes per step | Same-session before run |
| --- | --- | --- | --- | --- | --- |
| `idle_queue` (50 slots, nothing playing) | 8.7 µs | 8.98 µs | 10,318 | 2 | None (not measured before) |
| `playing_queue` (50 slots, active playing) | 9.0 µs | 9.30 µs | 10,458 | 2 | Task 5.5: 9.63 µs (after) |
| `music_grouped` (2,000 artists, 6,000 albums, one picked) | 344.1 µs | 264.4 µs | 1,253 | 2 | Task 5.5: 249.8 µs; task 6.6: 242.3 µs (note 3) |
| `shows_all` (300 series, All mode) | 6.4 µs | 6.65 µs | 1,253 | 2 | Task 6.6: 6.68 µs |
| `home_rows` (50 Continue rows, 100 recent rows) | 44.6 µs | 41.9 µs | 1,445 | 2 | Task 6.6: 40.7 µs |
| `search_open` (query `bench`, 200 results) | 71.9 µs | 29.3 µs | 17,109 | 2 | Task 7.1: 49.3 µs (before), 29.4 µs (after) |
| `settings_f2` (Settings on its F2 section) | 6.9 µs | 6.96 µs | 1,904 | 2 | Task 7.2: 7.18 and 7.11 µs (before), 6.97 and 6.96 µs (after) |

| Bench | Baseline mean per pass | Final mean per pass | Per scroll step (final) | Same-session before run |
| --- | --- | --- | --- | --- |
| 100 scroll steps over 5,000 rows | 39.8 ms | 0.0478 ms | ≈ 0.48 µs | Task 8.1: 34.48 ms (before), 0.046 ms (after) |

The final front-end run took 20,925 samples at ±0.16%.

Notes:

1. Some baseline numbers do not reproduce on this machine today. `search_open`
   measured 49.3 µs before task 7.1 in the same session, against the 71.9 µs
   baseline. The front end measured 34.48 ms before task 8.1 in the same session,
   against the 39.8 ms baseline. The baseline columns above are unchanged. Compare
   a final value with its same-session before run, not with the baseline alone.
2. The bytes per step changed meaning. The baseline bytes measured a full frame at
   steady state. A section now rides a frame only when it changes, so the
   steady-state timed step sends an empty batch, which serializes to 2 bytes. The
   final bytes column does not show a size reduction against the baseline. It
   shows that the timed unit carries no section payload.
3. The final Music median is 264.4 µs. A rerun of that fixture alone gave 264.2 µs.
   That is 9% above the task 6.6 run and 6% above the task 5.5 run. Criterion's
   change line against the last saved run reports +9.4%, "Performance has
   regressed". Between task 6.6 and HEAD, only tasks 7.1 and 7.2 change Rust under
   `src/` and `crates/mbv-gui/src/`. Both edit the Search and Settings projections
   and the config read. The Library scroll tasks of section 8 are front end only.
   The cause is not found. Open question for the parent.

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

## Task 7.1: the Search projection

Run at HEAD `dbcc74804` (before) and with task 7.1 applied (after), on the same
machine, with `timeout 900 cargo bench -p mbv-gui --bench sections -- search_open`.
The before run was taken in the same session, because the medians that earlier
runs saved no longer match this machine's timings. The table holds fixture 6,
the one that reads the Search section. The fixture's query is open, so the
empty-query `None` does not run in it. The timed change here is the rows moved
into the frame instead of cloned a second time.

| Fixture | Before median step | Median step | Bytes per step |
| --- | --- | --- | --- |
| `search_open` (query `bench`, 200 results) | 49.3 µs | 29.4 µs | 2 |

The bytes column has the same meaning as in Task 5.5: the timed step is steady
state, so the batch is empty and serializes to 2 bytes.

Criterion's change line, after against before, is time −39.6% (p = 0.00), with
"Performance has improved" outside the noise threshold. The gain is kept. Before
this task, `gui_search_projection` cloned the rows, and `GuiSearchFrame::carried`
cloned them again. Now the projection clones once and the frame moves the rows.

Fixture 7 was measured at the post-7.1 state as the before for Task 7.2. Its
median was 7.17 µs. Criterion reported −1.1% against the prior run, within the
noise threshold, so the Search change does not move the Settings step.

## Task 7.2: the Settings projection

Run at HEAD `4c7c93efb` (task 7.1 committed, task 7.2 uncommitted) and then with
task 7.2 applied, on the same machine, with
`timeout 900 cargo bench -p mbv-gui --bench sections -- settings_f2`. Each state
was run twice in one session. The medians are `median.point_estimate` in
`target/criterion/sections/settings_f2/new/estimates.json`. The table holds
fixture 7, the Settings step.

| Fixture | Before median step (two runs) | Median step (two runs) | Bytes per step |
| --- | --- | --- | --- |
| `settings_f2` (Settings on its F2 section) | 7.18 µs, 7.11 µs | 6.97 µs, 6.96 µs | 2 |

The bytes column has the same meaning as in Task 5.5: the timed step is steady
state, so the batch is empty and serializes to 2 bytes.

Criterion's change line, after against before, is time −2.9% (p = 0.00), with
"Performance has improved". Two runs of the same code gave a change line of
−1.1% (within the noise threshold) for the before state, and "No change in
performance detected" for the after state. The gain is about 2.7% of the step,
and each state reproduces within about 1%, so the gain is kept.

The three parts were not measured one at a time. `App::with_config` and
`App::gui_settings()` borrow the configuration and the state where they used to
clone them, which removes clones from the Settings path. `gui_settings_row_choices`
returns a borrowed slice, but the row DTO still copies it once, as before, so it
saves no copy by itself. It is kept because design D10 names that shape.

## Task 8.1: the Library scroll store

Run at HEAD `89edf1b53` (before) and with task 8.1 applied (after), on the same
machine, with `timeout 600 pnpm vitest bench --run --reporter=verbose` in
`crates/mbv-gui/ui`. The bench is the one in `rows.bench.ts`. Vitest reports a
mean, not a median; this is the bench's only statistic. Before, the run took 64
samples with a relative margin of error of ±1.19%. After, the two runs took
about 21,500 samples each, at ±0.20%.

| Bench | Before mean per pass | Mean per pass | Per scroll step (after) |
| --- | --- | --- | --- |
| 100 scroll steps over 5,000 rows | 34.48 ms | 0.046 ms | ≈ 0.46 µs |

The before run was taken in this session, so it differs from the 39.8 ms
baseline above, which was taken on an earlier run. Before this task, each step
rebuilt the 5,000 canvas rows (≈ 0.34 ms per step). After it, a step moves only
the scroll store, and the rows come back from the memo, so the clamp is the
only work left.

**Music bench check (after task 9.1).** In one session, `music_grouped` ran 259.8 µs at HEAD `bae0a17e4` and 276.2 µs with the task 7.1 and 7.2 code put back to `dbcc74804`. The pre-7.x code is not faster, so tasks 7.1 and 7.2 did not cause the rise against the task 6.6 run (242.3 µs). The `music_grouped` code at `dbcc74804` is the same as at the task 6.6 run, so the same code gave 242.3 µs in that session and 276.2 µs in this one. The cause of that difference between sessions is not known.

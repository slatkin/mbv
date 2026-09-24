# Invariant 8 — Server-row accounting is distinct from retained browse items

**Scope:** `BrowseLevel` pagination and exhaustion (`src/app/state/types/browse.rs`,
`src/app/dispatch/library/event.rs`, `src/app/dispatch/library/browse.rs`, and
`src/app/dispatch/library/search.rs`).

## The invariant

`BrowseLevel::fetched_rows` is the pagination and exhaustion truth: it counts
server rows consumed for the level, including rows omitted from `items` by a
client-side filter. `items` is the retained browse presentation and may contain
fewer rows than the server reported. `total_count` remains the server's count,
not the retained-item count.

`is_fully_loaded`, the next page's `start_index`, and the exhaustion guard must
therefore all use `fetched_rows`:

- `is_fully_loaded` is true when every server row has been consumed;
- page requests start at the number of server rows already consumed; and
- a page containing only filtered rows still terminates pagination when its
  server-row count reaches `total_count`.

## Why it matters

Using `items.len()` for exhaustion after filtering can either keep requesting
already-consumed rows or treat a complete level as partial. Both cases break
browse navigation: the former can refetch forever, while the latter delays
search/prefetch decisions and can make a complete music level appear to grow.

## How the code maintains it today

`dispatch/library/event.rs` records each page's pre-filter length in `fetched_rows`
before grouped-music filtering and adds that count to the level. The browse
page helper keeps the same server-row accounting, and pagination uses the
resulting value for its next offset and exhaustion checks. Restored positions
carry the persisted value when available and fall back to the retained item
count for older snapshots that predate this field.

The PR #744 P1 was the remaining failure: `BrowseLevel::is_fully_loaded()`
still compared `items.len()` with `total_count` even though filtering could
make those values incomparable. It now compares `fetched_rows` with
`total_count`; tests cover both complete and incomplete filtered levels.

## What breaks if it is violated

- A filtered page can be requested again from the wrong offset.
- A fully consumed level can remain apparently incomplete, causing stale
  cursor-growth and whole-library prefetch decisions.
- A filtered final page can fail to stop paging because its retained item count
  never reaches the server total.

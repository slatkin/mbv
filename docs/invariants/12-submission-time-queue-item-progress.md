# Invariant 12 — Submission-time queue items must carry current progress, never a cached snapshot

**Scope:** every code path that materializes a `QueueItem` for play or
enqueue — in particular `src/app/dispatch/audiobookshelf/browse/mod.rs`'s
`selected_audiobookshelf_queue_item_target` (both its shelf-cache-hit branch
and its build-from-episode fallback), and the cache sources those paths read
(`audiobookshelf_shelf_cache`; `AudiobookshelfQueueItem` construction in
`crates/mbv-core/src/audiobookshelf_catalog.rs`).

## The invariant

Any code path that materializes a `QueueItem` for play or enqueue must apply
the current progress (`position_ticks` / `played` / `is_finished`) from the
owning browse/state source at submission time. A cached or
previously-constructed item must never be submitted verbatim: before a hit
from any item cache (shelf cache, per-show cache, detail cache) is returned
as a queue item, it must be patched from the same progress lookup the
build-from-scratch fallback path uses.

## Why it matters

Cache entries are built at different times and for different purposes than
queue submission, and their progress fields are not maintained by the
progress pipeline. The Audiobookshelf shelf cache's items are constructed
with zeroed progress (`position_ticks: 0`, `played: false`,
`is_finished: false` in `crates/mbv-core/src/audiobookshelf_catalog.rs`),
because the catalog payload they mirror carries no per-user progress. The
browse state's `state.progress` map, by contrast, is kept current.

## What breaks if it is violated

`selected_audiobookshelf_queue_item_target` used to return a shelf-cache hit
verbatim, before consulting `state.progress`. Because shelf items are built
with zeroed progress, every podcast play or enqueue that resolved through the
cache resumed from tick 0 — a partially-played episode started over from the
beginning — while the same episode resolved through the fallback path
(correctly) resumed from its actual position. Which path a submission took
depended purely on whether the episode happened to be in the shelf cache,
so the same user action had two different resume behaviours.

Fixed in commit `50c4d12d` ("fix: preserve podcast shelf resume progress"):
the cache-hit branch now patches the cloned item's `position_ticks`,
`played`, and `is_finished` from the same `state.progress` lookup the
fallback path uses, before returning it.

## How the code maintains it today

- `selected_audiobookshelf_queue_item_target` performs **one** progress
  lookup into `state.progress` for the `(library_item_id, episode_id)`
  identity up front, and both branches consume it: the shelf-cache-hit
  branch overwrites the cached item's `position_ticks` / `played` /
  `is_finished` from it, and the fallback branch seeds a freshly-constructed
  `AudiobookshelfQueueItem` with the same values.
- The cache-hit branch clones the cached item and patches the clone; it
  never mutates the cache entry itself. Cache entries stay zeroed; the
  correction happens only on the submission copy.

## Where it currently fails

No known violation. However, nothing structural prevents a future cache
source from being read as a queue item without this patching step — the
guarantee lives in the shape of `selected_audiobookshelf_queue_item_target`
(one shared lookup, both branches consume it), not in the type system. Any
new materialization path (a new cache, a new provider's play action) must
repeat this pattern: resolve progress from the owning state source at
submission time, and never return a cached item unpatched.

Note this invariant is the **submission-time counterpart** to
[Invariant 6](06-queue-progress-application-sites.md), which covers the
*post-submission* mirrors (daemon canonical queue, `ExecutionSequence`,
shell `PlaybackQueue` mirror): Invariant 6 ensures progress keeps flowing
into the queue after an item is submitted; this invariant ensures the item
enters the queue with correct progress in the first place. Neither can
substitute for the other — a zeroed item that is submitted verbatim is
stale in every mirror until something downstream corrects it, and the
submission-time sources (`state.progress`) are the only authority that has
the answer at that moment.

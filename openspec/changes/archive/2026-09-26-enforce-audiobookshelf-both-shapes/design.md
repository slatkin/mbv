# Design

## Context

`QueueItem` (`crates/mbv-core/src/playback/queue/items.rs`) is a flat
four-variant enum. Its `Serialize` is derived (`#[serde(tag = "kind")]`) and
its `Deserialize` is hand-written, which also accepts legacy untagged Emby
payloads. `QueueItemKind` is a flat projection. Its only external consumer is
`src/app/state/context_menu_capabilities.rs`, which compares against `Emby`.

These sites handle the shapes differently on purpose, and each one stays
shape-aware:

- `daemon/control_queue.rs` and `player/proxy.rs`: `abs-queue` and
  `abs-book-queue` are separate negotiated capabilities.
- `player/sources.rs` (`prepare_episode_source` / `prepare_book_source`),
  `player/reporting.rs` (`ActiveItemLifecycle`), `player/run/decisions.rs`,
  `player.rs`, and `player/types.rs`.
- Progress events: `AudiobookshelfProgress` / `AudiobookshelfBookProgress`.
- `cast/dispatch.rs`, and the `src/app/dispatch/audiobookshelf/browse.rs`
  episode lookups (shelf cache).

## Goals / Non-Goals

**Goals:**
- A site that asks "is this Audiobookshelf-owned?" cannot compile
  episode-only by accident.
- Persisted and wire JSON stays byte-compatible.
- Invariant 04 is no longer needed as a doc.

**Non-Goals:**
- Folding in invariant 06 (progress mirrors). It stays a separate #810 child.
- Merging the two capability gates, or the two progress event types.
- Renaming `AudiobookshelfQueueItem` / `AudiobookshelfBookQueueItem`.

## Decisions

1. **Nested enum, not a predicate.** `QueueItem::Audiobookshelf(AudiobookshelfItem)`
   with `AudiobookshelfItem::{Episode, Book}`.
   *Alternatives:* an exhaustive `provider()` accessor still lets a raw
   variant match skip books, and docs-only enforces nothing. Both were
   rejected (user decision).
2. **Hand-written `Serialize`.** It emits the existing flat `kind` tags by
   serializing the inner struct with a `kind` field added, mirroring the
   existing `Deserialize`. `Deserialize` maps `"Audiobookshelf"` →
   `Episode` and `"AudiobookshelfBook"` → `Book`. Deriving `Serialize` on a
   nested enum would change the JSON shape, which would break the redb/json
   state and ctrl peers.
3. **`AudiobookshelfItem` owns the shape-shared accessors.** Methods such as
   `title`, `duration_ticks`, `position_ticks`, `content_id`, `cover_path`
   and `is_played` are implemented once on `AudiobookshelfItem` (matching
   both shapes inside), and `QueueItem` delegates to them. This removes the
   duplicated `Audiobookshelf(_) | AudiobookshelfBook(_)` arms in `items.rs`.
4. **Predicates.** `is_audiobookshelf()` becomes the both-shapes predicate.
   `is_audiobookshelf_any()` and `is_audiobookshelf_book()` are deleted. The
   two capability gates match on `QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(_))`
   or `Book(_)` directly, so the shape they test is visible where they test
   it.
5. **`QueueItemKind` stays flat** (`Audiobookshelf`, `AudiobookshelfBook`).
   It is a presentation/wire discriminator. `kind()` derives it from the
   nested shape.
6. **URL resolution narrowing.** `mpv_url_for_queue_item` takes a
   `MpvUrlSource<'_>` enum (`Emby(&EmbyItem)` / `Feed(&FeedEntry)`), built by
   a `QueueItem::mpv_url_source() -> Option<MpvUrlSource>` that returns
   `None` for Audiobookshelf. The caller already routes Audiobookshelf to
   active-file projection, so it handles `None` by rejecting with the
   existing `CommandRejected`/log path instead of panicking. That removes
   both `unreachable!()` arms.

## Risks / Trade-offs

- [Wide mechanical diff, about 45 files] → The compiler drives it: change the
  enum first and fix every error. No logic changes at call sites except the
  ones listed in Decisions 4 and 6.
- [Serialize drift breaks persisted state] → Add round-trip tests. Golden JSON
  for an episode and a book must serialize to the pre-change bytes (field
  order included) and deserialize back to the same shape.
- [ctrl peers on older builds] → Same bytes on the wire, so no protocol bump
  is needed.

## Migration Plan

No data migration. Roll back by reverting the commit, which is safe because
the JSON is unchanged.

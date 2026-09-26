# Proposal

## Why

`docs/invariants/04-audiobookshelf-means-both-shapes.md` exists because
`QueueItem` has two sibling Audiobookshelf variants (`Audiobookshelf` for
episodes, `AudiobookshelfBook` for books). Any site that matches only one of
them silently sends books down the Emby/Feed path, and the compiler says
nothing. All five violations listed in #806 have since been fixed with
regression tests (`01ed6bff0`, `0c31fb9de`, `4b0f6b2fb`). The rule itself is
still unenforced, and the episode-only predicate `is_audiobookshelf()` is
still public and invites the next person to repeat the bug. This change is
the first step of umbrella #810 (replace invariants with types).

## What Changes

- Nest the two shapes under one variant:
  `QueueItem::Audiobookshelf(AudiobookshelfItem)` with
  `enum AudiobookshelfItem { Episode(AudiobookshelfQueueItem), Book(AudiobookshelfBookQueueItem) }`.
  A match on `QueueItem::Audiobookshelf(_)` then covers both shapes by
  construction. Only code with a named shape-specific reason (source
  preparation, lifecycle, the transport/capability gates, progress events,
  cast dispatch) looks inside.
- Remove `QueueItem::AudiobookshelfBook`, `is_audiobookshelf_book()` and
  `is_audiobookshelf_any()`. `is_audiobookshelf()` now means both shapes.
  Shape-specific accessors (`as_audiobookshelf`, `as_audiobookshelf_book`)
  stay as narrowing helpers.
- `mpv_url_for_queue_item` loses its two `unreachable!()` arms: it takes a
  narrowed, non-Audiobookshelf input, so the compiler rules out a book or
  episode reaching it.
- The wire and persisted JSON format does not change: `"kind": "Audiobookshelf"`
  and `"kind": "AudiobookshelfBook"` stay flat tags, using hand-written
  `Serialize` next to the existing hand-written `Deserialize`.
- Replace invariant 04's doc with a short note that the type now enforces it,
  and tick 04 on umbrella #810.

No user-visible behaviour changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a type-level refactor, so the change sets `skip_specs: true`.

## Impact

- `crates/mbv-core`: `playback/queue/items.rs` (enum, serde, helpers) and
  every `QueueItem` match site. Today that is about 45 files and 160 matches,
  spread across player, daemon, config, cast, remote_player and ctrl.
- `src/app`: the Audiobookshelf dispatch/browse sites and the context-menu
  capabilities.
- Persistence (`queue_state.json`, redb) and the ctrl protocol keep the same
  bytes, and round-trip tests prove it.
- `CONTEXT.md` **QueueItem** entry, `docs/invariants/04-*`.

## Context

See proposal.md — Why. Today `QueueSlot.item` is a bare `EmbyItem` (`crates/mbv-core/src/playback_queue.rs:73`) and the play boundary (`player_proxy.rs` / `player_run_*`) is welded to the `EmbyClient` streaming path. Two current couplings shape the approach:

1. **Progress sync mutates the item.** `ProgressState::from_item(&EmbyItem)` and `apply_to_item(&mut EmbyItem)` read/write `playback_position_ticks` and `played` directly on the item (`playback_queue.rs:64-79`). A feed entry has no such fields in this change (deferred to #472).
2. **queue_state.json** serializes the slot item; wrapping it in an enum changes the on-disk shape.

## Goals / Non-Goals

**Goals:**
- One green, compiling checkpoint: the enum lands, all existing Emby queue behavior is byte-for-byte unchanged, and the only new runtime path is "a Feed slot plays its URL."
- Confine the tagged shape to the local queue + `queue_state.json`. Nothing else (ctrl, renderers) learns about `Feed`.

**Non-Goals (design-level):**
- No feed playback state (`position_ticks`, `played`) — #472. This means a Feed slot has no progress to sync.
- No accessor surface beyond what the queue itself reads. Library/home renderers keep taking `EmbyItem`.

## Decisions

**1. `QueueItem` enum wraps the item; `QueueSlot` gains a match, not a rewrite.**
`QueueSlot.item: EmbyItem` → `QueueSlot.item: QueueItem`. Accessors the queue needs (`title`, `duration`, `media_kind`, `artwork_url`) become methods on `QueueItem` that match the variant. Alternative — a trait object `Box<dyn Playable>` — rejected: only two closed variants, an enum is simpler, `Clone`/`Debug`/serde all derive, and matching at the one play site is clearer than dynamic dispatch.

**2. Progress sync guards on the Emby variant.**
`ProgressState::from_item` / `apply_to_item` only apply to `QueueItem::Emby`. For `QueueItem::Feed`, progress sync is a no-op and `ProgressState` stays at its default — there is no feed state to carry in this change. This keeps the `position_ticks`/`played` fields entirely inside `EmbyItem` and out of `FeedEntry`, which is what makes #472 a clean downstream addition rather than a migration.

**3. `FeedEntry` is minimal and lives in `mbv-core`.**
Fields: `guid`, `title`, `enclosure_url: Option<String>`, `link: Option<String>`, `mime_type: Option<String>`, `duration_ticks: Option<u64>`. It sits next to `EmbyItem` (api_types or a new `feed_entry.rs`) so both the queue and #471 consume it. No progress fields — see decision 2.

**4. `queue_state.json`: serde-tagged, read-legacy.**
Serialize always writes the tagged shape (serde `#[serde(tag = ...)]` or externally-tagged enum). Deserialize accepts legacy bare-item JSON as `QueueItem::Emby` via `#[serde(untagged)]`-style fallback or a custom `Deserialize` that tries tagged then bare. A round-trip test is mandatory: legacy bare JSON → loads as Emby; save → tagged; tagged → reloads with kind preserved.

**5. Ctrl stays Emby-only; Feed items are dropped at the boundary.**
The ctrl messages (`AdoptQueue`/`QueueAppend`/`ReplaceQueue`, `Box<EmbyItem>`) keep carrying bare `EmbyItem`. Where the local queue is projected onto a ctrl message, `filter_map` the slots: `QueueItem::Emby(e) => Some(e)`, `Feed => None`. No new capability string, no version bump — feed playback is local-player only. Alternative — a tagged ctrl item behind a capability handshake — deferred until remote feed playback is actually wanted (#472 territory or later).

**6. Play boundary matches once.**
In `player_proxy.rs` / `player_run_*`, match the active slot's `QueueItem`: `Emby` → existing `EmbyClient` streaming-URL resolution, unchanged; `Feed` → `enclosure_url.or(link)` handed to mpv directly (mpv delegates YouTube links to yt-dlp). Resolved synchronously; the player is otherwise untouched.

## Risks / Trade-offs

- **A Feed slot reaching a code path that assumes Emby fields** (e.g. a stray `apply_to_item`, a renderer, a ctrl send) → guard/match is centralized: progress sync guards on Emby (decision 2), ctrl filters Feed out (decision 5), renderers never receive `QueueItem`. Any `EmbyItem`-specific access forces a match, so the compiler flags a missed site.
- **Legacy queue_state.json fails to parse** if the untagged fallback is written incorrectly → the round-trip test with a real pre-change JSON fixture is the gate; treat a parse failure as a broken checkpoint, not a warning.
- **Accessor creep** (adding `album`/`artist`/… to `QueueItem` to satisfy a renderer) → out of scope by decision; if a renderer needs those it is reading `EmbyItem`, not a queue slot. Resist widening the enum's method set.

## Migration Plan

No data migration step — `queue_state.json` is read-legacy on first load and rewritten tagged on next save. Rollback: a previous build reading a tagged `queue_state.json` would fail to parse the new shape; acceptable for a single-user local file (delete it to reset). No wire/protocol change means no coordinated deploy with remote peers.

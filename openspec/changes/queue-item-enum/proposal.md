## Why

The playback queue is just a play mechanism, but today every slot is a bare `EmbyItem` — the queue can only hold and play Emby library items. To play a feed entry (RSS/podcast enclosure, YouTube link) through the same queue, a slot must be able to carry *either* an Emby item or a feed entry and play whichever it holds. This is the enabling seam for the Feeds feature (#471); the rename in #469 (`MediaItem` → `EmbyItem`) was its prerequisite.

## What Changes

- Introduce `QueueItem { Emby(EmbyItem), Feed(FeedEntry) }` and store it in `QueueSlot.item` (`crates/mbv-core/src/playback_queue.rs`), replacing the bare `EmbyItem`.
- Add a **minimal** `FeedEntry` — identity + playback fields only (`guid`, `title`, `enclosure_url`, `link`, `mime_type`, `duration_ticks`). No `position_ticks`, no `played`; feed playback state is deferred to #472.
- Add the small accessor set the queue actually needs (`title()`, `duration()`, `media_kind()`, `artwork_url()`), implemented for both variants. Do **not** grow accessors to cover library-render fields (`album`, `artist`, `series_name`) — the library/home renderers keep reading `EmbyItem` directly.
- Match the variant at the **play boundary** (`player_proxy.rs` / `player_run_*`): `Emby` keeps the existing `EmbyClient` streaming-URL path unchanged; `Feed` hands `enclosure_url` (else `link`) to mpv directly.
- **BREAKING (persistence, back-compat handled):** `queue_state.json` changes from a bare item object to a tagged shape. Deserialize accepts legacy bare items (decode as `QueueItem::Emby`); serialize always writes the tagged shape.
- Keep the **ctrl protocol carrying bare `EmbyItem`** — feed playback is local-player only for now. A `Feed` item that would cross the ctrl boundary to a remote peer is omitted; no new ctrl capability, no wire-shape change.

## Capabilities

### New Capabilities
- `feed-queue-item`: the playback queue can hold and play a feed entry (non-Emby item) alongside Emby items, selecting the play path by item type, while ctrl and library rendering stay Emby-only.

### Modified Capabilities
<!-- None. queue-only-playback covers panel rendering, not item typing; unchanged. -->

## Impact

- **Code:** `crates/mbv-core/src/playback_queue.rs` (slot type, accessors, persistence), `player_proxy.rs` + `player_run_*` (play-boundary match), `queue_actions.rs`, and their sibling tests. ~15 files, several of them tests.
- **Persistence:** `queue_state.json` on-disk shape (back-compatible read).
- **Ctrl protocol:** unchanged — no capability string, no version bump.
- **Renderers:** unchanged — library/home keep operating on `EmbyItem`.
- **New type:** `FeedEntry` (minimal), consumed by #471 (Feeds tab) and extended by #472 (playback state).

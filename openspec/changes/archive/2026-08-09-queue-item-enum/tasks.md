## 1. Types

- [ ] 1.1 Add minimal `FeedEntry` in `mbv-core` (`guid`, `title`, `enclosure_url: Option<String>`, `link: Option<String>`, `mime_type: Option<String>`, `duration_ticks: Option<u64>`); derive `Debug, Clone, Serialize, Deserialize`. No progress fields.
- [ ] 1.2 Add `QueueItem { Emby(EmbyItem), Feed(FeedEntry) }` with variant-matching accessors the queue needs: `title()`, `duration()`, `media_kind()`, `artwork_url()` (Feed → `None`). Do not add library-render accessors.
- [ ] 1.3 Add a `primary_source()` helper on `FeedEntry` returning `enclosure_url` else `link`.

## 2. Queue wiring

- [ ] 2.1 Change `QueueSlot.item` from `EmbyItem` to `QueueItem` (`playback_queue.rs`); update `QueueSlot::new` and all constructors/call sites to wrap Emby items as `QueueItem::Emby`.
- [ ] 2.2 Guard progress sync on the Emby variant: `ProgressState::from_item` / `apply_to_item` apply only to `QueueItem::Emby`; `Feed` slots keep default `ProgressState` (no-op sync).
- [ ] 2.3 Update `queue_actions.rs` and any queue callers to construct/consume `QueueItem` (Emby paths wrap as before).

## 3. Persistence

- [ ] 3.1 Serialize `queue_state.json` slots with the tagged `QueueItem` shape (always tagged on write).
- [ ] 3.2 Deserialize accepts legacy bare-item JSON as `QueueItem::Emby` (tagged-then-bare fallback).
- [ ] 3.3 Add a round-trip test: a legacy bare-item JSON fixture loads as Emby; save produces tagged; tagged reloads with item kind preserved.

## 4. Play boundary

- [ ] 4.1 In `player_proxy.rs` / `player_run_*`, match the active slot's `QueueItem`: `Emby` → existing `EmbyClient` streaming path unchanged; `Feed` → hand `primary_source()` to mpv directly.
- [ ] 4.2 Keep the player otherwise unchanged; feed source resolved synchronously.

## 5. Ctrl boundary

- [ ] 5.1 Where the local queue is projected onto ctrl messages (`AdoptQueue`/`QueueAppend`/`ReplaceQueue`), `filter_map` slots so only `QueueItem::Emby` items are sent; drop `Feed`. No wire-shape change, no new capability string, no version bump.

## 6. Verify

- [ ] 6.1 `cargo test -p mbv-core` green, including the queue persistence round-trip test.
- [ ] 6.2 `cargo clippy --workspace --all-targets` green.
- [ ] 6.3 `make check-code-file-lines` passes (split any file pushed over 800 lines).
- [ ] 6.4 Existing Emby queue behavior unchanged — existing queue tests pass untouched in intent.

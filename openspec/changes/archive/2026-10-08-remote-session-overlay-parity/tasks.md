# Tasks

## 1. Session payload facts

- [x] 1.1 Add `now_playing_series_name: Option<String>` and `now_playing_logo_etag: Option<String>` to `SessionInfo` (`crates/mbv-emby/src/types.rs`), parse them in `client_sessions.rs` from `NowPlayingItem.SeriesName` and `NowPlayingItem.ImageTags.Logo`, and extend `make_session` (`test_support.rs`) plus any other constructor sites. Contract test (owner: mbv-emby unit tests): a session payload whose `NowPlayingItem` carries `SeriesName` and `ImageTags.Logo` parses both into the new fields, and a payload without them parses `None`/`None` (defensive branch pinned by the spec's "Remote session payload lacks the series name" and "Remote item without a resolvable logo reference" scenarios).

## 2. Slotless title parts

- [x] 2.1 In `slotless_playback_title_parts` (`src/app/shell/playback.rs`), build `PlaybackTitleParts::two(episode_name, series_name)` when the watched Emby session's now-playing item type is `Episode` with a non-empty series name; keep the one-part session-name title for every other case, and keep the Cast branch one-part. Contract test (owner: shell playback unit tests): an Emby session episode with a series name yields two parts; a Cast now-playing name and an Emby session without a series name still yield one part.

## 3. Slotless logo owner

- [x] 3.1 Generalize `overlay_logo_source` (`src/app/state/projection/card.rs`) into a field-based helper taking `(item_type, item_id, series_id, logo_etag)`; the `EmbyItem` arm keeps its current keys (`Movie` → `{id}:Logo:{etag}`, `Episode` → `{series_id}:Logo`) and the slotless path calls it with the session's `now_playing_*` fields.
- [x] 3.2 In `queue_title_site`, resolve the slotless logo owner and issue the same `["Logo"]` fetch with the same cache key the local path uses, passing the key into `ensure_title_overlay_protocol`. Contract tests (owner: `src/app/state/projection/card/title_site_tests.rs`): a slotless session episode with a series id starts the series-logo fetch and composes the overlay variant with the logo key once ready; a slotless session episode without a series id (and a movie without an etag) keeps the text overlay with no logo fetch.

## 4. Verification

- [x] 4.1 `cargo fmt`, `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run` for `mbv-emby` and the TUI crate's projection/shell tests touched above.

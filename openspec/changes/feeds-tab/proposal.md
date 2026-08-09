## Why

With the queue able to hold and play a feed entry (#470), the actual user-facing feature can land: subscribe to RSS/podcast/video feeds, browse their entries, and play them through the same queue. This is the MVP — browse and play, no memory of what was watched.

## What Changes

- Store feed subscriptions in `config.toml` as `[[feeds]]` entries — each `{ name, url, kind: Audio | Video }`. The immutable subscription *list* is local config (portable via the user's dotfiles repo). Mutable playback state (position/played) is explicitly **out of scope** — that is #472.
- Extend the existing RSS/Atom parser (`src/app/feed_parse.rs`) to capture the per-entry fields a `FeedEntry` needs: `guid`, enclosure URL, MIME type, and duration (→ ticks). Today it extracts only `title` + `link`.
- Add a **Feeds tab** — the last tab in the tab bar, visible only when at least one subscription exists. It lists entries grouped by subscription, with an "All" group sorted by publish date descending.
- Play an entry: build a `QueueItem::Feed` (from #470), add it to the queue, play. Enclosure URL preferred, link as fallback.
- Add a **management overlay** to add / remove / edit subscriptions, writing `config.toml`. A failed fetch/parse on add surfaces via the existing status/notify path and the subscription is not saved.

## Capabilities

### New Capabilities
- `feed-subscriptions`: the user can subscribe to feeds in local config, browse their entries in a dedicated tab, and play entries through the queue, statelessly (no resume/played memory).

### Modified Capabilities
<!-- None. Depends on #470's feed-queue-item capability but does not change it. -->

## Impact

- **Depends on #470** (`QueueItem`/`FeedEntry`) — the queue and the type it plays must exist first.
- **Config:** new `[[feeds]]` array-of-tables. Parse in `config_parse.rs`, write in `config_save.rs`, new `feeds` field on `Config` (`config_types_paths.rs`).
- **Parser:** `src/app/feed_parse.rs` extended to emit `guid`/enclosure/MIME/duration.
- **UI:** new tab wired into the tab bar (`chrome_tabs.rs` `all_names` chain + tab-selection routing), a feed-entry renderer (reusing `home_feed.rs` patterns, rendering concrete `FeedEntry`), and a management overlay (`src/app/render/overlays/` pattern).
- **Out of scope:** position/resume/played/watched-filter, redb/mbvd/roaming, remote feed playback / ctrl feed sync — all #472.

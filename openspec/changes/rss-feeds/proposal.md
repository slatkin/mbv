# RSS Feeds

## Problem

mbv can only browse and play content from Emby libraries. Users who want to
follow RSS/Atom feeds — audiobookshelf audiobook feeds, YouTube channels,
podcasts — must use a separate app. There is no way to subscribe to a feed,
browse its entries, or play its media from within mbv.

## Solution

Add a "Feeds" feature: users subscribe to RSS/Atom feed URLs, browse their
entries grouped by feed in a feed-view tab, and play enclosure/link URLs
directly through mpv. Feed subscriptions and entry state (positions, played
status) are persisted in the redb shared store hosted by the daemon, so they
roam across machines.

## Key decisions

- **Queue model**: A `QueueItem` enum wraps `EmbyItem` (renamed from
  `MediaItem`) and `FeedEntry`. Shared accessor methods (title, duration,
  playback URL, position key) give the queue a uniform interface. Queue,
  rendering, and transport code work through these accessors.

- **Position tracking**: The player's existing progress-reporting path is
  reused. For feed entries, reports are written to redb instead of the Emby
  API. Same cadence, same lifecycle (started / progress / stopped).

- **Storage**: Feed definitions and entry state live in dedicated redb tables
  (not the existing `SharedDocumentKind` system). Keyed per-user, per-feed,
  per-entry. The daemon hosts the store but has no feed-fetching logic.

- **Feed parsing**: A third-party crate (`feed-rs` or equivalent) replaces the
  hand-rolled parser. Extracts guid, title, enclosure URL, link, pub_date,
  duration, description. Entry identity uses guid with fallback to enclosure
  URL, then title+date hash.

- **Feed kind**: Audio or video, inferred from enclosure MIME types on first
  fetch. User can override in the management panel.

- **Polling**: All feeds refresh async at app startup and on manual refresh
  keybinding. Cooldown prevents redundant fetches. No daemon-side polling.

- **UI**: A "Feeds" tab appears at the end of the tab bar when feeds are
  defined. Reuses the existing feed-view layout (group per feed, "All" pill,
  per-group episode list). Watched/unwatched is a runtime toggle via
  keybinding. A new sidebar panel for feed subscription management
  (add/remove/edit).

## Non-goals

- Downloading or importing media into Emby libraries (this is a feed reader,
  not a downloader).
- Daemon-side feed polling or caching (clients poll themselves).
- Config-file-based feed definitions (definitions live in redb).
- Mixed audio/video within a single feed (per-feed kind, not per-entry).

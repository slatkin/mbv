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
  media kind, position key, artwork URL) give the queue a uniform interface.
  Playback URL resolution is variant-specific and happens at the play
  boundary, not through an accessor. Queue, rendering, and transport code
  work through these accessors.

- **Wire compatibility**: The `MediaItem` → `EmbyItem` rename is
  wire-invisible (serde field names are unchanged); wrapping items in the
  `QueueItem` enum is a wire/persistence shape change. Legacy bare-MediaItem
  JSON decodes as the Emby variant; serialization always writes the tagged
  shape. Feed variants in queue sync are gated by a capability string and are
  omitted toward peers that do not announce it, so a mixed-version pair keeps
  the rest of the queue intact.

- **Position tracking**: The player's existing progress-reporting path is
  reused. For feed entries, reports are written to redb instead of the Emby
  API. Same cadence, same lifecycle (started / progress / stopped).

- **Storage**: Feed definitions and entry state live in dedicated redb tables
  (not the existing `SharedDocumentKind` system). Keyed per-user, per-feed,
  per-entry. The daemon hosts the store but has no feed-fetching logic.

- **Feed parsing**: The `feed-rs` crate replaces the hand-rolled parser.
  Extracts guid, title, enclosure URL, link, pub_date, duration, description,
  and the feed-level title. Entry identity uses guid with fallback to
  enclosure URL, then title+date hash.

- **Feed kind**: Audio or video, inferred from enclosure MIME types on first
  fetch. User can override in the management panel; per-entry MIME types take
  precedence when present.

- **Polling**: All feeds refresh async at app startup and on manual refresh
  keybinding. A fixed 30-minute cooldown prevents redundant fetches. No
  daemon-side polling.

- **UI**: A "Feeds" tab appears at the end of the tab bar when feeds are
  defined. Uses the existing feed-view layout (group per feed, "All" pill,
  per-group episode list). Watched/unwatched is a runtime filter toggle via
  keybinding. A conditional feed-management overlay (sessions-panel pattern)
  opened from the Feeds tab handles add/remove/edit.

## Non-goals

- Downloading or importing media into Emby libraries (this is a feed reader,
  not a downloader).
- Daemon-side feed polling or caching (clients poll themselves).
- Config-file-based feed definitions (definitions live in redb).
- Mixed audio/video within a single feed (one kind per subscription; entries
  without a MIME type classify by that kind, per-entry MIME types win when
  present).

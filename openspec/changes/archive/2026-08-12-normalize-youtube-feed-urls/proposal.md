Tracking: [#491](https://github.com/slatkin/mbv/issues/491)

## Why

Subscribing to a YouTube channel requires the RSS feed URL
(`feeds/videos.xml?channel_id=UC…`), but the URL a user actually has is the
channel page (`youtube.com/@ChineseCookingDemystified`). Today they must run
that through a third-party converter (vimrss) by hand before pasting it into
the feed form. The subscribe flow should accept the channel URL directly.

## What Changes

- Add-feed resolves a YouTube channel URL to its canonical RSS feed URL before
  fetching, and persists the resolved URL in config.
- `youtube.com/channel/UC…` is rewritten to `feeds/videos.xml?channel_id=UC…`
  with no network call (the id is already in the URL).
- `youtube.com/@handle`, `/c/<name>`, and `/user/<name>` are resolved by
  fetching the channel page and reading the RSS `<link rel="alternate">` href
  it advertises.
- Non-YouTube URLs pass through unchanged; existing RSS/Atom subscriptions are
  unaffected.
- Resolution failure aborts the add (existing add-failure toast); nothing is
  saved. There is no fall-back to storing the unresolved URL.

Out of scope: playlists (an Emby feature, not feeds), auto-filling the
subscription name from the channel title (a separate roadmap item), and any
non-YouTube provider.

## Capabilities

### New Capabilities
- `feed-url-normalization`: resolving a user-provided YouTube channel URL to
  the canonical RSS feed URL during feed subscribe.

### Modified Capabilities

(none — `feed-subscriptions` is still an unarchived delta in the `feeds-tab`
change; this adds a new sibling capability rather than modifying it.)

## Impact

- `src/app/feed_parse.rs`: new `normalize_feed_url` function (reuses the
  existing TLS `ureq` agent).
- `src/app/feeds_manage_actions.rs`: `submit_feed_add`'s background thread
  normalizes before `fetch_and_parse_entries` and returns the resolved URL in
  `FeedAddResult` so config stores it.
- No config schema change, no ctrl-protocol change, no UI change.

# Feed Subscriptions — Spec

## Capability

Users can subscribe to RSS/Atom feeds, browse their entries, play media from
them, and track playback position — all persisted in the shared redb store.

---

## Feed management

- A feed subscription has: id (generated), name, URL, kind (audio | video),
  and a last-fetched timestamp.
- Kind is inferred from enclosure MIME types on first fetch. User can override.
- Subscriptions are stored per-user in a dedicated redb table.
- A new sidebar panel allows adding, editing, and removing feed subscriptions.
  - Add: user enters a URL. mbv fetches the feed, infers name from the feed
    title and kind from enclosure types, and lets the user confirm or change
    both before saving.
  - Edit: change name or kind. URL changes create a new subscription.
  - Remove: deletes the subscription and all associated entry state.

## Feed polling

- All subscribed feeds are fetched async at app startup.
- Manual refresh via keybinding at any time.
- A cooldown (e.g. 30 minutes) prevents redundant automatic fetches. Manual
  refresh ignores the cooldown.
- Polling happens in the client. The daemon hosts redb but does not fetch feeds.

## Feed parsing

- Use a third-party feed-parsing crate (not hand-rolled XML).
- Extract per entry: guid, title, enclosure URL, link, pub_date, duration,
  description, enclosure MIME type.
- Entry identity: guid if present, else enclosure URL, else hash of
  title + pub_date.

## Entry storage

- Cached entries are stored per-user, per-feed, per-entry in a dedicated redb
  table.
- Each entry record holds: parsed fields (above), position_ticks, played flag.
- New entries from a poll are merged: new guids are inserted, existing guids
  are updated (title, URL, etc.) without overwriting position or played state.
- Entries removed from the feed are retained (the user may still want to
  resume them).

## Queue integration

- `QueueItem` is an enum: `Emby(EmbyItem)` | `Feed(FeedEntry)`.
- `MediaItem` is renamed to `EmbyItem`.
- Shared accessors on `QueueItem`: `title()`, `duration()`, `playback_url()`,
  `position_key()`, `artwork_url()`.
- Queue, rendering, and transport code use these accessors — no branching on
  variant except where genuinely variant-specific (Emby session reporting,
  feed position reporting).
- Playback URL resolution: enclosure URL if present, else link URL (handles
  YouTube where mpv + yt-dlp resolves the page URL).

## Position tracking

- The player's existing progress-reporting lifecycle (started, progress,
  stopped) is reused.
- For `FeedEntry` items, reports write to redb instead of the Emby API.
- Same reporting cadence as Emby items.
- On play, the stored position is read from redb and the player seeks to it.

## Feeds tab

- Appears as the last tab in the library tab bar.
- Only visible when at least one feed subscription exists.
- Uses the existing feed-view layout:
  - "All" pill: all entries across feeds, sorted by pub_date descending.
  - Per-feed groups: entries within one feed, sorted by pub_date descending.
- Watched/unwatched filter is a runtime toggle via keybinding (not a config
  setting).

## Dependencies

- Requires the daemon to be running (redb host).
- If the daemon is unreachable, the Feeds tab is hidden or shows an
  unavailable state.

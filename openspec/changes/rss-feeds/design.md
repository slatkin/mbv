# RSS Feeds — Design

## Data model

### QueueItem enum

```
enum QueueItem {
    Emby(EmbyItem),     // renamed from MediaItem
    Feed(FeedEntry),
}
```

Shared accessors on `QueueItem`:

```
fn title(&self) -> &str
fn duration(&self) -> Option<u64>        // ticks
fn playback_url(&self) -> String         // enclosure or link
fn position_key(&self) -> String         // stable identity for position lookup
fn artwork_url(&self) -> Option<String>
```

The queue, player transport, rendering, and ctrl protocol work through these
accessors. Variant-specific code (Emby session reporting, redb position
writes) is behind match arms only at the reporting boundary.

### FeedEntry

```
struct FeedEntry {
    feed_id: String,           // which feed subscription this belongs to
    guid: String,              // stable identity (guid > enclosure URL > hash)
    title: String,
    enclosure_url: Option<String>,
    link: Option<String>,
    pub_date: Option<String>,
    duration_ticks: Option<u64>,
    description: Option<String>,
    mime_type: Option<String>,
}
```

### Feed subscription (redb)

```
struct FeedSubscription {
    id: String,                // generated UUID
    name: String,              // display name (defaulted from feed title)
    url: String,               // RSS/Atom URL
    kind: FeedKind,            // Audio | Video
    last_fetched: Option<u64>, // unix timestamp
}
```

## Redb schema

Two new tables, separate from the existing `shared_documents` table:

### `feed_subscriptions` table

Key: `(user_id: &str, feed_id: &str)`
Value: JSON-serialized `FeedSubscription`

### `feed_entries` table

Key: `(user_id: &str, feed_id: &str, entry_guid: &str)`
Value: JSON-serialized entry record:

```json
{
    "guid": "...",
    "title": "...",
    "enclosure_url": "...",
    "link": "...",
    "pub_date": "...",
    "duration_ticks": null,
    "description": "...",
    "mime_type": "audio/mpeg",
    "position_ticks": 0,
    "played": false
}
```

Position and played state are per-entry, per-user. Updated by the player's
progress-reporting path (for position) and on playback completion (for played).

## Feed parsing

Replace the hand-rolled parser in `feed_parse.rs` with a crate
(e.g. `feed-rs`). The existing idle feed can continue using the current parser
or migrate — either way the new feed system uses the crate.

### Entry identity resolution

```
fn entry_id(entry) -> String:
    if entry.guid is present and non-empty:
        return guid
    if entry.enclosure_url is present:
        return sha256(enclosure_url)
    return sha256(title + pub_date)
```

### Kind inference

On first fetch of a new subscription:
1. Scan enclosure MIME types across entries.
2. If all are `audio/*` → Audio.
3. If all are `video/*` → Video.
4. Mixed or absent → default to Video (YouTube has no enclosures).
5. User can override via the sidebar panel.

## Polling flow

```
App startup
  ├── read all FeedSubscriptions from redb
  ├── for each: spawn async fetch if last_fetched > cooldown
  │     ├── parse feed with crate
  │     ├── merge entries into redb (insert new, update existing, keep state)
  │     └── update last_fetched timestamp
  └── Feeds tab renders from redb cache immediately

Manual refresh (keybinding)
  └── same as above, ignoring cooldown
```

## Player integration

### Playback URL resolution

```
fn playback_url(item: &QueueItem) -> String:
    match item:
        Emby(e) => get streaming URL from Emby API (existing path)
        Feed(f) => f.enclosure_url.unwrap_or(f.link)
```

mpv handles direct URLs natively. For YouTube links, mpv delegates to yt-dlp.

### Progress reporting

The player already calls progress-report hooks at regular intervals and on
stop. The existing path:

```
player reports progress → Emby API (PlayingProgress, PlayingStopped)
```

New path for feed entries:

```
player reports progress → redb (update position_ticks on feed_entries row)
player reports stopped  → redb (update position_ticks, set played if finished)
```

The branch point is inside the progress reporter, keyed on `QueueItem`
variant. The player itself doesn't change.

### Resume

On play of a feed entry:
1. Read `position_ticks` from redb.
2. If > 0, pass `--start=<seconds>` to mpv.

## UI integration

### Feeds tab

Position: last tab in the tab bar, after all Emby libraries.
Visibility: only when at least one `FeedSubscription` exists in redb.

Rendering reuses `FeedHomeVideoState` and the existing feed-view renderer:
- Groups = feed subscriptions (like folder groups today).
- "All" pill = entries from all feeds, sorted by `pub_date` descending.
- Per-group = entries from one feed, sorted by `pub_date` descending.
- Watched/unwatched toggle via keybinding (filters on `played` field).

### Feed management sidebar panel

New sidebar panel (like queue panel, sessions panel):
- Lists subscribed feeds with name and kind indicator.
- Keybindings: `a` add, `e` edit, `d` delete.
- Add flow: enter URL → fetch → show inferred name and kind → confirm.
- Edit: change name or kind.
- Delete: confirm, then remove subscription and all entry state from redb.

## File impact

### mbv-core (crates/mbv-core/)
- `Cargo.toml`: add `feed-rs` dependency.
- New `feed_store.rs`: redb table definitions, CRUD for subscriptions and
  entries.
- New `feed_types.rs`: `FeedSubscription`, `FeedEntry`, `FeedKind`.
- `shared_worker.rs`: expose feed tables alongside existing shared DB.
- Rename `MediaItem` → `EmbyItem` in `api_types.rs` and all call sites.
- New `queue_item.rs`: `QueueItem` enum with accessors.
- Player progress reporting: branch on `QueueItem` variant.

### mbv (src/)
- `feed_parse.rs`: new crate-based parser (or replace contents).
- `types_feed.rs`: `FeedEntry` display types.
- `feed_actions.rs`: polling logic, merge logic, Feeds tab data loading.
- New sidebar panel files for feed management.
- Queue rendering: work through `QueueItem` accessors.
- Library tab bar: append Feeds tab conditionally.

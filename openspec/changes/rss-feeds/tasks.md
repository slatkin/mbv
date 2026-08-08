# RSS Feeds — Tasks

## Phase 1: Data model foundation

- [x] Add `feed-rs` crate to `mbv-core/Cargo.toml`
- [x] Create `feed_types.rs` in mbv-core: `FeedSubscription`, `FeedEntry`,
      `FeedKind` (Audio | Video) structs with serde derives
- [x] Create `feed_store.rs` in mbv-core: two new redb tables
      (`feed_subscriptions`, `feed_entries`), CRUD operations for
      subscriptions, merge logic for entries (insert new, update existing,
      preserve position/played state)
- [x] Wire feed tables into `open_shared_db` initialization alongside
      existing `shared_documents` table
- [x] Tests for feed store: create/read/update/delete subscriptions, entry
      merge (new entries added, existing entries preserve state, identity
      fallback chain)

## Phase 2: QueueItem enum

- [ ] Rename `MediaItem` → `EmbyItem` across the codebase (api_types.rs and
      all call sites)
- [ ] Create `QueueItem` enum in mbv-core with `Emby(EmbyItem)` and
      `Feed(FeedEntry)` variants
- [ ] Implement shared accessors: `title()`, `duration()`, `playback_url()`,
      `position_key()`, `artwork_url()`
- [ ] Migrate queue internals to use `QueueItem` instead of `EmbyItem`
      directly
- [ ] Migrate queue serialization/deserialization (queue state persistence)
- [ ] Migrate ctrl protocol queue messages to carry `QueueItem`
- [ ] Migrate all rendering code that displays queue items to use accessors

## Phase 3: Feed parsing and polling

- [ ] Create crate-based feed parser: takes a URL, returns parsed entries
      with guid, title, enclosure URL, link, pub_date, duration, description,
      MIME type
- [ ] Entry identity resolution: guid → enclosure URL hash → title+date hash
- [ ] Kind inference: scan enclosure MIME types, default to Video if absent
- [ ] Polling orchestrator: on app startup, spawn async fetch for all
      subscriptions past cooldown. Merge results into redb.
- [ ] Manual refresh keybinding: re-fetch all feeds ignoring cooldown
- [ ] Tests for parser integration and entry identity

## Phase 4: Position tracking

- [ ] Branch the player's progress-reporting path on `QueueItem` variant:
      Emby → Emby API (existing), Feed → redb write
- [ ] Write `position_ticks` to `feed_entries` table on progress/stop
- [ ] Set `played = true` on playback completion
- [ ] On play of a `FeedEntry`, read stored position from redb and seek
- [ ] Tests for position write/read round-trip

## Phase 5: Feeds tab

- [ ] Add "Feeds" tab to the library tab bar (last position, visible only
      when subscriptions exist)
- [ ] Load feed data from redb into `FeedHomeVideoState` (groups = feeds,
      items = entries per feed)
- [ ] "All" pill: aggregate all entries sorted by pub_date descending
- [ ] Per-feed groups: entries sorted by pub_date descending
- [ ] Watched/unwatched toggle keybinding (filter on `played` field)
- [ ] Play action on an entry: build `QueueItem::Feed`, add to queue, play

## Phase 6: Feed management sidebar panel

- [ ] New sidebar panel: list subscribed feeds with name and kind
- [ ] Add flow: enter URL → fetch → show inferred name/kind → confirm → save
      to redb
- [ ] Edit flow: change name or kind of existing subscription
- [ ] Delete flow: confirm → remove subscription and all entry state from redb
- [ ] Panel keybindings: `a` add, `e` edit, `d` delete

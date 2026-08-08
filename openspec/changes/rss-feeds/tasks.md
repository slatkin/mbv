# RSS Feeds — Tasks

## Phase 1: Data model foundation

- [ ] Create `feed_types.rs` in mbv-core: `FeedSubscription`, `FeedEntry`,
      `FeedKind` (Audio | Video) structs with serde derives
- [ ] Create `feed_store.rs` in mbv-core: two new redb tables
      (`feed_subscriptions`, `feed_entries`), CRUD operations for
      subscriptions, merge logic for entries (insert new, update existing,
      preserve position/played state)
- [ ] Wire feed tables into `open_shared_db` initialization alongside
      existing `shared_documents` table
- [ ] Shared-service feed commands/events on the existing shared-service
      channel (mirroring `SharedDataCmd` / `SharedStoreRequest`) plus the
      capability string `SHARED_DATA_CAP_FEEDS_V1` (`"shared-mbv-state-feeds-v1"`)
- [ ] Last-write-wins conflict policy for entry-row writes (position, played);
      no CAS for feed rows (unlike `shared_documents`)
- [ ] Tests for feed store: create/read/update/delete subscriptions, entry
      merge (new entries added, existing entries preserve state, identity
      fallback chain)

## Phase 2: QueueItem enum

- [ ] Rename `MediaItem` → `EmbyItem` across the codebase (api_types.rs and
      all call sites)
- [ ] Create `QueueItem` enum in mbv-core with `Emby(EmbyItem)` and
      `Feed(FeedEntry)` variants
- [ ] Implement shared accessors: `title()`, `duration()`, `media_kind()`,
      `position_key()`, `artwork_url()` — no `playback_url()`; URL resolution
      is variant-specific at the play boundary
- [ ] Migrate queue internals to use `QueueItem` instead of `EmbyItem`
      directly
- [ ] Migrate queue serialization/deserialization (queue state persistence):
      backward-compatible — legacy bare-`MediaItem` JSON decodes as the Emby
      variant; serialization always writes the new tagged shape
- [ ] Migrate ctrl protocol queue messages to carry `QueueItem`; gate feed
      variants behind the capability string `CTRL_CAP_QUEUE_FEED_ITEMS`
      (`"queue-feed-items"`) and omit them toward peers that don't announce it
- [ ] Migrate all rendering code that displays queue items to use accessors

## Phase 3: Feed parsing and polling

- [ ] Add `feed-rs` dependency to the root `Cargo.toml` (binary crate, not
      mbv-core)
- [ ] Rewrite `feed_parse.rs` as a thin wrapper over `feed-rs`; delete the
      hand-rolled parser; migrate the existing idle feed to it (feed title +
      entry title/link only)
- [ ] Parser result type includes the feed-level title (default subscription
      name in the add flow) plus per-entry: guid, title, enclosure URL, link,
      pub_date, duration, description, MIME type
- [ ] Entry identity resolution: guid → enclosure URL hash → title+date hash
- [ ] Kind inference: scan enclosure MIME types, default to Video if absent
- [ ] Polling orchestrator (in new `src/app/feeds_actions.rs`): on app startup,
      spawn async fetch for all subscriptions past the 30-minute cooldown.
      Merge results into the store.
- [ ] Manual refresh keybinding: re-fetch all feeds ignoring cooldown (F5,
      existing global refresh binding)
- [ ] Tests for parser integration and entry identity

## Phase 4: Position tracking

- [ ] Branch the player's progress-reporting path on `QueueItem` variant:
      Emby → Emby API (existing), Feed → store write
- [ ] Convert feed durations/positions to Emby ticks at parse time (10^7
      ticks/second)
- [ ] Write `position_ticks` to `feed_entries` table on progress/stop;
      write-through only if the (user, feed_id, guid) row still exists,
      otherwise the write is dropped silently
- [ ] Set `played = true` using the same completion rule as Emby items (EOF
      with known runtime, or final position ≥ 95% of known runtime)
- [ ] On play of a `FeedEntry`, read stored position from the store and seek
      with `--start=<position_ticks / 10^7>`; honor the same resume threshold
      as Emby (6% of known runtime; positive resume for unknown runtime)
- [ ] Tests for position write/read round-trip

## Phase 5: Feeds tab

- [ ] Add "Feeds" tab to the library tab bar (last position, visible only
      when subscriptions exist; hidden when the daemon is unreachable)
- [ ] Generalize `FeedHomeVideoState` and the feed-view renderer
      (`src/app/render/home_feed.rs`) off `MediaItem` to QueueItem-backed
      view data — do not reuse the MediaItem-typed state as-is
- [ ] "All" pill: aggregate all entries sorted by pub_date descending
      (missing/unparseable dates sort last)
- [ ] Per-feed groups: entries sorted by pub_date descending
- [ ] Watched/unwatched filter toggle: `w` (bare, scoped to the Feeds tab;
      filters on the `played` field)
- [ ] Play action on an entry: build `QueueItem::Feed`, add to queue, play

## Phase 6: Feed management overlay

- [ ] New conditional overlay (sessions-panel pattern, not a persistent
      panel): lists subscribed feeds with name and kind; opens from the Feeds
      tab via `s` (`m` is taken by the global mute binding)
- [ ] Add flow: enter URL → fetch → show inferred name/kind → confirm → save
      to the store. A failed fetch/parse means the subscription cannot be
      saved; surface the error via the existing status/notify mechanism
- [ ] Edit flow: change name or kind of existing subscription; URL changes
      create a new subscription
- [ ] Delete flow: confirm → remove subscription and all entry state from the
      store (cascade); already-queued snapshots stay playable
- [ ] Overlay keybindings: `a` add, `e` edit, `d` delete (safe — the overlay
      captures input); manual refresh via existing F5

## Close out

- [ ] Keep CONTEXT.md vocabulary accurate: FeedEntry, FeedSubscription,
      FeedKind, QueueItem, EmbyItem (renamed from MediaItem)

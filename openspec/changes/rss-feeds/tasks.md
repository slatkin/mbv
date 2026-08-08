## 1. Shared Store Extensions

- [ ] 1.1 Add `FeedSubscriptions` and `FeedEntryState` variants to `SharedDocumentKind`
- [ ] 1.2 Extend `shared_protocol.rs` snapshot/notification handling for new document kinds
- [ ] 1.3 Extend `shared_store.rs` read/write helpers for new document kinds
- [ ] 1.4 Extend `shared_client.rs` to track revisions and handle notifications for feed documents
- [ ] 1.5 Define `FeedSubscription` struct (url, title_override, kind, created_at)
- [ ] 1.6 Define `FeedEntryState` struct (position_ticks, watched, last_played)
- [ ] 1.7 Define `FeedEntryStateMap` as HashMap keyed by (feed_url, entry_key)

## 2. Feed Parsing

- [ ] 2.1 Add `feed-rs` crate dependency
- [ ] 2.2 Create `feed_parse.rs` module with async fetch-and-parse function
- [ ] 2.3 Implement entry key derivation (guid > enclosure_url > title+date hash)
- [ ] 2.4 Implement feed kind inference from enclosure MIME types
- [ ] 2.5 Define `ParsedFeed` struct (title, entries) and `ParsedEntry` struct (key, title, enclosure_url, duration, pub_date)

## 3. QueueItem Abstraction

- [ ] 3.1 Define `FeedEntry` struct for queue items (entry_key, title, enclosure_url, duration, feed_url, feed_kind)
- [ ] 3.2 Define `QueueItem` enum wrapping `MediaItem` and `FeedEntry`
- [ ] 3.3 Implement accessor trait/methods on `QueueItem` (title, duration, playback_url, position_key, is_audio)
- [ ] 3.4 Migrate `PlaybackQueue` from `MediaItem` to `QueueItem`
- [ ] 3.5 Update queue serialization to tag variants (backward-compatible JSON)
- [ ] 3.6 Update queue deserialization to skip unknown variants gracefully

## 4. Ctrl Protocol Capability

- [ ] 4.1 Add `queue-feed-items-v1` capability constant
- [ ] 4.2 Advertise capability in daemon hello
- [ ] 4.3 Advertise capability in client hello
- [ ] 4.4 Filter feed items from queue payloads when peer lacks capability

## 5. Progress Routing

- [ ] 5.1 In player progress path, check queue item variant before reporting
- [ ] 5.2 Route `FeedEntry` progress to shared store `FeedEntryState` document
- [ ] 5.3 On playback complete, set watched=true in `FeedEntryState`
- [ ] 5.4 Handle shared-store write failures gracefully (log, continue playback)

## 6. Feed Subscription Management

- [ ] 6.1 Create sidebar panel component for feed management
- [ ] 6.2 Implement add-feed flow (URL input, fetch, infer kind, store subscription)
- [ ] 6.3 Implement remove-feed flow (delete subscription and entry state)
- [ ] 6.4 Implement edit-feed flow (title override, kind override)
- [ ] 6.5 Add keybinding to open feed management panel

## 7. Feeds Tab UI

- [ ] 7.1 Add Feeds tab to tab bar (after libraries, conditional on subscriptions + shared store)
- [ ] 7.2 Implement pillbar with "All" + per-feed pills
- [ ] 7.3 Implement entry list using feed-view layout structure
- [ ] 7.4 Render entry metadata (title, duration, pub_date) from parsed feed
- [ ] 7.5 Render watched/unwatched indicator from `FeedEntryState`
- [ ] 7.6 Implement watched toggle keybinding
- [ ] 7.7 Hide Feeds tab when shared store disconnects

## 8. Feed Refresh

- [ ] 8.1 Implement async refresh of all feeds on app launch (after shared-store connect)
- [ ] 8.2 Implement manual refresh keybinding
- [ ] 8.3 Implement refresh cooldown to prevent redundant fetches
- [ ] 8.4 Update UI as entries arrive during background refresh

## 9. Feed Entry Playback

- [ ] 9.1 Wire play/enqueue/play-next actions from Feeds tab to queue
- [ ] 9.2 Pass enclosure URL directly to mpv (no Emby stream resolution)
- [ ] 9.3 Show error for entries without enclosure URL
- [ ] 9.4 Apply audio-only owner fall-through for video-kind feed entries
- [ ] 9.5 Restore feed entry position from `FeedEntryState` on play

## 10. Integration Testing

- [ ] 10.1 Test shared-store round-trip for FeedSubscriptions document
- [ ] 10.2 Test shared-store round-trip for FeedEntryState document
- [ ] 10.3 Test queue serialization with mixed Emby and feed items
- [ ] 10.4 Test old-client compatibility (feed items filtered from queue)
- [ ] 10.5 Test feed kind inference from MIME types
- [ ] 10.6 Test entry key stability across re-fetches

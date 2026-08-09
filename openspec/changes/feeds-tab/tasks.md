## 1. Config

- [ ] 1.1 Add `FeedSubscription { name, url, kind: FeedKind }` and `FeedKind { Audio, Video }` in a small new module (not by growing `config_types_paths.rs`, already 677 lines); add `feeds: Vec<FeedSubscription>` to `Config`.
- [ ] 1.2 Parse a `[[feeds]]` array-of-tables in `config_parse.rs` (mirror existing `get_str`/array patterns); tolerate missing/partial entries.
- [ ] 1.3 Write `[[feeds]]` in `config_save.rs` via the existing read-modify-write toml merge (preserve the rest of the file).
- [ ] 1.4 Round-trip test: config with `[[feeds]]` parses, saving preserves them and unrelated keys.

## 2. Parser

- [ ] 2.1 Add a sibling `fetch_and_parse_entries` in `feed_parse.rs` yielding `FeedEntry` (from #470): extract `guid`, enclosure URL, MIME type, and duration → ticks; reuse `extract_tag` / `extract_atom_link`. Leave the idle-feed `fetch_and_parse_rss` path unchanged.
- [ ] 2.2 Add a MIME → `FeedKind` inference helper (default Video when absent); tolerate missing enclosure (fall back to link) and unparseable duration (`None`).
- [ ] 2.3 Parser tests: entry with enclosure+guid+duration; entry with only a link; malformed duration yields `None` without failing the feed.

## 3. Feeds tab wiring

- [ ] 3.1 Append the Feeds tab after the library names in `chrome_tabs.rs` `all_names`, gated on `!config.feeds.is_empty()`.
- [ ] 3.2 Route tab selection: branch the "feeds tab index" case before the library-index lookup so it is never treated as an Emby library; audit each `library_tab` → library mapping site.
- [ ] 3.3 Guard test: selecting the feeds tab index shows feed entries and does not route into library code (no Emby library fetch/display for that tab).
- [ ] 3.4 Feed-tab state/actions in their own new files (not by growing `feed_actions.rs`, already 679 lines): hold fetched `FeedEntry` lists per subscription. No auto-fetch — start empty ("press r to load").
- [ ] 3.5 Bind `r` on the active Feeds tab to re-fetch all subscriptions and update the entry lists; no fetch-on-open, no timer.

## 4. Feeds tab rendering

- [ ] 4.1 Render `FeedEntry` directly using the `home_feed.rs` pill-bar + list layout; groups = subscriptions plus an "All" group.
- [ ] 4.2 "All" group sorts entries by publish date descending, missing dates last.
- [ ] 4.3 Do not adapt Emby-shaped state or generalize the renderer off `EmbyItem`.

## 5. Play

- [ ] 5.1 "Play" on an entry builds `QueueItem::Feed(FeedEntry)`, appends it to the bound playback queue, and starts it through the feed-capable player action (mechanics come from #470). Validate a playable enclosure/link before dispatching; Feed playback never reports to Emby.
- [ ] 5.2 Wire Enter on the selected Feed entry to that action with the appropriate audio-only/headless decision and current UI volume. It must not fall through to Emby library queue actions.

## 6. Management overlay

- [ ] 6.1 Add/remove/edit overlay modeled on `src/app/render/overlays/`; expose it through a `Manage feeds` row in F2 Settings so the first subscription can be added while the Feeds tab is hidden.
- [ ] 6.2 Add fetches+parses the feed first; on failure surface via the existing status/notify path and do not save. Ignore stale or cancelled asynchronous add results.
- [ ] 6.3 On success append to `config.feeds` and persist via `config_save.rs`; remove/edit rewrite the same list. Edit changes only name and kind; URL changes require removal and a new subscription.
- [ ] 6.4 After every mutation, resync subscriptions, clear fetched entry data, clamp selected group/cursor/scroll, and do not auto-fetch. If the last subscription is removed while Feeds is selected, fall back to Home.

## 7. Verify

- [ ] 7.1 Add a real feed via the overlay → it appears in `config.toml` and the Feeds tab.
- [ ] 7.2 Play an entry → mpv plays the enclosure URL through the queue.
- [ ] 7.3 Restart → subscription persists (from config); no FeedEntry playback position or watched state is remembered.
- [ ] 7.4 `cargo test -p mbv-core` green; `cargo clippy --workspace --all-targets` green; `make check-code-file-lines` passes.

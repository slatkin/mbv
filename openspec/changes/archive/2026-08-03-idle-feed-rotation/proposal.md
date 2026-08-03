## Why

When nothing is playing, the playback panel sits empty -- a blank area that wastes valuable screen real estate. Replacing this dead space with an idle content feed (RSS headlines) gives the user something to glance at while keeping the area responsive to playback state.

## What Changes

- Add an `[idle_feed]` section to `config.toml` with `rss_url` (default: `https://novaramedia.com/feed/`) and `rotation_interval_secs` (default: 10).
- Parse the config fields in `mbv_core::config_parse` and store them in the `Config` struct.
- Add a new `IdleFeed` struct to `src/app/types_feed.rs` holding fetched feed items and rotation state (current index, last-rotation timer).
- Fetch the RSS feed asynchronously on startup and periodically refresh it.
- Render feed item titles in the playback panel area **only** when `PlaybackState.active` is `false` (nothing playing / playback is idle).
- Rotate the displayed item every `rotation_interval_secs` seconds, starting from the latest item.
- Make displayed titles clickable: if the terminal supports OSC 8 hyperlinks, wrap the title; otherwise render as plain text.

## Capabilities

### New Capabilities

- `idle-feed-rotation`: Display rotating RSS feed headlines in the playback panel when nothing is playing. Configurable feed URL and rotation interval.

### Modified Capabilities

<!-- No existing specs to modify. -->

## Impact

- **`crates/mbv-core/src/config_types_paths.rs`**: Add `idle_feed_rss_url` and `idle_feed_rotation_secs` fields to `Config`.
- **`crates/mbv-core/src/config_parse.rs`**: Parse `[idle_feed]` TOML section.
- **`crates/mbv-core/src/config_save.rs`**: Save `[idle_feed]` section on settings write-back.
- **`dist/config.toml`**: Document the new section.
- **`src/app/types_feed.rs`**: Add `IdleFeed` struct for feed items and rotation state.
- **`src/app/app_struct.rs`**: Add `idle_feed` field to `App`.
- **`src/app/render/`**: Add or modify playback-panel renderer to show feed titles when idle.
- **`src/app/layout.rs`**: May need new hit-target rects for clickable feed titles.
- **`src/app/mod.rs`**: Periodic feed refresh and rotation tick in the event loop.

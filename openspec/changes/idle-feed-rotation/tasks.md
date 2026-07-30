## 1. Config plumbing

- [x] 1.1 Add `idle_feed_rss_url` (String) and `idle_feed_rotation_secs` (u64) fields to `Config` struct in `crates/mbv-core/src/config_types_paths.rs`, with defaults in `Default::default()`.
- [x] 1.2 Parse `[idle_feed]` section in `crates/mbv-core/src/config_parse.rs`: extract `rss_url` (default `https://novaramedia.com/feed/`) and `rotation_interval_secs` (default 10, clamp to >= 1).
- [x] 1.3 Save `[idle_feed]` section in `crates/mbv-core/src/config_save.rs` for settings write-back.
- [x] 1.4 Add `[idle_feed]` section with commented defaults to `dist/config.toml`.

## 2. Feed state and types

- [x] 2.1 Add `IdleFeedItem` struct (title: String, link: Option<String>) and `IdleFeed` struct (items: Vec<IdleFeedItem>, current_index: usize, last_rotation: Instant, last_fetch: Instant, items_tx: Sender, items_rx: Receiver) to `src/app/types_feed.rs`.
- [x] 2.2 Add `idle_feed: Option<IdleFeed>` field to `App` struct in `src/app/app_struct.rs`.
- [x] 2.3 Wire `idle_feed` into `AppInit` and `App::new`/`App::new_remote` constructors (set to `None` initially; `run()` spawns the initial fetch).

## 3. Feed fetching

- [x] 3.1 Implement `spawn_idle_feed_fetch()` in `src/app/feed_actions.rs`: spawn a thread that fetches the RSS URL with ureq, parses `<item>`/`<entry>` titles and links, sends `Vec<IdleFeedItem>` through the items channel.
- [x] 3.2 Implement RSS/Atom XML parsing inline: extract `<item>` blocks (or `<entry>` blocks for Atom), extract the first `<title>` and `<link>` child from each. Order items newest-first.
- [x] 3.3 In the run loop (`src/app/mod.rs` `run()` method): spawn initial feed fetch on startup, then re-spawn every 30 minutes. Drain `idle_feed.items_rx` alongside other channels.
- [x] 3.4 When items arrive via the channel, update `idle_feed.items` and reset `current_index` to 0.

## 4. Rotation and display

- [x] 4.1 Add `advance_idle_feed_rotation()` method: if enough items exist and `rotation_interval_secs` has elapsed, increment `current_index` (wrapping around).
- [x] 4.2 Modify `render_player_panel()` in `src/app/render/chrome_player.rs`: when `show_controls` is false and `idle_feed` has items, render the current item's title (with optional OSC 8 link) in the title row instead of the blank bar.
- [x] 4.3 Implement OSC 8 hyperlink wrapping: add a helper function `osc8_link(url: &str, text: &str) -> String` that produces the escape sequence; gate on terminal support detection.
- [x] 4.4 Call `advance_idle_feed_rotation()` in the render path (or event loop tick) so rotation updates trigger re-renders.

## 5. Terminal support detection

- [x] 5.1 Add `osc8_supported: bool` field to `App` struct.
- [x] 5.2 Detect OSC 8 support at startup: check `$TERM` env var and/or known terminal list (kitty, foot, WezTerm, iTerm2, Windows Terminal, Alacritty, tmux). Set `osc8_supported` accordingly.
- [x] 5.3 Use `osc8_supported` flag to gate OSC 8 escape sequence wrapping in the render path.

## 6. Documentation

- [x] 6.1 Update `dist/config.toml` with the new `[idle_feed]` section, commented with defaults and explanation.

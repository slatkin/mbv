## Context

When nothing is playing and no remote session is connected, the playback panel (the area below the tab bar on the right side) renders empty -- blank bars with no content. This is wasted space that could display useful information to the user.

The proposal specifies showing rotating RSS feed headlines in this idle space, with a configurable feed URL and rotation interval.

The existing `ureq` crate (already a workspace dependency) can handle HTTP fetches. RSS/Atom XML parsing is straightforward enough to implement inline without a new dependency, avoiding adding a feed-parsing crate to the dependency tree.

## Goals / Non-Goals

**Goals:**
- Fetch and parse a configurable RSS feed URL at startup and periodically refresh
- Display feed item titles in the playback panel title row when playback is idle
- Rotate displayed items every N seconds (configurable), starting from the newest
- Make titles clickable via OSC 8 hyperlinks when the terminal supports them
- Keep the feature fully configurable via `config.toml`

**Non-Goals:**
- Full RSS reader with article content display
- Feed categories, filtering, or search
- Multiple simultaneous feeds
- OPML import
- Offline caching of feed contents
- Image/media enclosure display from feeds

## Decisions

### 1. RSS parsing: inline XML parsing vs. new dependency

**Decision:** Parse RSS/Atom XML inline using a simple regex-based approach on the raw XML text, extracting `<item>`/`<entry>` blocks and their `<title>` and `<link>` children.

**Rationale:** RSS/Atom feeds are structurally simple for headline extraction. Adding a full XML parsing crate (e.g., `quick-xml`, `feed-rs`) would pull in transitive dependencies for a narrow use case. The inline approach handles 95%+ of real-world feeds (including the default Novara Media feed) with zero new dependencies.

**Alternatives considered:**
- `quick-xml` + `atom_syndication`/`rss` crates: Full spec compliance, but adds ~5-10 transitive dependencies.
- `ureq`'s built-in XML support: None exists; ureq is HTTP-only.

### 2. Fetch strategy: synchronous blocking in event loop vs. async background thread

**Decision:** Fetch the RSS feed on a background thread using `std::thread::spawn`, communicating results back via a new `mpsc::channel`. The main run loop drains this channel alongside existing channels (`card_image_rx`, `player_rx`, etc.).

**Rationale:** The existing codebase already uses `std::thread::spawn` + `mpsc` extensively (image fetches, session polls, search queries). Sticking with this pattern avoids introducing async runtime complexity into the synchronous TUI event loop. `ureq`'s blocking HTTP client is already in the dependency tree and works well with `std::thread::spawn`.

**Alternatives considered:**
- Tokio async: Overkill -- the app already has tokio for zbus/MPRIS only, and mixing tokio tasks with the synchronous event loop adds coordination complexity.
- Fetch in the main thread: Would block the UI on every fetch/refresh, unacceptable.

### 3. Where to add the feed state: existing `types_feed.rs` vs. new file

**Decision:** Add a new `IdleFeed` struct to the existing `src/app/types_feed.rs` file.

**Rationale:** `types_feed.rs` already holds feed-related types (`FeedHomeVideoGroup`, `FeedHomeVideoState`, `SavePlaylistDialog`). The name is broad enough ("feed types") to encompass RSS feed state too. Adding a new file would be over-engineering for a single struct.

### 4. Rendering: modify `chrome_player.rs` vs. new render module

**Decision:** Modify the existing `chrome_player.rs` `render_player_panel` method to check for idle state and render feed content instead of blank bars in the title row.

**Rationale:** The playback panel rendering is already centralized in `chrome_player.rs`. The idle feed is a direct replacement for the "no content" case in the same spatial area. Adding a new render module would require wiring it into `render_main` in `mod.rs`, duplicating the player panel layout logic.

Specifically, when `show_controls` is false and an idle feed item is available, the title row area (currently rendering a blank colored bar) will instead render the feed item title. When playback is active (`show_controls` is true), the existing behavior is unchanged.

### 5. OSC 8 hyperlinks: implementation approach

**Decision:** Render feed item titles as OSC 8 hyperlinks when supported, wrapping the title text with `\x1b]8;;<url>\x1b\\<title>\x1b]8;;\x1b\\` escape sequences. Detect support once at startup by checking `$TERM` and terminal query responses.

**Rationale:** Ratatui does not natively support OSC 8 hyperlinks in `Span`/`Paragraph`. However, hyperlinks can be injected as raw escape sequences embedded in the text content string, which ratatui passes through to the terminal unchanged.

**Terminal detection:** Check the `$TERM` environment variable for known OSC 8-supporting terminals. The escape sequences are no-ops on unsupported terminals (the text renders normally without the link), so a false positive is harmless -- the link just won't be clickable.

### 6. Rotation timing: how to implement the 10-second interval

**Decision:** Store an `Instant` of the last rotation in the `IdleFeed` struct. On each render tick, check if `elapsed() >= rotation_interval`. If so, advance the index and update the last-rotation timestamp.

**Rationale:** The render loop already has timing infrastructure (`last_render`, `render_interval`). A simple `Instant` comparison is zero-cost and doesn't require spawning a timer thread.

## Risks / Trade-offs

- **[Risk] RSS feed XML parsing with regex may break on edge-case feeds** → Mitigation: Parse only `<title>` and `<link>` from well-formed `<item>` blocks. If parsing yields zero items, silently degrade (show nothing, log a warning). The default Novara Media feed is known-good.

- **[Risk] Feed fetch failure (network down, bad URL) causes silent empty panel** → Mitigation: Log the error. Don't flash the status bar. The feature is purely cosmetic -- the prior behavior (empty panel) is the fallback.

- **[Risk] OSC 8 hyperlink escape sequences may corrupt terminal output on very old terminals** → Mitigation: Gate behind a terminal capability check. If unsupported, render plain text only.

- **[Trade-off] Feed refresh frequency** → Initial implementation refreshes every 30 minutes to avoid hammering the feed server while keeping content reasonably fresh. The rotation interval (10s default) is separate -- it cycles through already-fetched items without re-fetching.

## Open Questions

- None. All decisions are resolved above.

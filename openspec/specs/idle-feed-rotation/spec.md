# idle-feed-rotation Specification

## Purpose
TBD - created by archiving change idle-feed-rotation. Update Purpose after archive.

## Requirements

### Requirement: Configurable idle-feed RSS URL and rotation interval
The system SHALL accept an `[idle_feed]` section in `config.toml` with fields `rss_url` (string, default `https://novaramedia.com/feed/`) and `rotation_interval_secs` (integer, default 10, minimum 1).

#### Scenario: Default configuration when section is absent
- **WHEN** `config.toml` has no `[idle_feed]` section
- **THEN** the system SHALL use `https://novaramedia.com/feed/` as the RSS URL and 10 seconds as the rotation interval.

#### Scenario: Custom configuration
- **WHEN** `config.toml` has `[idle_feed]` with `rss_url = "https://example.com/rss"` and `rotation_interval_secs = 30`
- **THEN** the system SHALL fetch from `https://example.com/rss` and rotate items every 30 seconds.

#### Scenario: Invalid rotation interval clamps to minimum
- **WHEN** `config.toml` has `rotation_interval_secs = 0`
- **THEN** the system SHALL clamp the rotation interval to 1 second.

### Requirement: Feed fetching and parsing
The system SHALL fetch the configured RSS feed URL on startup and periodically (every 30 minutes), parse it to extract item titles and links, and store them for display.

#### Scenario: Successful fetch and parse
- **WHEN** the RSS feed is fetched successfully and contains valid `<item>` entries with `<title>` and `<link>` children
- **THEN** the system SHALL store the parsed items (title and link pairs), ordered from newest to oldest.

#### Scenario: Fetch failure is silent
- **WHEN** the RSS feed fetch fails (network error, HTTP error, invalid URL)
- **THEN** the system SHALL log the error and keep any previously fetched items (or an empty list). The playback panel SHALL render as if no feed is configured.

#### Scenario: Empty or malformed feed
- **WHEN** the RSS feed is fetched but contains zero parseable items
- **THEN** the system SHALL log a warning and show no feed content in the playback panel.

### Requirement: Idle feed display in playback panel
The system SHALL display the current feed item's title in the playback panel title row ONLY when playback is idle (nothing playing and no remote session connected), replacing the otherwise blank title area.

#### Scenario: Idle state shows feed title
- **WHEN** nothing is playing and no remote session is connected, and at least one feed item has been fetched
- **THEN** the playback panel title row SHALL display the title of the current feed item.

#### Scenario: Active playback hides feed
- **WHEN** playback becomes active (local or remote)
- **THEN** the feed title SHALL be hidden and the normal now-playing title SHALL be displayed instead.

#### Scenario: No feed items yet shows blank space
- **WHEN** nothing is playing but no feed items have been fetched yet (startup, or fetch pending)
- **THEN** the playback panel SHALL render the existing blank bar, unchanged from current behavior.

### Requirement: Feed item rotation
The system SHALL rotate the displayed feed item every `rotation_interval_secs` seconds, cycling from the newest item (index 0) through to the oldest, then wrapping back to the newest.

#### Scenario: Normal rotation
- **WHEN** the idle feed is displaying and `rotation_interval_secs` has elapsed since the last rotation
- **THEN** the system SHALL advance to the next feed item (index = (current_index + 1) % item_count).

#### Scenario: Wrap-around at end
- **WHEN** the current feed item is the last item in the list
- **THEN** the next rotation SHALL wrap to the first (newest) item.

#### Scenario: Rotation pauses during active playback
- **WHEN** playback becomes active while a feed item is displayed
- **THEN** rotation SHALL pause. When playback becomes idle again, rotation SHALL resume from the item that was displayed before playback started.

### Requirement: Clickable feed titles via OSC 8 hyperlinks
The system SHALL render feed item titles as clickable links using OSC 8 escape sequences when the terminal supports hyperlinks, allowing the user to click the title to open the link in their default browser.

#### Scenario: OSC 8 rendering on supported terminal
- **WHEN** the terminal supports OSC 8 hyperlinks (e.g., kitty, foot, iTerm2, WezTerm, Windows Terminal)
- **THEN** the feed item title SHALL be wrapped in OSC 8 escape sequences (`\x1b]8;;<url>\x1b\\<title>\x1b]8;;\x1b\\`) so that clicking it opens the URL.

#### Scenario: Plain text on unsupported terminal
- **WHEN** the terminal does not support OSC 8 hyperlinks
- **THEN** the feed item title SHALL be rendered as plain text without escape sequences.

#### Scenario: Item has no link
- **WHEN** a feed item has a title but no link element
- **THEN** the title SHALL be rendered as plain, non-clickable text.

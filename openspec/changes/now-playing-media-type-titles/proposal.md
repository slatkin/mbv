## Why

The playback panels' title row shows the queue item's raw title for everything except Emby
episodes, so a podcast episode, a song and a movie all read the same way: one undifferentiated
name. The show, artist, podcast or feed the item came from is never shown, even though the queue
item already carries it. One media type (Emby episodes) already splits into two parts, but that
split is Emby-only, untested, and reached by re-matching the item against a formatted string.

## What Changes

- The playback panel title row SHALL name what is playing, per media type:

  | Media | Title part | Context part |
  |---|---|---|
  | Emby movie | item name | — |
  | Emby episode | episode name | series name |
  | Emby music track | track name | artist |
  | Emby home video | item name | — |
  | Audiobookshelf podcast episode | episode title | show (podcast) title |
  | Feed entry | entry title | subscription name |

  Items with no context part render the title alone. A context part that the item does not carry
  (an Audiobookshelf episode queued outside its show) degrades to the title alone.

- The two parts SHALL be delineated by colour, not by a separator glyph: the title part in a new
  **aqua title role** and the context part in a new **yellow context role**, separated by one
  space. The existing `" - "` forms in `QueueItem::display_name()` and `EmbyItem::playback_label()`
  stay as they are for the colourless surfaces (toasts, MPRIS, log lines); the panel does not use
  them.

- The two roles are new named semantic roles rather than the existing `ACCENT` /
  `TEXT_FOCUS_ACCENT` roles, so a future edit to the focus accent cannot move the now-playing
  title.

- One shared, provider-neutral title-part mapping SHALL replace the panel's Emby-only special
  case, so the panel and any future consumer read one rule. The mapping SHALL be built from the
  queue item itself, not by re-finding the item through `item.display_name() == title`.

- **Non-goals**, deliberately not in this change:
  - **ABS audiobook file titles.** Audiobook playback is currently broken, so a
    `book title - file title` label cannot be verified. Audiobooks keep the title-alone fallback.
  - **Media lists.** Home, library and queue rows keep their current content and colours. They
    adopt the shared mapping later; this change only makes that adoption a wiring change rather
    than a second implementation.
  - **Plain-text titles.** `playback_label()` and `display_name()` are unchanged.

## Capabilities

### New Capabilities
<!-- None: this change modifies existing behavior only. -->

### Modified Capabilities

- `queue-playback-panel`: the panel's title row content was previously unspecified; it gains a
  requirement stating what the title row names per media type and which two colour roles carry
  the title and context parts.

## Impact

- **Code (`crates/mbv-core`)**: queue-item title presentation gains the shared mapping
  (`QueueItem` / `QueueItem::display_name_parts`), provider-neutral and colour-free. The Emby-only
  rule currently in the App layer moves here and is widened to music, podcast episodes and feed
  entries. The feed's subscription name is resolved from `FeedEntry.feed_id` against the feed
  subscriptions; the Audiobookshelf episode's show title is the existing optional `show_title`.

- **Code (`src/app`)**: `shell_playback.rs` builds the panel's title parts from the queue item
  instead of a re-matched title string and no longer needs the by-value clones that work around
  the current builder's `&mut self` receiver;
  `components/{library,queue}_playback_panel.rs`,
  `render/components/{chrome_player,chrome_player_context}.rs`,
  `render/theme/{mod,primitives}.rs` and `palette.rs` carry and resolve the two new roles.

- **Tests**: the panel's existing two-part episode title has no coverage today, so this change
  adds the first tests for that path (both panels, buffer-level) in addition to colour-role
  assertions per media type. The media-list painters' existing two-tone tests are unaffected and
  serve as the regression guard that list rows did not move.

- **No spec delta** for `ui-design-system` (its "no existing role fits a call site" requirement
  already permits adding a named role) or for `canonical-media-lists` (list behavior is
  unchanged).

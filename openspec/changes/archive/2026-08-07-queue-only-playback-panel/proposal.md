## Why

Queue-only mode hides the right column entirely, which means the playback panel (seekbar, title, controls) disappears. The user loses playback context when they most want to focus on the queue. Adding it back — beneath the hero image in narrow terminals, beside it in wide ones — keeps playback visible without re-introducing the library column.

## What Changes

- In queue-only mode, render the full playback panel (seekbar + title + controls) with `DARK_BG` background.
- **Narrow layout** (terminal width < 100): playback panel appears as its own row beneath the hero image, above the queue list. Full width of the left column, `DARK_BG` background.
- **Wide layout** (terminal width >= 100): hero image and playback panel sit side-by-side. Image left-aligned in the left column, playback panel in the right column, 2-cell gap between them. Playback panel height matches the image height (content top-aligned, `DARK_BG` fills remaining space).
- `render_card_image` gains a left-alignment option so the image hugs the left edge instead of centering within its area.

## Capabilities

### New Capabilities
- `queue-only-playback`: Rendering rules for the playback panel in queue-only mode, including narrow/wide layout threshold and the two-column arrangement.

### Modified Capabilities
- `panel-mode`: Queue-only state currently specifies "the right column (tab bar, player, library list, status bar) SHALL NOT be rendered." The playback panel (player) now renders in the left column instead. The tab bar, library list, and status bar remain hidden.

## Impact

- `src/app/render/mod.rs` — `render_main`: queue-only branch gains playback panel placement logic (narrow stacked vs wide side-by-side).
- `src/app/render/card.rs` — `render_card_image`: add alignment parameter, return actual image width alongside height.
- `src/app/render/chrome_player.rs` — `render_player_panel`: no signature change needed; called with `DARK_BG` and a taller area in wide mode.
- `src/app/palette.rs` — `DARK_BG` already exists, no changes.

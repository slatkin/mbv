## Why

Below 80 columns, the two-column Power View layout no longer fits: a
side-by-side queue and library render squeezed and mostly unusable. The
existing `x` panel-mode cycle (both → library-only → queue-only → both)
doesn't know about terminal width, so a `both`-mode narrow terminal has no
good state to land in. Separately, queue-only mode deliberately renders the
queue panel with unfocused styling (no cursor highlight, no scrollbar) even
while it holds input focus — at narrow widths, where the queue panel *is*
the whole screen, this reads as fully broken: no visible selection, no way
to tell the panel is interactive.

## What Changes

- Below 80 columns, `x` switches between exactly two states — library panel
  full-width (default) or queue panel full-width — instead of the existing
  three-state both/library-only/queue-only cycle. This "mini view" is
  ephemeral: it tracks its own last-shown panel, separate from the normal
  `panel_mode`/`panel_focus` prefs, so leaving and returning to a wide
  terminal restores whatever wide-mode state was active before narrowing,
  untouched.
- **BREAKING** (behavior, not API): the queue panel's forced-unfocused
  styling in queue-only mode is removed. Queue-only — at any width, not
  just mini view — now renders with normal focused styling (cursor
  highlight, scrollbar) whenever the queue actually holds panel focus,
  matching how it looks in `both` mode. This resolves a real bug: at
  narrow widths, the missing focused styling made the queue panel look and
  feel unresponsive to input even though key routing was correct.
- At 80+ columns, the existing three-state cycle and its focus-follow
  behavior are unchanged.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `panel-mode`: adds the narrow-terminal (<80 cols) two-state mini-view
  cycle on `x`, and removes the queue-only forced-unfocused-styling
  requirement in favor of always rendering focused styling when the queue
  holds focus, regardless of width or panel mode.

## Impact

- `src/app/action.rs` (`Command::CyclePanelMode` handler): branch on
  terminal width to drive either the existing 3-state cycle or the new
  2-state mini-view toggle.
- `src/app/app_struct.rs` / `src/app/construct.rs`: new ephemeral
  `mini_view_focus` field (not persisted, defaults to library).
- `src/app/render/mod.rs`: layout and the `queue_focused` styling
  computation read an effective mode/focus derived from terminal width
  instead of the raw `panel_mode`/`panel_focus` fields directly; drop the
  `panel_mode != QueueOnly` guard on focused styling.
- `src/app/input.rs`: key routing (`handle_key_view_dispatch`) reads the
  same effective focus.
- `src/app/mod.rs`: new `MINI_VIEW_THRESHOLD: u16 = 80` constant, distinct
  from the unrelated `TWO_COLUMN_THRESHOLD` (82, library panel's internal
  list-column layout).
- Tests in `input_resolver_handle_key_tests.rs`, `render/tests_queue.rs`,
  and new narrow-width coverage.

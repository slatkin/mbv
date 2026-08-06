## Why

The Power View shows two panels side by side: the left column holds the playback card, queue list, and visualizer; the right column holds the tab bar, player, library list, and status bar. The user can collapse the left column (the `x` key) so the library spans the full width, which helps when the terminal is narrow or the queue is not needed. That collapse is a binary toggle: both panels, or right-only.

There is no way to go the other way — to give the queue the full width. On a narrow terminal, or when the user is curating the queue, the library column wastes space and the queue reads cramped. The natural extension is a three-way cycle: both panels, then right-only (today's collapse), then left-only, then back to both. One key, one forward cycle, no new settings.

The user asked for exactly this: focus the whole window on whichever side matters most at the moment, on start, defaulting to both.

## What Changes

- Replace the `queue_column_collapsed: bool` flag on `App` with a tri-state `panel_mode: PanelMode` enum with values `Both`, `LibraryOnly`, and `QueueOnly`. `Both` is the constructed default; nothing is persisted, matching today's behavior.
- `x` (`Command::TogglePowerSidebar`) becomes `Command::CyclePanelMode`, advancing one step per press: `Both -> LibraryOnly -> QueueOnly -> Both`.
- Focus follows the mode: `LibraryOnly` forces focus to the library panel (today's behavior when collapsing); `QueueOnly` forces focus to the queue panel.
- The layout renderer computes the left/right column widths from `panel_mode`: `Both` uses the stored `queue_column_width`, `LibraryOnly` gives the library the full width (today's collapsed path), and `QueueOnly` gives the queue the full width while the right column renders nothing.
- When the mode is not `Both`, the queue-column resize keys and the Alt+Left focus-return-to-queue key stay disabled, as they are today when collapsed.

## Capabilities

### New Capabilities

- `panel-mode`: the Power View layout is a three-state cycle controlled by `x`, allowing the full window to show both panels, only the library, or only the queue.

### Modified Capabilities

None. The existing two states (both panels, library-only) behave exactly as they do today; the new state is queue-only.

## Impact

- **Code**: `src/app/app_struct.rs` (field), `src/app/action.rs` (command + dispatch), `src/app/input_lib_power_keys.rs` (handler), `src/app/input_resolver.rs` (binding name), `src/app/input_queue_keys.rs` (guards), `src/app/render/mod.rs` (layout), `src/app/construct.rs` and `src/app/tests.rs` (construction). Tests in `src/app/input_resolver_handle_key_tests.rs` and `src/app/input_power_movie_detail_tests.rs`.
- **Behavior**: `x` now cycles through three states instead of toggling two. A press from the default shows the library full width; a second press shows the queue full width; a third returns to both panels.
- **Data/API**: None. No persisted settings change; `queue_column_width` is untouched.
- **Risk**: Low-medium. The renderer branches on the flag at six sites; each becomes an enum match, and the new `QueueOnly` branch must render an empty right column without panicking on degenerate `Rect`s.

## Non-Goals

- Persisting the panel mode across sessions.
- A separate key or chord for each state; the cycle is one key.
- Changing the queue column width, its resize keys, or the stored `queue_column_width` preference.
- New visuals or styling for the panels; the existing renderers draw into whatever width they are given.

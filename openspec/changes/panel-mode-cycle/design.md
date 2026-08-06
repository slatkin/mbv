## Context

The Power View layout is computed in `render_power` (`src/app/render/mod.rs`). Today the left (queue) column width comes from two places that must agree:

- `queue_column_width: u16` (persisted preference) — the stored width of the left column.
- `queue_column_collapsed: bool` (runtime only) — when true, the left column width is forced to `0` and the library gets the full window.

The renderer branches on `queue_column_collapsed` at six sites in `render/mod.rs` (lines 275, 286, 303, 394, 429, 470), and the input layer checks it in `input_queue_keys.rs` (lines 23 and 71). The `x` handler (`input_lib_power_keys.rs:51-56`) dispatches `Command::TogglePowerSidebar` (`action.rs:98`), whose dispatch (`action.rs:564-571`) flips the flag and moves focus from Queue to Library when it becomes true.

The design replaces the boolean with a tri-state enum. Every existing check becomes a match, the new `QueueOnly` state is the mirror image of the existing `LibraryOnly` state, and the focus rule extends symmetrically.

## State and command

`src/app/app_struct.rs`:

- Remove `pub(super) queue_column_collapsed: bool`.
- Add `pub(super) panel_mode: PanelMode` with `use super::super::PanelMode` or a `mod` alongside — follow the existing `types_settings.rs` pattern for where the enum type lives (near `PanelFocus`). Suggested placement: `src/app/types_settings.rs` next to `PanelFocus`, since both are small layout/focus enums.

```rust
pub(super) enum PanelMode {
    Both,
    LibraryOnly,
    QueueOnly,
}
```

Constructed as `PanelMode::Both` in `construct.rs` (replacing `queue_column_collapsed: false`) and in the test constructor `tests.rs`.

`src/app/action.rs`:

- Rename `Command::TogglePowerSidebar` to `Command::CyclePanelMode`.
- Dispatch advances one step: `match self.panel_mode { PanelMode::Both => PanelMode::LibraryOnly, PanelMode::LibraryOnly => PanelMode::QueueOnly, PanelMode::QueueOnly => PanelMode::Both }`.
- Focus rule runs after the step:
  - `LibraryOnly` and `panel_focus == Queue` -> `set_panel_focus(PanelFocus::Library)` (today's behavior, unchanged).
  - `QueueOnly` -> `set_panel_focus(PanelFocus::Queue)`.
  - `Both` -> leave focus alone.

The resolver binding (`input_resolver.rs:195`) keeps key `x` and handler `handle_key_power_sidebar_toggle`; the name string `"sidebar_toggle_x"` and handler name may stay as-is to keep the resolver/test surface small, or be renamed to `panel_mode_cycle_x` / `handle_key_panel_mode_cycle` — the codebase favors meaningful names, so rename both.

## Layout

`src/app/render/mod.rs`. The six branches on `queue_column_collapsed` become matches on `self.panel_mode`:

- `left_w` (line 275): `Both => self.queue_column_width`, `LibraryOnly => 0`, `QueueOnly => area.width`.
- `left_area` (line 286): `Rect::default()` when `LibraryOnly` (as today when collapsed); full window in `QueueOnly`; the existing rect in `Both`.
- Left background (line 303): rendered whenever `panel_mode != LibraryOnly`.
- `(lib_area, queue_area)` split (line 394): `LibraryOnly => (right_area, Rect::default())` (as today); `QueueOnly => (Rect::default(), full-width queue rect)`; `Both => (right_area, queue rect under the card)`.
- Right-panel gutters (line 429, `power_right_panel_content_area`): the full-bleed/no-gutter library already applies when `LibraryOnly`; extend the same call to skip gutters in `QueueOnly` — in that state the argument is a `Rect::default()` so the library area must be produced without touching degenerate `Rect`s.
- Queue render guard (line 470): render queue list whenever `panel_mode != LibraryOnly` (today the guard is `!queue_column_collapsed`), which covers both `Both` and `QueueOnly`.

The `QueueOnly` queue area needs the same structure as today's left column (card on top, queue list below, optional visualizer at the bottom) but stretched to the full window width. The existing code path that builds `queue_area` under the card can be reused by feeding it the full-window rect as `left_content`; the queue list and visualizer then fill the whole column unchanged.

Degenerate `Rect` safety: in `LibraryOnly`, `Rect::default()` for the left column already renders nothing today; in `QueueOnly`, the right column rects (`right_full_area`, `tab_area`, `player_area`, `status_area`, `right_area`) become `Rect::default()`s. Verify each renderer that draws into them (tabs, player, status) skips a zero-width/zero-height rect, since some `ratatui` widgets can panic on zero-dimension areas. Where a renderer does not already bail on empty areas, add a guard.

## Input guards

`src/app/input_queue_keys.rs`:

- Line 23 (`handle_queue_column_width_key` early-return): change `|| self.queue_column_collapsed` to `|| self.panel_mode != PanelMode::Both`.
- Line 71 (Alt+Left return-to-queue): change `&& !self.queue_column_collapsed` to `&& self.panel_mode == PanelMode::Both`.

## Tests

- `src/app/input_resolver_handle_key_tests.rs`: update `x_toggles_sidebar_via_handle_key` to a cycle test (`Both -> LibraryOnly -> QueueOnly -> Both`); update `x_moves_queue_focus_to_library_when_collapsing_power_sidebar` for the enum; `x_does_not_toggle_power_sidebar_while_context_menu_is_open_via_handle_key` still passes with the new name; `h_no_longer_toggles_sidebar_via_handle_key` unchanged. Replace any direct `queue_column_collapsed` field writes with `panel_mode` writes.
- `src/app/input_power_movie_detail_tests.rs`: replace `app.queue_column_collapsed = true` with `app.panel_mode = PanelMode::LibraryOnly`.
- `src/app/render/tests_queue.rs` and any render test reading `queue_column_collapsed`: set `panel_mode` explicitly (the constructor defaults to `Both`, which preserves today's both-panels expectations).
- New tests:
  - Full cycle through all three states on successive `x` presses, asserting `panel_mode` and `panel_focus` after each.
  - `QueueOnly` forces focus to Queue even when the library was focused.
  - `Both` leaves focus untouched when cycling back.
  - Resize keys and Alt+Left are inert in `QueueOnly` and in `LibraryOnly`.
  - A render test asserting the `QueueOnly` layout yields a queue area spanning the full width and a defaulted right column.

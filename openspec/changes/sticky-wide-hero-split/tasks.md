## 1. Drag-end message and shell persistence

- [x] 1.1 Add `ResizeListPaneEnd(u16)` to `ShellRequest` and emit it from `split_gesture_msg` on `DragEnd` only when the gesture changed the width; verify the existing click-without-motion and outside-gap cases still emit nothing via the panel focused-component tests.
- [x] 1.2 Handle `ResizeListPaneEnd` in `shell_messages.rs` (set `app.list_pane_width` through `normalize_list_pane_width`, then `save_prefs()`); verify with a unit test that Live sets memory without writing prefs and End writes prefs.

## 2. Prefs save/load

- [x] 2.1 Add `list_pane_width` (nullable) to `save_prefs`/`load_prefs` and initialize `App.list_pane_width` from prefs in `construct.rs` instead of hard-coding `None`; verify round-trip (width, null, missing key, non-numeric) with a prefs unit test.
- [x] 2.2 Add `save_prefs()` to the library arm of `refresh_current_view` after the existing `list_pane_width = None` clear; verify refresh-then-load yields the default split via a test that persists a width, refreshes, and reloads prefs.

## 3. Integration and spec parity

- [ ] 3.1 Extend the wide-split live-tick integration test (`tests_tick_integration_wide_split.rs`) to drive press/drag/release through `Application::tick()` and assert one prefs write with the final width; verify no write occurs for press-release without motion.
- [ ] 3.2 Run `cargo nextest run -p mbv` for the touched areas plus `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt`; verify the delta-spec scenarios (persisted restore, refresh clears persisted, clamp on narrow terminal) each have covering tests.

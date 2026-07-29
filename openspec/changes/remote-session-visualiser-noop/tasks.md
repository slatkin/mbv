## 1. Gate the visualiser binding

- [x] 1.1 Update `App::handle_key_visualizer` in `src/app/input.rs` to return `Some(false)` without side effects when `connected_session_id.is_some()`.
- [x] 1.2 Preserve existing handling for local playback and direct remote daemon connections without an attached session id.

## 2. Verify behavior

- [x] 2.1 Add a focused test that pressing `v` with an attached remote session leaves `visualizer_enabled` unchanged and emits no visualiser command.
- [x] 2.2 Add or retain a focused test that pressing `v` without an attached session still toggles the visualiser.
- [x] 2.3 Run formatting and the focused test target.

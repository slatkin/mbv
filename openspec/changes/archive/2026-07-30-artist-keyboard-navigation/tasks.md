## 1. Cursor navigation logic

- [x] 1.1 Add `jump_power_music_group_display_cursor_to_artist(&mut self, lib_idx: usize, forward: bool) -> bool` method in `src/app/render/album_cursor.rs` that builds the display plan, finds the current position in the target list, scans for the next/previous `ArtistHeader` target, and applies the selection (or falls back to last/first item at boundaries)

## 2. Key dispatch

- [x] 2.1 Add Ctrl+PageUp and Ctrl+PageDown arms in `handle_lib_key()` in `src/app/input_lib_power_keys.rs` that call `jump_power_music_group_display_cursor_to_artist(lib_idx, false/true)` when in the grouped album view, and fall through to existing PageUp/PageDown behavior otherwise

## 3. Verification

- [x] 3.1 Run `cargo build` and fix any compilation errors
- [x] 3.2 Run `cargo clippy` and `cargo fmt` to ensure code quality

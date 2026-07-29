## 1. API layer

- [x] 1.1 Add `rename_playlist(&self, playlist_id: &str, new_name: &str) -> Result<(), String>` method in `crates/mbv-core/src/api_client_playlists.rs` that POSTs to `/Items/{playlist_id}` with JSON body `{"Name": new_name}` using the existing builder pattern.

## 2. Type definitions

- [x] 2.1 Add `RenamePlaylist { id: String }` variant to `SavePlaylistStage` in `src/app/types_feed.rs`.
- [x] 2.2 Add `DeletePlaylist { id: String, name: String }` variant to `ConfirmAction` in `src/app/types_confirm.rs`.
- [x] 2.3 Add `PlaylistRenamed { id: String, new_name: String }` and `PlaylistDeleted { id: String, name: String }` variants to `LibEvent` in `src/app/types_events.rs`.

## 3. Async spawn helpers

- [x] 3.1 Add `spawn_rename_playlist(&mut self, playlist_id: String, new_name: String)` in `src/app/library_load_actions.rs` — spawns a thread that calls `rename_playlist()`, sends `LibEvent::PlaylistRenamed`, then sends `LibEvent::PlaylistsLoaded` to refresh the list.
- [x] 3.2 Add `spawn_delete_playlist(&mut self, playlist_id: String, name: String)` in `src/app/library_load_actions.rs` — spawns a thread that calls `delete_playlist()`, sends `LibEvent::PlaylistDeleted`, then sends `LibEvent::PlaylistsLoaded` to refresh the list.

## 4. Event handling

- [x] 4.1 Handle `LibEvent::PlaylistRenamed` in `src/app/lib_event_actions.rs` — flash status "Renamed to '<new_name>'" and clear the rename dialog.
- [x] 4.2 Handle `LibEvent::PlaylistDeleted` in `src/app/lib_event_actions.rs` — flash status "Deleted '<name>'" and clear the confirmation modal.

## 5. Key handlers

- [x] 5.1 Add `KeyCode::Char('n')` arm in `handle_key_playlists()` in `src/app/input_playlist_keys.rs`, gated by `self.playlists_open.is_none()` — opens the rename dialog pre-filled with the selected playlist's name, setting `save_playlist_dialog` with stage `RenamePlaylist { id }`.
- [x] 5.2 Add `KeyCode::Char('d')` arm in `handle_key_playlists()` in `src/app/input_playlist_keys.rs`, gated by `self.playlists_open.is_none()` — sets `confirm_modal` to a `DeletePlaylist` confirmation.
- [x] 5.3 Update `handle_save_playlist_key()` in `src/app/input_playlist_keys.rs` to handle `RenamePlaylist` stage on Enter (spawn rename, clear dialog) vs the existing `EnterName`/`ConfirmOverwrite` behavior.
- [x] 5.4 Add `ConfirmAction::DeletePlaylist` arm in `handle_key_confirm_modal()` in `src/app/input_confirm_keys.rs` — on `y`, spawn delete and clear the modal.

## 6. Rendering

- [x] 6.1 Update the hint bar in `src/app/render/overlays/playlists.rs` (line 30) to show `"[n]rename [d]delete"` alongside existing hints in the list view.
- [x] 6.2 Update `render_save_playlist_dialog()` in `src/app/render/overlays/playlists.rs` to show "Rename Playlist" as the title when the stage is `RenamePlaylist`, and pre-fill the input with the current playlist name.

## 7. Verification

- [x] 7.1 Run `cargo build` and fix any compilation errors.
- [x] 7.2 Run `cargo clippy` and `cargo fmt` to ensure code quality.

## Why

The playlists panel (F4) is read-only — users can browse and play but cannot rename or delete playlists without leaving the TUI. Adding rename and delete operations inline keeps users in the TUI workflow and brings the playlists panel to feature parity with basic playlist management.

## What Changes

- Add `rename_playlist()` method to `EmbyClient` (POST /Items/{id} with `{"Name": "..."}` body) in `api_client_playlists.rs`.
- Add key bindings in the playlists panel list view: `n` for rename (opens a name-input dialog pre-filled with the current playlist name), `d` for delete (opens a confirmation modal).
- Reuse the existing `SavePlaylistDialog` and `SavePlaylistStage` pattern for rename name input, extended with a new `RenamePlaylist` stage.
- Reuse the existing `ConfirmModal` + `ConfirmAction` pattern for delete confirmation.
- Update the hint bar in the playlists panel to show the new key options.
- Wire up async API calls through the existing `LibEvent` / background-thread pattern.

## Capabilities

### New Capabilities

- `playlist-management`: Rename and delete playlists from the playlists panel (F4), with inline name editing and destructive-action confirmation.

### Modified Capabilities

(none)

## Impact

- `crates/mbv-core/src/api_client_playlists.rs` — new `rename_playlist()` method.
- `src/app/types_feed.rs` — extend `SavePlaylistStage` with `RenamePlaylist { id: String }` variant.
- `src/app/types_confirm.rs` — new `ConfirmAction::DeletePlaylist { id: String, name: String }` variant.
- `src/app/types_events.rs` — new `LibEvent` variants for async rename/delete results.
- `src/app/input_playlist_keys.rs` — key handlers for `n` (rename) and `d` (delete) in playlist list view.
- `src/app/input_confirm_keys.rs` — dispatcher arm for `DeletePlaylist` confirmation.
- `src/app/lib_event_actions.rs` — handle new `LibEvent` variants.
- `src/app/library_load_actions.rs` — spawn helpers for rename/delete async API calls.
- `src/app/render/overlays/playlists.rs` — update hint bar, render rename dialog.
- `src/app/app_struct.rs` — possibly rename-related state fields if a separate dialog is used.

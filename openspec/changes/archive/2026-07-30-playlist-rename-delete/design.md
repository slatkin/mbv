## Context

The playlists panel (opened with F4) renders a list of Emby playlists with keyboard navigation (Up/Down/PageUp/PageDown/Home/End), browsing (Right into playlist items), playback (Enter), and refresh (r). The panel has no way to rename or delete playlists — these operations are only available through the Emby web UI.

The Emby API supports rename via `POST /Items/{ItemId}` with body `{"Name": "New Name"}` and delete via `DELETE /Items/{Id}`. The codebase already has `delete_playlist()` in `api_client_playlists.rs` (used only for the save-overwrite flow) but no `rename_playlist()`.

Existing patterns we reuse:
- **Name input dialog**: `SavePlaylistDialog` / `SavePlaylistStage::EnterName` — captures text input in a centered dialog, renders with block cursor, dispatches on Enter.
- **Confirmation modal**: `ConfirmModal` / `ConfirmAction` — shared modal for destructive actions, dispatched in `input_confirm_keys.rs`.
- **Async API calls**: Background threads send `LibEvent` variants via `mpsc::Sender<LibEvent>`, handled in `handle_lib_event()`.
- **Status messages**: `flash_status()` shows a transient status bar message.

## Goals / Non-Goals

**Goals:**
- Rename a playlist inline: press `n` on a playlist in the list view → name-input dialog opens pre-filled with current name → user edits → Enter saves via API → list refreshes → status message confirms.
- Delete a playlist inline: press `d` on a playlist in the list view → confirmation modal "Delete playlist 'X'?" → `y` confirms → API call → list refreshes → status message confirms.
- Rename dialog rejects empty names (just like the save dialog does).
- Delete confirmation requires explicit `y` to avoid accidental data loss.
- Panels and scroll positions are preserved across the operation (no cursor reset beyond the natural list refresh).

**Non-Goals:**
- Bulk rename/delete of multiple playlists.
- Rename or delete while browsing inside a playlist (only available from the top-level playlist list view).
- Undo support.
- Keyboard-agnostic key bindings (hardcoded `n`/`d` same as existing `r` for refresh).

## Decisions

### 1. Extend SavePlaylistStage for rename

Add a `RenamePlaylist { id: String }` variant to `SavePlaylistStage`. When the rename dialog is active, `save_playlist_dialog` holds `Some(SavePlaylistDialog { input: pre-filled current name, stage: RenamePlaylist { id } })`.

**Rationale**: Reuses the existing dialog input loop (`handle_save_playlist_key`) and rendering (`render_save_playlist_dialog`) with minimal code. The only changes are: the input is pre-filled, Enter calls `rename_playlist()` instead of `create_playlist()`, and Esc cancels back to the playlist list (clears the dialog without saving). The existing dialog rendering already supports variable content, so only the title text needs to change from "Save as Playlist" to "Rename Playlist".

**Alternative considered**: A separate `rename_playlist_dialog: Option<RenameDialog>` field. Rejected because it duplicates the input-loop logic and rendering that `SavePlaylistDialog` already provides. The stage enum approach is cleaner and is already used for `ConfirmOverwrite`.

### 2. Add ConfirmAction::DeletePlaylist for delete confirmation

Add `ConfirmAction::DeletePlaylist { id: String, name: String }` to the `ConfirmAction` enum. The `name` is stored for the flash message after successful deletion.

**Rationale**: Follows the exact same pattern as `SaveOverwritePlaylist`. The confirmation modal's dispatcher in `input_confirm_keys.rs` gets a new arm that calls the delete API, refreshes the list, and flashes a status message. No new field needed on `App`.

### 3. Key bindings: `n` for rename, `d` for delete, list view only

Both keys are matched in `handle_key_playlists()` under `KeyCode::Char('n')` and `KeyCode::Char('d')` with empty modifiers. They are gated by `self.playlists_open.is_none()` so they only apply in the top-level playlist list view, not when browsing a playlist's items.

**Rationale**: `n` and `d` are unused in the playlists panel today. Gating on `playlists_open.is_none()` prevents accidental deletion/rename while browsing playlist items. This matches how `Right` (browse into playlist) is gated.

### 4. Async pattern: spawn thread, send LibEvent

Both rename and delete follow the existing `spawn_load_playlists()` / `spawn_open_playlist()` pattern:
1. Clone the client and the `lib_tx` sender.
2. Spawn a thread that calls the API, then sends a `LibEvent` variant back.
3. `handle_lib_event()` receives the event, updates state, and flashes a status message.

New `LibEvent` variants: `PlaylistRenamed { id: String, new_name: String }` and `PlaylistDeleted { id: String, name: String }`.

**Rationale**: Consistent with all other background API work in the app. No new channels, no new threading model. The status flash provides immediate feedback after the async operation completes.

### 5. Delete confirmation flow

The `d` key press in the list view immediately sets `self.confirm_modal` to a `ConfirmModal` for `DeletePlaylist`. The user must press `y` (in the confirmation modal) to actually trigger the deletion. Pressing any other key that the modal doesn't handle (including `Esc`) clears the modal without deleting.

**Rationale**: Deletion is destructive and irreversible — requiring an explicit `y` press prevents accidental data loss. This is the same pattern used for `ClearQueue`, `RemoveActiveQueueItem`, `RescanLibrary`, and `SaveOverwritePlaylist`.

## Risks / Trade-offs

- [Risk] Rename dialog reuses `save_playlist_dialog`, which means it blocks the save flow while rename is active. → Mitigation: The save dialog and rename dialog are mutually exclusive in practice (save is triggered from the queue panel, rename from the playlists panel). Only one can be open at a time since they occupy the same `Option<SavePlaylistDialog>` slot, but these operations never overlap in normal usage.
- [Risk] Playlist list may show stale data briefly after rename/delete if the Emby server is slow. → Mitigation: We refresh the list after the API call completes (via `spawn_load_playlists()`), and the `playlists_loading` flag prevents double-loads.
- [Risk] User may have the playlist open (browsing items) while renaming it from the list view. → Mitigation: Not a concern — rename only available from list view, and the open playlist state is checked by ID on refresh, so if the playlist was renamed and then the user refreshes the open view, it will correctly reload by the (unchanged) ID.

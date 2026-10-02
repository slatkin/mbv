# Tasks

## 1. Autosave Playlists setting and glossary

- [ ] 1.1 In `crates/mbv-config`, replace the `save_playlist_on_consume` and `save_playlist_on_consume_audio` fields with `autosave_playlists: bool` (default false). In `parse.rs`, read `autosave_playlists` if present; otherwise use the OR of the two retired keys. In `save.rs`, write only `autosave_playlists`. Replace the existing `save_playlist_on_consume_audio` tests in `crates/mbv-config/src/tests/settings.rs` with one `#[case]` table covering the contract "a retired key set to true migrates to on, and the saved file has only the new key". Verify with `cargo nextest run -p mbv-config`.
- [ ] 1.2 In `crates/mbv-ui-model/src/settings.rs`, merge `SettingKey::SavePlaylistOnConsume` and `SettingKey::SavePlaylistOnConsumeAudio` into `SettingKey::AutosavePlaylists`, labelled "Autosave playlists". Update the toggle arm in `src/app/dispatch/settings.rs` and the case list in `src/app/tests/settings_activation.rs`. Add `autosave_playlists` to `OwnerSettings` (`crates/mbv-daemon/src/owner_settings.rs`) and to `fixed_reader`. Verify with `cargo check --workspace` and `cargo nextest run -p mbv-ui-model`.
- [ ] 1.3 In `CONTEXT.md`, remove **Save on consume** (and its Avoid line). Add **Autosave playlists**: the owner writes every change to a linked playlist back to Emby; Avoid "save on consume", "consume persistence". Add **Saved baseline**: the Emby item ids Emby last confirmed for the linked playlist. Update the **Queue source** entry, which mentions "save-on-consume decisions". Verify by reading the entries back.

## 2. Wire protocol

- [ ] 2.1 In `crates/mbv-ctrl`:
  - add `LinkedPlaylistState { NotLinked, Saved, Unsaved, Saving, SaveFailed }` (`NotLinked` is the default);
  - add `UnifiedQueueStateData.linked_playlist` with `serde(default)`;
  - add `CtrlCmd::UnifiedQueueUndo { op: QueueOpId }`;
  - add `saved_content: Option<Vec<String>>` (`serde(default)`) to `UnifiedQueueSourceUpdate`;
  - add the hello capability `owner-undo-playlist-state` and `CtrlCompatibility.supports_owner_undo`, true for the current protocol version.

  Mirror these in `crates/mbv-remote-player` (`QueueOp::Undo` and `QueueOp::SourceUpdate { saved_content }`) and in its ctrl translation. Contract test: a `UnifiedQueueStateData` JSON without `linked_playlist` decodes as `NotLinked`. Verify with `cargo nextest run -p mbv-ctrl -p mbv-remote-player` and `cargo check --workspace`. Call sites pass `saved_content: None` for now.

## 3. Owner-held undo

- [ ] 3.1 Add `crates/mbv-daemon/src/queue_undo.rs`: a bounded history (cap 100, oldest dropped first) of `Removed { item, index }`, `Moved { slot_id, from }` and `Appended { slot_ids }`. Store it on `DaemonPlayerOwner`. Record entries where the owner applies edits:
  - `handle_queue_remove_slot` and `handle_queue_remove_slots`: one `Removed` per item, in the order the TUI's `remove_slots` path pushed them;
  - `handle_queue_move_slot`;
  - `handle_queue_append`: the appended slot ids;
  - the consume branch reached from `handle_track_completed`.

  Clear the history on Replace, idle load, and Clear. Verify with `cargo check -p mbv-daemon`.
- [ ] 3.2 Handle `UnifiedQueueUndo`: pop one entry and apply the inverse through the same internal edit paths without recording it. For a removal, insert before the slot now at `index`, or at the end. For an append, remove the appended slots that still exist. Answer `QueueOpResult`:
  - `Applied(state)` when an entry was undone;
  - `Rejected("nothing to undo")` when the history is empty;
  - the existing stale-slot rejection when the entry no longer applies.

  The handler returns `EventOutcome::DIRTY` when it changes the queue. Contract tests in `crates/mbv-daemon/src/tests/` (one behaviour each), citing the `unified-playback-queue` "Queue undo is an owner operation" scenarios:
  - undoing a consume re-inserts at the former position;
  - undoing an append removes the appended slots;
  - undoing a move whose slot is gone is rejected;
  - undoing after a Replace answers nothing to undo.

  Verify with `cargo nextest run -p mbv-daemon`.

## 4. Owner playlist autosave

- [ ] 4.1 Add `crates/mbv-daemon/src/playlist_autosave.rs`. It holds `SavedBaseline { playlist_id, ids }` and `WriteState { Idle, InFlight { ids, pending }, Failed { ids } }`. A pure function `linked_state(source, content, baseline, write) -> LinkedPlaylistState` computes the state, and a pure `content(queue)` returns the Emby item ids in queue order. Contract test: one `#[case]` table over `linked_state` with one case per state (NotLinked, Saved, Unsaved, Saving, SaveFailed), plus "a non-Emby item does not change content". Verify with `cargo nextest run -p mbv-daemon`.
- [ ] 4.2 Persist the baseline: add `saved_baseline` (`serde(default)`) to `mbv_config::StayAliveQueueState`, written by `stay_alive_owner_queue_snapshot` and restored with the queue. When a restored queue names a playlist but has no baseline, adopt the restored content as the baseline. Set the baseline when the owner accepts a Replace or idle load whose source is `Playlist { id: Some(_) }`. Set it when it accepts a lineage-valid `SourceUpdate` carrying `saved_content` for a source that names a playlist. Drop it when the source stops naming that playlist. Fill `linked_playlist` in `unified_queue_state_for_peer`. Contract test: a persisted linked queue whose content differs from its baseline restores as `Unsaved`. Verify with `cargo nextest run -p mbv-daemon -p mbv-config`.
- [ ] 4.3 Autosave trigger and write. In `handle_event`, next to `persist_owner_queue_if_dirty`, under the same condition (Local role and `owner_queue_dirty`): if the queue is linked, `autosave_playlists` is on, and the content differs from the baseline, either start a write or replace `pending`. Then broadcast the queue state again if the linked state changed.
  - **The write.** It spawns a thread running an injected playlist writer (production: the owner's `EmbyOwnerContext` client calling `update_playlist_items`; tests: a recording fake that returns the outcome given to it, with no sleeps). The thread sends a new `DaemonEvent::PlaylistAutosaveComplete { playlist_id, ids, result }`.
  - **Completion.** Success sets the baseline (only if that playlist is still linked) and starts `pending` if it differs. Failure sets `Failed`. A write still in flight or pending for a playlist the queue no longer links to is finished anyway.
  - **Never.** No write for the packaged role, and none caused by Replace or Clear.

  Contract tests, citing `playlist-autosave` scenarios:
  - a headless consume writes the shortened content;
  - three edits during an in-flight write produce one further write with the final content;
  - Clear writes nothing;
  - with autosave off, a consume reports Unsaved and writes nothing;
  - a failed write reports SaveFailed and the next change writes again.

  Verify with `cargo nextest run -p mbv-daemon`.

## 5. TUI reads owner playlist state

- [ ] 5.1 Remove `App.queue_dirty` and every set and clear site. These include:
  - `queue.rs:175` and `queue.rs:311`, `actions.rs:559`, `confirm_keys.rs:200`;
  - `consume_quit.rs`, `queue_scope.rs`, `replacement.rs`, `run_loop/session.rs`;
  - `emby_service.rs`, `audiobookshelf/service.rs`.

  Also remove `pending_owner_source_update` and the dirty clearing in `adopt_owner_source`. Add a read of the adopted snapshot's `linked_playlist` (`App::linked_playlist_state()`, `NotLinked` with no snapshot). Wherever the code used `queue_dirty && queue_is_saved_playlist()` (the dirty-playlist prompt trigger and `try_quit`), use `matches!(state, Unsaved | SaveFailed)` instead. Delete the save branch of `on_video_consumed` and `on_audio_consumed`, keeping only what still serves the consume reaction, or delete the methods if nothing remains. Verify with `cargo check -p mbv`.
- [ ] 5.2 Pill: `autosave_status_spans` maps Unsaved to UNSAVED, Saving to SAVING, SaveFailed to SAVE FAILED, and Saved with `autosave_playlists` on to AUTOSAVE; anything else shows nothing. Reuse the existing UNSAVED and AUTOSAVE glyphs and pick nerd-font glyphs for SAVING and SAVE FAILED. Verify with `cargo check -p mbv`.
- [ ] 5.3 Explicit saves report content. Include the ids it wrote as `saved_content` in:
  - in-place Save completion (`PlaylistMutationComplete` success for the current origin and playlist): send a `SourceUpdate` with the unchanged source;
  - Save As and overwrite (`apply_saved_playlist_source`).
- [ ] 5.4 Update the tests that relied on the old flag, deleting rather than porting:
  - delete the `save_playlist_on_consume*` tests in `src/app/tests/queue/consume.rs`, now covered by 4.3;
  - change the confirm-prompt tests in `src/app/input/confirm_keys/tests.rs` to set the owner snapshot's `linked_playlist` instead of `queue_dirty`.

  Verify with `cargo nextest run -p mbv`.

## 6. TUI undo through the owner

- [ ] 6.1 `QueueRequest::Undo` sends `QueueOp::Undo` through `queue_op` when the owner link reports `supports_owner_undo`. Otherwise it shows "Undo is not supported by this player"; the existing remote-scope toast stays. Show the rejection reason as a toast when the answer is rejected. Delete `UndoEntry`, `queue_undo_stack`, `undo_stack_for_scope_mut`, the pushes in the remove path and in `record_undoable_queue_edit`, and the tests that assert on the client stack (`queue_op.rs` undo tests, the `consume.rs` undo-stack assertion, the `shell/queue.rs` remote-undo test seeding `UndoEntry`). Replace them with one contract test: Undo sends `UnifiedQueueUndo` as an answered op. Verify with `cargo nextest run -p mbv`.
- [ ] 6.2 Change the help line `Ctrl+Z  Undo removal` (`crates/mbv-render/src/components/help.rs`) to `Undo`. Verify with `cargo check -p mbv-render`.

## 7. Integration

- [ ] 7.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace`; all three must pass.
- [ ] 7.2 Manual check against a real Emby server with stay-alive on. Exercise this sequence:
  1. load a playlist and see it read Saved;
  2. consume with the TUI closed, then see the playlist shortened in the Emby web UI;
  3. reattach and undo the consume; the item returns locally and on Emby;
  4. remove and move entries; the pill goes SAVING and then AUTOSAVE;
  5. turn autosave off, edit, and see UNSAVED, the replace prompt, and save on quit;
  6. clear the queue and confirm the Emby playlist is untouched.

  Record the result in the PR description.

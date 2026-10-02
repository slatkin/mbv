# Proposal

## Why

Saving a playlist automatically currently happens only on consume. It is controlled
by two settings (`save_playlist_on_consume` for video, `save_playlist_on_consume_audio`
for audio), and the save runs in whichever TUI happens to be attached. Any other edit
(remove, move, add, delete-playing, undo) leaves the playlist "unsaved". Consumes that
happen while no TUI is attached to the Stay-alive process are never saved at all. The
UNSAVED / AUTOSAVE pill is driven by a Client-local `queue_dirty` bool:
- it is set from five scattered call sites;
- it is set for queues that have no playlist;
- it is cleared before a save succeeds;
- it resets on TUI restart.

So it does not reliably show whether the playlist matches Emby.

Once every edit saves straight to Emby, discarding unsaved changes can no longer undo
a mistake. Undo becomes the only way back, so it has to cover every edit that
autosaves.

## What Changes

- **BREAKING (config)**: `save_playlist_on_consume` and `save_playlist_on_consume_audio`
  are replaced by one setting, **Autosave Playlists** (`autosave_playlists`). On first
  load the setting starts on when either old key was true. The old keys are no longer
  written.
- The Stay-alive Player owner (Local role only; packaged mbvd excluded) knows when its
  queue is linked to an Emby playlist (the source is `QueueSource::Playlist { id }`). It
  keeps the **saved baseline**: the Emby item ids that Emby last confirmed for that
  playlist. When Autosave Playlists is on, any accepted change that makes the queue's
  Emby item ids differ from the baseline triggers a background write to Emby. This
  holds however the change happened: consume, remove, move, append, delete-playing,
  undo, or another Client's edit. The write never blocks the owner's event loop. At
  most one write per playlist is in flight, and the newest pending ids replace older
  ones.
- Clear and Replace never autosave. They change the queue's source instead of editing
  the playlist.
- Linked-playlist save state is computed by the owner and carried in its queue
  snapshot. Its states are: not linked, saved, unsaved, saving, and save failed. The
  TUI pill, the unsaved-playlist prompt and save-on-quit read this
  state instead of `queue_dirty`. `queue_dirty` is removed.
- Explicit saves (the prompt's Save, save on quit, Save As, overwrite) stay in the
  TUI. After a successful write, the TUI reports the saved ids to the owner, and the
  owner adopts them as the baseline.
- **Undo history moves to the Player owner.** The owner records an undo entry for each
  edit it applies and for each consume:
  - append, undone by removing the appended slots;
  - remove and multi-remove, undone by re-inserting at the former position;
  - move, undone by moving back;
  - consume, undone by re-inserting at the former position.

  Ctrl+Z sends a single answered Undo request. The owner applies the reverse edit, and
  that edit autosaves like any other. Replace and Clear discard the history. The
  history is held in memory with a fixed cap. The TUI's `queue_undo_stack` and
  `UndoEntry` are removed.
- A new ctrl capability advertises owner-held undo and linked-playlist state. The TUI
  sends Undo only to owners that advertise it.
- Help text "Undo removal" becomes "Undo".
- Last-writer-wins: an autosave overwrites playlist edits made elsewhere (for example,
  in the Emby web UI) while mbv held the playlist. This is a known limit.

## Capabilities

### New Capabilities
- `playlist-autosave`: the Autosave Playlists setting, the owner-held saved baseline,
  the linked-playlist save state, background autosave, and how explicit saves update
  the baseline.

### Modified Capabilities
- `unified-playback-queue`:
  - "Queue undo is an owner operation": the history moves to the owner and covers
    append and consume.
  - "A queue replacement deferred behind the unsaved-playlist prompt…": unsaved now
    comes from the owner's state, and an autosave never runs a held replacement.
  - "The Stay-alive process holds the queue source": clean is reported from the
    owner's baseline instead of from the source arriving.

## Impact

- **Wire protocol** (`crates/mbv-ctrl`):
  - an Undo command;
  - a playlist-saved report command;
  - a linked-playlist state field on `UnifiedQueueStateData` (`serde(default)`);
  - a capability string and a `CtrlCompatibility` bit.
- **Owner** (`crates/mbv-daemon`):
  - the undo stack;
  - recording undo entries in `control/queue_edit.rs`, `control/queue_setup.rs` and the
    consume path in `event_loop/player_events.rs`;
  - the baseline and the autosave worker, using the existing `EmbyOwnerContext` client;
  - `OwnerSettings` gains `autosave_playlists`;
  - the baseline is persisted in `StayAliveQueueState`.
- **Config** (`crates/mbv-config`): key migration in `parse.rs` and `save.rs`; the
  setting row in `crates/mbv-ui-model/src/settings.rs`.
- **TUI** (`src/app`):
  - remove `queue_dirty`, `UndoEntry` and `queue_undo_stack`, and simplify
    `on_*_consumed` and `record_undoable_queue_edit`;
  - the pill (`chrome_status.rs`), the dirty-playlist prompt and `try_quit` read the
    owner's state;
  - explicit saves report their ids to the owner;
  - the undo dispatch sends a request.
- **Glossary** (`CONTEXT.md`): retire **Save on consume** and add **Autosave
  playlists** and **Saved baseline**.

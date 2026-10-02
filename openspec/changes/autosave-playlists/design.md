# Design

## Context

See proposal.md (Why). The current state the approach builds on:

- **Owner loop.** The Stay-alive owner runs `DaemonLoop` in `crates/mbv-daemon`. Every
  event handler returns `EventOutcome`, and `handle_event` persists the owner queue once
  per event when `owner_queue_dirty` is set, in the Local role only
  (`event_loop.rs:196`, `persist_owner_queue_if_dirty`). Every handler that changes the
  canonical queue already reports this, whatever the cause: ctrl edits, consume in
  `handle_track_completed`, idle loads, and Clear.
- **Emby access and settings.** The owner already has an Emby client
  (`EmbyOwnerContext.client`, built from owner storage for both roles). It re-reads
  config through `OwnerSettings` (`owner_settings.rs`), which is how it already sees
  the consume toggles.
- **Queue answers.** Answered queue ops reply with
  `QueueOpResult { op, outcome: Applied(state) | Rejected(reason) }`. Optional
  features are negotiated through hello capability strings and `CtrlCompatibility`
  bits.
- **TUI save pipeline.** The TUI's in-place save already runs in the background
  (`spawn_playlist_save`, one active mutation per playlist in `playlist_mutations`).
  Save As and overwrite end with a lineage-guarded `QueueOp::SourceUpdate`
  (`apply_saved_playlist_source`).
- **TUI undo.** The TUI keeps `queue_undo_stack: Vec<UndoEntry { Remove, Move }>` and
  sends inverse ops (`undo_last_queue_edit`). Append, delete-playing and consume push
  nothing.

## Goals / Non-Goals

**Goals:**
- One place in the owner decides "the playlist content changed", so no edit path can
  skip autosave.
- The saved/unsaved state is computed by the owner and never stored by a Client.
- The owner's event loop never waits on Emby.

**Non-Goals:**
- Autosave in packaged mbvd. Undo does reach mbvd, because it lives in the shared
  daemon library.
- Detecting or merging conflicts with edits made outside mbv (last-writer-wins).
- Persisting undo history across an owner restart.
- Redo.
- Undoing Replace or Clear.

## Decisions

### D1. Autosave runs at the existing per-event persistence point

Autosave is evaluated in `handle_event` right next to `persist_owner_queue_if_dirty`,
under the same condition: the Local role and `owner_queue_dirty`. If the queue is
linked, Autosave Playlists is on, and the content differs from the baseline, a write
starts. If evaluation changes the linked-playlist state (for example, unsaved to
saving), the queue state is broadcast again.

*Alternatives:*
- Hooking each mutation handler. That means many sites (append, remove, move, consume,
  undo, ...) and is exactly how today's consume-only gap arose.
- Hooking `broadcast_queue_state`. That runs from 23 sites, including ones that only
  change status.

The persistence point is already the single "the queue changed" signal.

### D2. Playlist content and the baseline are plain id lists; state is computed

- **Content** is the Emby item ids in queue order, the same projection the TUI's
  `start_playlist_mutation` sends to `update_playlist_items`.
- **Owner state** lives in a new `playlist_autosave` module in `mbv-daemon`:
  - `baseline: Option<SavedBaseline { playlist_id, ids }>`;
  - `write: WriteState`, one of `Idle`, `InFlight { ids, pending: Option<Vec<String>> }`,
    or `Failed { ids }`.
- **`LinkedPlaylistState`** (`NotLinked | Saved | Unsaved | Saving | SaveFailed`) is
  computed by a pure function of (source, content, baseline, write) and is never
  stored. The flags-in-a-struct shape is avoided per the type-invariant rule.
- **Wire field.** `UnifiedQueueStateData` gains `linked_playlist: LinkedPlaylistState`
  (`serde(default)` = `NotLinked`), filled in where the snapshot is built
  (`unified_queue_state_for_peer`). Older owners never send it, and older Clients
  ignore it.
- **Persistence.** The baseline is persisted as a new
  `StayAliveQueueState.saved_baseline` field (`serde(default)`).
- **When the baseline is set:**
  - the owner accepts a Replace or idle load whose source is
    `Playlist { id: Some(_) }`: baseline = the new content;
  - a write succeeds;
  - a playlist-content report arrives (D4).

  It is dropped when the source stops naming that playlist.

### D3. The write is a thread per save with at most one in flight per playlist; completion is a `DaemonEvent`

This follows the TUI's `spawn_playlist_save` pattern. Starting a write clones the
owner's Emby client and spawns a thread that calls `update_playlist_items`. The thread
sends a new `DaemonEvent::PlaylistAutosaveComplete { playlist_id, ids, result }` into
the owner's merged event channel.

- **Another change while a write is in flight** replaces `pending`, so only the newest
  content is written.
- **On completion:**
  - success sets the baseline to `ids`; if there is a `pending` that differs, the next
    write starts;
  - failure sets `Failed { ids }`;
  - in both cases, a write that is in flight or pending for a playlist the queue no
    longer links to is still finished (spec: "a pending write SHALL still be completed
    after the queue is replaced"). The baseline is then simply dropped.

*Alternative:* a long-lived worker modelled on `queue_persist_tx`. It would have to
coalesce per playlist across a channel; a thread per save plus `pending` is less code
and matches the TUI.

### D4. Explicit TUI saves report the content they wrote through `SourceUpdate`

`QueueOp::SourceUpdate` and `CtrlCmd::UnifiedQueueSourceUpdate` gain
`saved_content: Option<Vec<String>>` (`serde(default)`). The owner accepts the update
under the existing lineage check. When the content is `Some` and the new source names
a playlist, the owner sets that playlist's baseline to the content.

- Save As and overwrite already send this op and now include the ids they wrote.
- In-place Save (the prompt's Save answer and save on quit) sends it after success with
  the unchanged source.

The TUI's `pending_owner_source_update` and `adopt_owner_source` dirty-clearing are
removed: "saved" now arrives in the owner's snapshot.

*Alternative:* a new `PlaylistSaved` command. Rejected: it would duplicate the lineage
guard that `SourceUpdate` already has.

### D5. `queue_dirty` is replaced by reading `LinkedPlaylistState`

These all read the owner's state instead of the bool:
- `autosave_status_spans`;
- the dirty-playlist prompt's trigger;
- `try_quit`.

The prompt and quit-save act on `Unsaved | SaveFailed`. Pill states: Unsaved, Saving,
SaveFailed, and autosave on (Saved with the setting on). Each needs a label and a
nerd-font glyph; the existing UNSAVED and AUTOSAVE glyphs are reused, and Saving and
SaveFailed get new ones. All `queue_dirty = true/false` sites are deleted, along with the
save logic in `on_video_consumed` and `on_audio_consumed`, which moves to the owner.

### D6. Undo history is an owner-side bounded stack; Undo is one answered command

- **Owner stack.** A new `queue_undo` module in `mbv-daemon` holds a
  `VecDeque<UndoEntry>` capped at 100. Its entries are:
  - `Removed { item, index }`, used for remove, each item of a multi-remove, and
    consume;
  - `Moved { slot_id, from }`;
  - `Appended { slot_ids }`.
- **Where entries are recorded.** In the owner's apply paths, where the removed items
  and positions are known:
  - `control/queue_edit.rs` for remove, remove-slots and move;
  - `control/queue_setup.rs` for append;
  - the consume branch reached from `handle_track_completed`.

  A multi-remove pushes one entry per item, in the same order the TUI's current
  `remove_slots` path uses, so undo restores them one at a time just as it does today.
- **Clearing.** Replace, idle load and Clear clear the stack.
- **The command.** A new `CtrlCmd::UnifiedQueueUndo { op }` pops one entry, applies the
  inverse through the same internal edit functions, and does not record the inverse.
  It answers `QueueOpResult`:
  - `Applied(state)` when an entry was undone;
  - `Rejected("nothing to undo")` when the stack is empty;
  - the existing stale-slot rejection when the entry no longer applies.

  The undo's queue change sets `owner_queue_dirty`, so D1 autosaves it automatically.
- **TUI side.** `QueueRequest::Undo` sends this command through `queue_op` (new
  `QueueOp::Undo`). It shows the rejection text as a toast. `UndoEntry`,
  `queue_undo_stack`, `undo_stack_for_scope_mut` and `record_undoable_queue_edit`'s
  push are removed.

*Alternative:* Client-held history with consume entries pushed by attached Clients.
Rejected by the user: the Player marks items done, so it owns their reversal. It also
means one ordering, it covers headless consumes, and it survives TUI restarts.

### D7. Capability gating

A new hello capability string `owner-undo-playlist-state` and a
`CtrlCompatibility.supports_owner_undo` bit are added. The TUI sends Undo only when the
bit is set; otherwise it shows "Undo is not supported by this player". An owner without
the bit sends no `linked_playlist`, so its snapshots read `NotLinked` and the pill shows
nothing. This degrades gracefully, and owner and TUI ship from the same workspace.
`CtrlCompatibility` already carries an approved `struct_excessive_bools` expect that
covers additional `supports_*` bits, so no new suppression is needed.

### D8. Setting and migration

- **Config.** `mbv-config` replaces the two fields with `autosave_playlists: bool`.
  `parse.rs` reads `autosave_playlists` if present, otherwise the OR of the two retired
  keys. `save.rs` writes only the new key.
- **Settings row.** `SettingKey::SavePlaylistOnConsume` and
  `SettingKey::SavePlaylistOnConsumeAudio` become one `SettingKey::AutosavePlaylists`
  labelled "Autosave playlists".
- **Owner.** `OwnerSettings` gains `autosave_playlists`.
- **Glossary.** `CONTEXT.md` retires **Save on consume** and adds **Autosave
  playlists** and **Saved baseline**.

## Risks / Trade-offs

- [The owner overwrites edits made in the Emby web UI] → Accepted (last-writer-wins);
  recorded in the spec. Explicit Save already behaves this way.
- [`update_playlist_items` deletes then re-adds entries, so a crash mid-write can leave
  the playlist empty on Emby] → This is the existing behaviour of every save, and
  autosave just runs it more often. On restart the persisted baseline differs from the
  content, so the next change (or undo) rewrites it. Not mitigated further.
- [A write per consume or edit means more Emby traffic] → Coalescing (D3) bounds this to
  one write in flight plus one pending per playlist.
- [Two attached TUIs share one undo history, so Ctrl+Z in one undoes the other's edit]
  → Intended (spec scenario). It is a one-user app.
- [Owner restart loses undo history] → Accepted; the history is in memory by design.
- [Packaged mbvd has no autosave, and owners without the capability can't undo] →
  Owner and TUI ship together. The degraded path shows a toast instead of failing
  silently.

## Migration Plan

The config migration is read-time and one-way (D8). The persisted queue state and the
wire fields are additive with `serde(default)`. When the owner restores a linked queue
that has no persisted baseline, it adopts the restored content as the baseline, so the
playlist reads Saved instead of triggering a write at startup. Rollback: an older build ignores the new keys and fields, and the retired
consume keys are lost (autosave defaults off).

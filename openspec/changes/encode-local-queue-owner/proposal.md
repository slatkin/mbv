# Proposal

## Why

Issue #770: whether the Stay-alive process owns the Local queue is one bool
(`stay_alive_owner_is_queue_authority()`), and every caller rebuilds its own
Stay-alive branch from it. So we have repeated short-circuits, a Save As
completion that splits into two ~15-line paths, a separate owner-source
reconcile, and two lineage fields (`queue_lineage: u64` and
`owner_queue_lineage: Option<QueueLineage>`) passed side by side through
`PlaylistMutation` and `SessionEvent`. The scattering has already caused a bug:
the Overwrite-playlist completion only goes through the Bare-mode source writer.
Under Stay-alive that writer does nothing, so an overwrite never updates the
owner's Queue source, but the Client still marks the queue clean.

## What Changes

- Replace the bool with an exhaustively matched `LocalQueueOwner` enum
  (`ThisProcess`, `StayAlive`), derived from the Player endpoint. Each site
  that behaves differently per owner matches on it once. The per-owner work
  (persistence gate, generation fence, source adoption, playlist-source
  update) lives in one method each rather than being rebuilt at each call site.
- Rename the client-local counter `remote_queue_lineage: u64` to a
  `QueueEpoch` newtype. It is not a `QueueLineage`, and CONTEXT.md reserves
  "lineage" for owner-minted values.
- Fold the two fence fields into one `origin: QueueOrigin` field on every
  `PlaylistMutation` variant and on every playlist `SessionEvent` completion.
  `QueueOrigin` is `ThisProcess { epoch }` or `StayAlive { epoch, lineage }`,
  so the owner lineage can't be left off under Stay-alive or added under
  another owner.
- Save As and Overwrite completions go through one
  `apply_saved_playlist_source` path. Under Stay-alive both send an
  owner-lineage source update, and the queue is marked clean only after the
  owner's snapshot accepts it.
- **Behaviour fix:** Overwrite-playlist under Stay-alive now updates the
  owner's Queue source, the same way Save As does.
- **Behaviour change:** a Stay-alive Client that has no owner snapshot yet
  refuses Save, Save As and Overwrite when the request is made, with an error
  toast. Previously Save As created the Emby playlist and only then rejected
  the source update, leaving an orphan playlist.
- Delete `maybe_restore_queue_state` (a duplicate of `restore_queue_state`'s
  own guard) and `reject_stay_alive_queue_source_update`, which is folded into
  the single source-update path.
- Not taken from the issue: a separate `DirectRemote` variant. For the Local
  queue, a packaged-mbvd attachment behaves exactly like Bare at every
  affected site. Direct-remote handling is already typed by
  `QueueScope::Remote`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: "The Stay-alive process holds the queue source".
  An Overwrite-playlist completion becomes a lineage-guarded source update,
  and a Client with no owner snapshot refuses source-changing playlist saves.

## Impact

- `src/app/state/`: new `queue_owner.rs` (enum, newtypes, owner/origin
  accessors); `queue_scope.rs`, `app_struct.rs`, `construct.rs`,
  `types/playback.rs`, `types/events.rs`.
- `src/app/dispatch/`: `queue/mod.rs`, `queue/playlist_mutation.rs`,
  `run_loop/session.rs`, `session/player_event.rs`, plus mechanical
  `advance_queue_epoch` renames in `session/`, `actions/`, `run_loop/`,
  `input/`.
- `src/app/shell/run/mod.rs` (restore call), tests under `src/app/tests/`
  and `dispatch/actions/queue_state_tests.rs`.
- Docs: CONTEXT.md gains **Queue epoch**; invariant 13's "how the code
  maintains it" section is updated to the new names.
- No ctrl protocol, daemon, or persistence-format change.

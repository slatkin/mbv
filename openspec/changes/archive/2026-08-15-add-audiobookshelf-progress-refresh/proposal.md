## Why

Audiobookshelf listening progress can change on another device or app while
mbv is open. Today mbv only learns about that through an explicit REST
refresh, so browse and queue progress silently go stale until the user
navigates away and back. Audiobookshelf's Socket.IO channel pushes
`user_item_progress_updated` the moment any of the user's progress changes,
using the same API key already installed for REST, so mbv can close this gap
without adding a new credential or a polling loop.

## What Changes

- Add an Audiobookshelf Socket.IO client in the interactive bare-mode process
  only, connected/disconnected exactly when the Audiobookshelf Service
  transitions Ready/replaced/removed (mirrors `ws.rs`'s Emby lifecycle).
- Authenticate the socket with the existing installed API key via the `auth`
  client event; no new secret storage.
- Handle `user_item_progress_updated` by merging its `{id, data}` payload
  directly into cached episode/queue progress for the matching
  `(libraryItemId, episodeId)`, scoped to the current setup generation.
- Explicitly ignore every other Socket.IO event, in particular `stream_progress`
  (HLS transcode chunk-encode percentage), which is not listening progress.
- Never let a Socket.IO merge touch the actively Player-owned playback slot;
  that slot's progress stays driven exclusively by the existing REST
  `sync_playback_session_bounded` / `close_playback_session_bounded` lifecycle.
- No Local daemon, packaged `mbvd`, or ctrl protocol change — the daemon
  continues to have no Audiobookshelf Socket.IO involvement, matching how it
  already no-ops Emby's equivalent `UserDataChanged` event.

## Capabilities

### New Capabilities
- `audiobookshelf-progress-refresh`: Socket.IO connection lifecycle, auth,
  `user_item_progress_updated` handling, and the `stream_progress` exclusion,
  scoped to the interactive bare-mode process.

### Modified Capabilities
- `audiobookshelf-podcast-playback`: removes the "no Socket.IO connection
  SHALL be required or opened" boundary from bare-mode playback now that a
  Socket.IO connection exists for progress refresh, while keeping REST as the
  sole authority over the active owned session's own progress.
- `audiobookshelf-podcast-browsing`: the "Progress changes outside mbv ...
  live Socket.IO refresh is outside this capability" scenario becomes live
  refresh via the new capability instead of staying stale until REST reload.

## Impact

- New: an Audiobookshelf Socket.IO/Engine.IO v4 client module in
  `crates/mbv-core/src/`, built on the already-installed `tungstenite`
  dependency (no new crate), mirroring `ws.rs`'s background-thread/mpsc/
  reconnect-backoff shape.
- Changed: `src/app/` Audiobookshelf Service lifecycle call sites (Ready,
  replace, remove) gain socket connect/disconnect calls, mirroring
  `emby_service_actions.rs`.
- Changed: cached Audiobookshelf episode/queue progress gains a merge path
  fed by socket events, alongside the existing REST-fed paths.
- Unaffected: Local daemon, packaged `mbvd`, ctrl protocol, audiobook support.

# Proposal

## Why

In the Playlists sidebar (F4), the hint bar shows `[↵]play`, but Enter only loads the playlist into the queue. Playback does not start. To make this work, an earlier change added an `autostart: bool` flag to `PendingQueueAction::PlayItems`. That gave "play with play turned off" for the one F4 caller and replaced a named case with a flag that selects between two different code paths. The panel also has no shuffle or enqueue actions, although the Music library already has them. The open-playlist item rows also still use an obsolete row style: no zebra striping, a selected-row style that is not canonical, and names that wrap onto a second line.

## What Changes

- **BREAKING (behaviour):** In the Playlists sidebar, Enter replaces the queue with the playlist and **starts playback**. On a playlist row it starts at the first item. In the open playlist view it starts at the cursor item. A non-empty queue still shows the existing replace-queue confirmation first. The load-only behaviour is removed.
- New `s` (Shuffle) in both views: replace the queue with the whole playlist in random order and start playback. The queue source is `Shuffle`, not the playlist, so the shuffled order is never written back to the saved Emby playlist.
- New `a` (Enqueue): on a playlist row, append the whole playlist to the current queue. In the open view, append only the cursor item. Playback does not start. When the current queue is bound to another saved playlist, the appended items make that playlist dirty, which is how the user merges playlists.
- Both hint bars list the new keys.
- `PendingQueueAction` loses the `autostart` flag and becomes `PlayItems` + `ClearQueue`. The load-only branch of the pending-replacement executor is deleted. The owner's idle-load mechanism stays, because attached Emby Sessions and cast targets still use it directly.
- Open-playlist item rows use the same row presentation as the playlist list rows. Each row is one line, and long names are truncated. Odd rows get the playlist zebra stripe, based on the absolute index. The selected row uses the canonical selected-row background and foreground. The row number stays as a muted leading span.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `playlist-management`: Adds the sidebar's play, shuffle, and enqueue actions, the hint bar contents, and the open-view row presentation.
- `unified-playback-queue`: Two requirements use "loading a playlist without starting playback" as their example of a populate-only (idle) load. That user action no longer exists. The requirements are reworded around the remaining idle-load uses: attached Session and cast targets.
- `local-daemon-thin-client`: The "owner's accepted queue" requirement uses the same example. It is reworded the same way, and its two-Client load scenario no longer expects an idle queue.

## Impact

- `crates/mbv-components/src/playlists.rs`: Adds the key handling for `s` and `a`.
- `crates/mbv-ui-msg/src/shell.rs`: `PlaylistsActivate` becomes a playlist action request that carries the action.
- `src/app/shell/playlists.rs` and `src/app/dispatch/library/load.rs`: The shell handling for each action.
- `src/app/state/playback.rs`, `src/app/dispatch/queue/replacement.rs`, and `src/app/dispatch/queue/pending_playback.rs`: Remove the flag.
- The test builders of `PlayItems` in `src/app/input/confirm_keys/tests.rs`, `src/app/tests/queue/queue_op.rs`, and `src/app/dispatch/actions/tests/album_artist_playback.rs`.
- `crates/mbv-render/src/components/playlists.rs`: The open-view row painter and the hint strings.
- No protocol, persistence, or config changes.

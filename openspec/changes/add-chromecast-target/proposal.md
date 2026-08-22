## Why

mbv can hand playback to a remote Emby session, but an Emby session only plays Emby
library items. Feeds and Audiobookshelf have no path to a TV. Adding Google Cast
receivers as playback targets gives every mbv source a screen without making Emby the
gatekeeper.

## What Changes

- mbv discovers Google Cast receivers on the LAN and lists them in the F3 target panel
  alongside Emby sessions. The channel that produced a target determines how mbv controls
  it, so no capability probing is needed.
- Selecting a cast target attaches to it, the same way attaching to an Emby session works
  today. Playing a selection dispatches its items to the receiver's own media queue and
  the receiver owns them from there. mbv does not project, track, or reconcile the
  receiver's queue.
- mbv controls an attached receiver with standard cast transport: play, pause, stop, seek,
  next, previous, volume, and subtitle track selection. Now-playing state is read from the
  receiver's reported status.
- Media URLs are handed to the receiver directly. mbv does not proxy bytes and does not
  bind a listening socket. Emby items request a Chromecast device profile through the
  existing `PlaybackInfo` call so the server chooses direct-play or transcode; feed and
  Audiobookshelf-podcast sources use the URL their existing preparation already produces.
- Text subtitles are delivered as sidecar tracks the receiver renders; image-based
  subtitles are burned in server-side.
- **BREAKING for Audiobookshelf books only**: multi-file books are not castable. mbv
  reports them as uncastable rather than dispatching them, because a per-file position
  cannot be written back as a book position without corrupting resume.
- Progress is reported to providers while mbv is attached and polling. Quitting mbv leaves
  the receiver playing and stops reporting; on next launch mbv reattaches to a still-running
  session for control and display when `auto_reconnect` is enabled.
- The system-audio visualizer does not run while a cast target is attached, because no
  audio reaches the local PipeWire graph.

## Capabilities

### New Capabilities
- `cast-device-discovery`: Finding Google Cast receivers on the LAN, presenting them as
  playback targets beside Emby sessions, and identifying them durably across address
  changes.
- `cast-media-dispatch`: Deciding what media URL and subtitle tracks are sent to a
  receiver for a given queue item, per provider, and which items are uncastable.
- `cast-session-control`: Attaching to a receiver, dispatching items to it, controlling
  transport, presenting its reported state, reporting progress, and the session's
  lifecycle across mbv exit and restart.

### Modified Capabilities
- `system-audio-visualizer`: Capture is suppressed while a cast target is attached, in the
  same way it is already suppressed for an attached Emby session.

## Impact

- **New dependencies**: a Google Cast sender crate (`rust_cast`, synchronous, matching
  mbv-core's blocking style) and an mDNS browser (`mdns-sd`, pure Rust).
- **`crates/mbv-core`**: new cast client and discovery modules;
  `api_client_playlists.rs` (`get_playback_info` accepts a device profile);
  `config_parse.rs`/`config_save.rs`/`config_state.rs` (cast target persistence).
- **`src/app`**: the F3 target panel, attachment state beside `connected_session_id`,
  input gating in `input_resolver.rs`, `session_connect.rs` (reattach), and
  `visualizer.rs` (capture gate).
- **Deliberately not affected**: the playback queue engine, `PreparedSource`, the
  `Player`/`PlayerProxy` local-vs-remote structure, `RemoteQueueProjection`, the ctrl
  protocol, and mbvd. A cast target is not a player target and does not create one.
- **No delta needed for `queue-scope-remote-handoff`**: a cast target has no separate
  remote queue, so `playback_target_queue_scope()` resolves to Local and its existing
  no-op branch already describes the behaviour.

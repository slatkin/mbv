## Context

See `proposal.md` — Why.

The structure this design attaches to:

- `src/app/input_resolver.rs:119` — `has_remote_session: self.connected_session_id.is_some()
  || self.player.is_remote()`. The existing gate that makes transport keys work for a
  target mbv does not itself play.
- `crates/mbv-core/src/api_client_sessions.rs` — the Emby remote-control client
  (`get_sessions_with_active_within` filtering on `SupportsRemoteControl`, `session_play`,
  `session_play_items`, `session_transport`, `session_seek`, `session_set_volume`,
  `session_set_subtitle_index`). The shape a cast target imitates.
- `crates/mbv-core/src/api_client_playlists.rs:2` — `get_playback_info(item_id)` calls
  `/Items/{id}/PlaybackInfo` and already parses external subtitle URLs.
- `crates/mbv-core/src/config_state.rs` + `Config.auto_reconnect` +
  `src/app/session_connect.rs` (`try_auto_reconnect`) — at-exit target persistence and
  launch-time restore.
- `src/app/visualizer.rs:37` — `visualizer_should_run()`, a boolean gate that already
  excludes attached Emby sessions via `connected_session_id.is_none()`.

An earlier revision of this change treated a cast receiver as a second *output* of the
local `Player`, forking `install_active_projection`. That was abandoned: review found that
`cmd_submit_queue` (`crates/mbv-core/src/player_run_commands.rs:433`) only routes through
`PreparedSource` when a file is already active or the queue holds Audiobookshelf items.
Ordinary Emby and feed queues take a separate branch that builds URLs with
`mpv_url_for_queue_item` and loads the entire queue into mpv's native playlist
(`:474-485`), where **mpv**, not mbv, performs advancement. There is no single
renderer-neutral seam to fork, and creating one would have meant refactoring working local
playback before any casting existed.

## Goals / Non-Goals

**Goals:**

- Give every provider a path to a TV without changing how local playback works.
- Reuse the attached-target pattern that already works for Emby sessions.
- Touch no code that the local playback path depends on.

**Non-Goals:**

- Projecting, tracking, or synchronising mbv's queue onto a receiver. Only mbv-to-mbv
  targets carry that expectation.
- mbv transcoding anything. Where a rendition is needed, the provider produces it.
- mbv acting as a media server or binding a listening socket.
- Casting Audiobookshelf books.
- mbvd changes. A cast target is not a player target, so nothing crosses the daemon
  boundary and none of the `AGENTS.md` daemon rules are engaged.

## Decisions

### Cast is an attached target, not a player output

Alternatives: a second output of the local `Player`; a third `PlayerProxyInner` arm.

The output approach was implemented on paper and abandoned for the reason in Context — the
seam it assumed does not exist for the providers this feature is for. The attached-target
approach is the shape already proven by the Emby session path, and it makes the queue
question disappear rather than answering it.

The original objection to this shape was that the Emby session path works only because
*Emby* owns the queue, while a receiver owns nothing. That objection is answered by giving
the receiver ownership: a cast receiver has its own media queue and its own advancement,
so dispatching to it is the same act as dispatching to an Emby session.

Consequence: nothing in `player_run_commands.rs`, `player_sources.rs`, `player_proxy.rs`,
or `player_runtime_controller.rs` changes. `player-target-locality` needs no delta, because
no player target is created.

### The receiver owns its queue; mbv dispatches and forgets

Alternative: project a bounded window of mbv's queue onto the receiver and reconcile it.

Windowing requires tracking correspondence between mbv occurrences and receiver entries,
detecting divergence, and reissuing on every edit — the machinery `RemoteQueueProjection`
provides for mbv-to-mbv targets. Applying it to a receiver buys synchronised reordering, a
capability no cast sender offers and none of this feature's uses need. Dispatch-and-forget
removes an entire capability's worth of state.

Multi-item dispatch is preserved: a played selection goes to the receiver's queue in one
act and the receiver advances through it. "Bypass mbv's queue" constrains tracking, not
item count.

### The receiver fetches media directly; mbv never proxies

Alternative: a local HTTP origin proxying to providers with the right headers, which would
uniformly solve header-based credentials and local files.

Rejected on scope: mbv is a client, not a streaming server. Consequence accepted: any
source whose credential cannot be expressed in a URL is uncastable, and local files are out
of scope.

### Emby negotiates the rendition; other providers do not

`get_playback_info` already calls `/Items/{id}/PlaybackInfo`; adding a Chromecast device
profile makes the server choose direct-play or transcode and return the URL. Emby holds the
incompatible containers and is the only provider that transcodes on demand. Feed enclosures
and Audiobookshelf podcast audio are web-shaped media that direct-play.

### Text subtitles as sidecar tracks, image subtitles burned in

Emby converts text subtitles to WebVTT on request and `get_playback_info` already parses
the URLs. Sidecar costs the server nothing and stays toggleable. Image subtitles (PGS,
VOBSUB) have no sidecar representation and require a burned-in rendition, so the device
profile is built per item rather than once.

**Superseded for v1, text subtitles only** (see Risks below): the sidecar half of this
decision assumed a cast-protocol track primitive that `rust_cast` 0.21 does not expose. Text
subtitles are dropped from v1 rather than built on a hand-rolled protocol path. The
image-subtitle burn-in half is unaffected — it is a device-profile decision the server makes,
independent of cast-protocol track support — so it stays in scope.

### Audiobookshelf books are excluded

Alternative: dispatch a book's files as separate receiver entries and display per-file
position.

A book's position is defined across its whole timeline. A receiver reports position within
the file it is playing, and without the merged timeline mbv would either display a position
that means nothing to the user or write a per-file offset back as a book position and
corrupt resume. Excluding books is one clear message instead of a subtly wrong number.
Podcast episodes are single-file and unaffected.

### Discovery is a second channel beside `/Sessions`

Alternative: static device configuration, avoiding a new dependency.

Emby's `/Sessions` already acts as discovery for Emby and daemon targets, but only because
every such target is an Emby client. Receivers are not. Two independent scans is a benefit
rather than a cost: the channel that produced a target determines how to control it, so no
probing is needed. Static config was rejected because DHCP will move the devices; discovery
persists the receiver's advertised identifier, not its address.

### Exit orphans the session; reattach restores control only

Cast's own model is that the receiver owns the session and senders come and go, so stopping
the TV because a terminal closed is wrong. Reattach is governed by the existing
`Config.auto_reconnect` rather than a new setting.

Because mbv no longer tracks a queue, reattach is simpler than in the abandoned revision:
it restores control and display from reported status and identifies the playing item if it
can. It never resumes or re-dispatches.

### Progress reporting is attached-only

mbv reports progress for an item it dispatched while it is attached and receiving status.
It cannot report for a receiver it is not talking to, so progress stops when mbv exits.
Accepted consequence: an item finished after mbv quits is not marked played. This follows
from orphaning, and the alternative — a background reporter — is a daemon feature and out
of scope.

### Visualizer suppression follows the attached-session precedent

`visualizer_should_run()` is a boolean gate, not a panel system: an attached Emby session
suppresses capture by failing `connected_session_id.is_none()`. An attached cast target adds
one more clause. Matching that precedent exactly means the visualizer panel stays selected
and simply shows nothing, as it already does for an attached session — no forced panel
switch, no new state.

### Dependencies

`rust_cast` for the sender protocol — synchronous, matching mbv-core's blocking style
(`ureq`, blocking `get_sessions`) and avoiding tokio. `mdns-sd` for discovery — pure Rust,
no C toolchain dependency. Both are confirmed only as far as their published descriptions;
tasks 1.2–1.4 verify the specific operations against a real device before anything depends
on them.

## Risks / Trade-offs

- **Audiobookshelf podcast episodes may not be castable at all** → RESOLVED by task 1.1
  against the live server (`192.168.0.6:13378`): a real downloaded-episode `contentUrl`
  (`/api/items/{id}/file/{fileId}`) returned `HTTP 200` when fetched with `?token=<api key>`
  appended and no `Authorization` header, and `HTTP 401` with neither. ABS media URLs CAN
  carry their credential in the URL. ABS podcast casting stays in scope; task 4.4 is NOT
  dropped.
- **rust_cast 0.21's public API cannot express Cast media tracks or a queue-jump message**
  → Confirmed by reading `rust_cast` 0.21.0's full source (`channels/media.rs`,
  `cast/proxies.rs`): neither the friendly `Media`/`QueueItem` wrappers nor their wire-level
  `proxies::media` counterparts have a `tracks`/`activeTrackIds`-carrying field usable from
  outside the crate, and there is no `QUEUE_UPDATE`/`EDIT_TRACKS_INFO` message type or a way
  to send a hand-built `CastMessage` (the crate never exposes the connected `MessageManager`
  to callers). Consequences for this stage's `cast_client.rs`:
  - Subtitle-track selection (a proposal.md transport requirement) is NOT implemented; there
    is no wire primitive to build it on. Before task 4.6 (sidecar subtitle descriptors) or
    task 5.5 (subtitle transport key) can land, someone needs to pick one of: (a) hand-roll
    the TLS/`MessageManager` bootstrap directly against `rustls`/`rustls-native-certs`
    (already transitive deps of `rust_cast`) to get a raw send path, (b) find/vendor a more
    complete Cast crate, or (c) drop subtitle-track support from v1 scope. This is a decision
    for whoever owns the media-dispatch/attach stages, not resolved here.
  - `skip_next`/`skip_previous` are implemented instead by reloading the last-dispatched
    `MediaQueue` via `QUEUE_LOAD` with a shifted `start_index`, rather than a native
    queue-jump message. This is a real, tested protocol action (confirmed against a live
    Shield) but is a deliberate substitution, not the SDK's own `queueJumpToItem`.
  - **Decision (user, post-spike): (c) — subtitle-track support is dropped from v1.** No
    sidecar text-subtitle tracks are sent to a cast receiver, and there is no subtitle
    transport command for cast targets. Image-based subtitles are unaffected: burn-in is a
    device-profile choice the Emby server makes (task 4.1/4.2), not a cast-protocol track,
    so it does not depend on `rust_cast`'s track support and stays in scope. Upgrade path if
    this is revisited: hand-roll a raw `MessageManager` send against the already-transitive
    `rustls`/`rustls-native-certs` deps, or vendor a fuller Cast crate.
- **The Cast heartbeat channel requires an explicit keep-alive pump** → Confirmed against a
  live Shield: a connected sender that only issues `GET_STATUS` polls (no heartbeat replies)
  was disconnected by the receiver (`get_status` started failing with "failed to fill whole
  buffer" / "Broken pipe") within roughly 90 seconds. Having the poller call
  `CastClient::keep_alive()` (an unsolicited `heartbeat.pong()`) once per poll tick at the
  same 5-10s cadence already targeted below kept the connection alive through a full 90s
  test with zero failures. Task 6.1's status-poll loop must call `keep_alive()` on every
  tick; it is not optional plumbing.
- **Device profile tuning is empirical** → Too permissive and the receiver errors on
  playback; too strict and items transcode that would have direct-played. Start strict,
  loosen against observed Shield behaviour, keep the profile in one place. Confirmed
  starting point (task 1.4): a Shield Android TV direct-played an Emby `video/mp4`
  (H.264/AAC) direct-play URL with no server-side transcoding, and correctly reported
  `duration`/`currentTime` immediately after load. Deeper codec/container coverage (HEVC,
  AC3, image-subtitle burn-in) was not exhaustively probed in the spike; SHIELD Android TV
  receivers run Google's ExoPlayer-based default media receiver and are known to support a
  broad codec set, but task 4.1's device profile should still be tuned against real
  failures rather than assumed.
- **Identifying what the receiver is playing is heuristic** → Progress reporting depends on
  matching reported media back to a dispatched item. If matching fails, the specs require
  reporting nothing rather than reporting against the wrong item.
- **Reported status is the only source of truth for display** → A receiver that reports
  slowly or sparsely will make the now-playing panel feel less live than local playback.
  Extrapolation covers the gap; a receiver that misreports rate will drift until the next
  reconcile.
- **Two scans lengthen panel open** → Run the mDNS browse concurrently with the `/Sessions`
  fetch and render Emby targets as they arrive, per the discovery spec. Confirmed feasible
  with `mdns-sd` 0.21 (`ServiceDaemon::browse` returns a channel of `ServiceEvent`s
  immediately; a bounded `recv_timeout` loop collects results without blocking the Emby
  session fetch).
- **Real-device confirmation for tasks 1.2/1.3 was protocol-level plus one direct visual
  check** → mbv's own spike process cannot see the TV screen, so playback correctness was
  read from the receiver's own `MEDIA_STATUS` responses (`playerState: Playing`,
  `currentTime` advancing across polls, `duration` populated, and — for the queue case — the
  `media.contentId`/`currentItemId` changing to the second dispatched item's URL with
  `currentTime` resetting near zero) rather than watched directly. The user separately
  looked at the TV during this run and confirmed the cast picture/audio actually played,
  matching the protocol-level signal.

## Migration Plan

Additive and isolated. No existing playback path changes: no cast target is attached unless
the user selects one, and the local player is untouched by attachment. The single spec delta
adds a clause to a gate that already has three.

Rollback is removing cast targets from the panel; nothing else has been modified to
accommodate them.

## Open Questions

- **Status poll interval.** Somewhere in the 5–10s range, tuned against observed drift once
  extrapolation runs. Does not change the specs or the task breakdown.
- **Whether uncastable items should be filtered out of a dispatched selection silently when
  the selection is large.** The specs require naming them; if that proves noisy in practice
  it is a presentation change, not a behavioural one.

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

- **Audiobookshelf podcast episodes may not be castable at all** → If ABS media URLs cannot
  carry a credential without a request header, ABS drops out entirely and only the
  "uncastable item" path applies to it. Verify against the live server first (task 1.1);
  this affects only the ABS task, not the feed or Emby paths.
- **Device profile tuning is empirical** → Too permissive and the receiver errors on
  playback; too strict and items transcode that would have direct-played. Start strict,
  loosen against observed Shield behaviour, keep the profile in one place.
- **Identifying what the receiver is playing is heuristic** → Progress reporting depends on
  matching reported media back to a dispatched item. If matching fails, the specs require
  reporting nothing rather than reporting against the wrong item.
- **Reported status is the only source of truth for display** → A receiver that reports
  slowly or sparsely will make the now-playing panel feel less live than local playback.
  Extrapolation covers the gap; a receiver that misreports rate will drift until the next
  reconcile.
- **Two scans lengthen panel open** → Run the mDNS browse concurrently with the `/Sessions`
  fetch and render Emby targets as they arrive, per the discovery spec.

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

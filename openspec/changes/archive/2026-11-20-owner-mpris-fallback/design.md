# Design

## Context

MPRIS today is a Client-side surface: `mbv_desktop::mpris::start` runs in every local Client
process (`src/app/state/construct/remote.rs::start_mpris`), claims `org.mpris.MediaPlayer2.mbv`
with the zbus default flags (`AllowReplacement | ReplaceExisting | DoNotQueue`), and is re-pointed
by `rebind_mpris_to_current_player` (`src/app/dispatch/session/switch.rs`) on every change of
attachment. When the last Client exits, the name leaves the bus even while a Stay-alive Owner
plays. With several Clients attached, each new Client takes the name and the older one is not
queued, so the name can also leave the bus while a Client is still attached.

The local Owner process (`src/local_daemon.rs`, `mbv --__local-daemon`) holds the canonical
`PlayerStatus` and the transport sender; both arrive in its `on_player_ready` hook
(`mbv_daemon::DaemonPlayerHandle`). The status is the presented status of the active output: Cast
and Emby Session outputs write theirs into the same `Arc` (`crates/mbv-daemon/src/cast.rs`).

A Client that moves to a remote Player owner keeps its link to the local Owner as the suspended
local session and drains it every tick (`attach_remote_owner`, `switch.rs`), so the Client knows
the local Owner's playback status while it controls another Player owner.

## Goals / Non-Goals

**Goals:**

- `org.mpris.MediaPlayer2.mbv` is on the session bus for as long as the local Owner runs, whatever
  Clients attach or exit, with no handoff.
- A Client that controls a remote Player owner publishes it, and local playback wins over it.
- The packaged `mbvd` keeps no MPRIS, with no hook signature change on the packaged entry.

**Non-Goals:**

- Any new setting, protocol capability, or ctrl message.
- Changes to MPRIS metadata, art keys, or transport resolution.
- Audio-window or tray behavior changes.

## Decisions

### D1 — Two bus names, one for each publisher

The MPRIS Bus Name Policy lets a player run several instances, each additional instance under its
own name with a dot and a unique identifier, such as `org.mpris.MediaPlayer2.vlc.instance7389`
(<https://specifications.freedesktop.org/mpris-spec/latest/>, "Bus Name Policy"). The local Owner
holds `org.mpris.MediaPlayer2.mbv`. A Client that controls a remote Player owner holds
`org.mpris.MediaPlayer2.mbv.instance<pid>`, where `<pid>` is its process id (the identifier must
not begin with a digit, which `instance` satisfies). The two publishers never contend for a name,
so no process polls for a free name and no handoff can leave a gap.

Alternative considered: one shared name, held by the Owner while no Client holds it (the first
draft of this change). Rejected: the claim gate either missed an attached Client that had lost the
name to a newer Client, or needed a name-ownership poll in the Owner plus claim flags in every
Client, for a result the two-name rule gets without any coordination.

### D2 — The Owner publisher holds the fixed name for its lifetime

`mbv_desktop::mpris::start_owner` serves `MediaPlayer2`/`MediaPlayer2Player` and claims
`org.mpris.MediaPlayer2.mbv` with `RequestNameFlags::ReplaceExisting | DoNotQueue`, without
`AllowReplacement` (zbus 5.19.0 `Connection::request_name_with_flags`). No later claim can take the
name from it. `ReplaceExisting` takes the name from a Client of an older build that still holds it
at upgrade time; such a Client claimed with the zbus defaults, which include `AllowReplacement`.

Source: the `DaemonPlayerHandle`'s `Arc<Mutex<PlayerStatus>>` as the status, its `transport_tx` as
the command sender, no disconnect signal (the Owner cannot be disconnected from itself), and
`image_disk_cache_path` as the art resolver, so the `queue-artwork-title-overlay` art-key rule
holds for this publisher too. The source never changes, so the Owner publisher has no rebind.
While the Owner plays nothing it publishes `Stopped`, as the Client publisher does today.

### D3 — The Client publishing rule is a pure function

A Client publishes when both hold:

- its attached endpoint is not `DaemonEndpoint::Local` (Direct remote control or a Library route),
  and
- the local Owner's status is not `Playing`, that is not `active && !paused`.

The local status comes from the suspended local link (`App::suspended_local`). With no suspended
link, or a disconnected one, the local status counts as not `Playing`: the Client cannot observe
local playback, and the Owner still publishes its own name. A paused local Owner yields to remote
playback; only a playing one wins.

The rule is a pure function over `(endpoint is local, local status)` in `mbv-desktop`, unit-tested
without D-Bus (test builds never claim the real bus, issue #757's rule).

### D4 — The Client publisher claims and releases its instance name each tick

`mbv_desktop::mpris::start` serves its interfaces on a session connection without requesting any
name. `MprisSource` gains the rule closure beside the status, sender, and disconnect closures;
`rebind` sets it with the others. The existing 500 ms poll evaluates the rule each tick: when it
turns true, the poll calls `request_name_with_flags` for the instance name; when it turns false,
the poll calls `Connection::release_name`. While the rule is false, the poll publishes no property
signals. Interfaces are live before every claim, so a consumer never sees the name without them.

`rebind_mpris_to_current_player` already runs on every change of attachment
(`switch_to_direct_remote`, `switch_to_library_route`, `restore_local_mode`, connect, and daemon
restart), so the rule closure always reads the current endpoint and the current suspended link.

### D5 — Spawned from the Local entry's existing hook; packaged entry untouched

The Owner publisher starts in `src/local_daemon.rs`'s `on_player_ready`, which already receives
the status and the transport sender. `mbvd`'s entry passes a no-op hook
(`crates/mbvd/src/main.rs`), so its exclusion needs no hook change — the same structural rule the
tray uses (`local-daemon-tray`'s "The packaged system daemon has no tray"). A Client attached to
`mbvd` is a Client that controls a remote Player owner and publishes under D3.

### D6 — Failures stay silent and non-fatal

A missing session bus or a failed connection build is logged through `tracing` and parks the
publisher thread. A failed claim or release (for example `Err(zbus::Error::NameTaken)` for the
Owner's name while another process holds it without `AllowReplacement`) is logged, and the
publisher tries again on the next tick. No failure warns on a terminal or affects playback — the
"missing tray is not an error" rule from `local-daemon-tray`, restated in the spec for MPRIS. Test
builds keep both publishers compiled out (`#[cfg(not(test))]`), the same guard the Client
publisher uses today.

## Risks / Trade-offs

- [While the Owner is idle and a Client publishes remote playback, desktop widgets list two mbv
  players] → The Owner shows `Stopped` and the Client shows `Playing`; media-key routers
  (playerctld, GNOME, KDE) prefer a playing player. While local playback plays, the Client holds
  no name, so local playback is the only mbv player that plays.
- [A local Owner that is paused shows next to remote playback] → Accepted: only a playing local
  Owner wins (D3).
- [`mpris.rs` grows past the file-size limit] → Split along the existing seam: keep interfaces and
  the publisher loop in `mpris.rs`, move the Owner entry point into `mpris/owner.rs` and the
  publishing rule into `mpris/rule.rs`, with tests in `mpris/tests.rs`, per the
  one-file-per-module rule.
- [The Owner process gains a D-Bus connection it did not have before] → It is one user-session
  connection behind the same silent-failure rule as the tray; headless hosts are unaffected.

## Migration Plan

One release; no state or protocol to migrate. A Client of an older build that still holds
`org.mpris.MediaPlayer2.mbv` loses it to the new Owner (D2); the build-mismatch restart at
startup replaces such a pairing anyway. Rollback is reverting the change: the Client publisher
returns to claiming the fixed name.

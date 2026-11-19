# Design

## Context

MPRIS today is a Client-side surface: `mbv_desktop::mpris::start` runs in every local Client
process (`src/app/state/construct/remote.rs::start_mpris`), reads status through the Client's
player link, and rebinds that link on every owner switch. The local Owner process
(`src/local_daemon.rs`, `mbv --__local-daemon`) holds the canonical `PlayerStatus`, the transport
sender, and the `AttachedClientsCell` — all three already arrive in its `on_player_ready` hook
(`mbv_daemon::DaemonPlayerHandle`). The zbus 5.19.0 already in the workspace supplies every name
handoff primitive this change needs; no dependency changes.

## Goals / Non-Goals

**Goals:**

- `org.mpris.MediaPlayer2.mbv` stays on the session bus whenever the local Owner runs, whatever
  Client windows are open or closed, with no publisher gap during handoffs.
- A starting Client always wins the name without protocol or retry machinery on its side.
- The packaged `mbvd` keeps no MPRIS, with no hook signature change on the packaged entry.

**Non-Goals:**

- Moving the Client's MPRIS anywhere. A Client attached to the local Owner, to a network daemon,
  or to `mbvd` publishes exactly as today, through the same rebind machinery.
- Any new setting, protocol capability, or ctrl message.
- Audio-window or tray behavior changes.

## Decisions

### D1 — Owner fallback claims the name only when it is free

The fallback claims with `RequestNameFlags::AllowReplacement | RequestNameFlags::DoNotQueue`
(zbus 5.19.0, `Connection::request_name_with_flags`) and never with `ReplaceExisting`. So the
fallback takes the name only when free; it can never displace a live publisher — a Client
attached to a network `mbvd` or any other current owner is safe.

A starting Client needs no change: `connection::Builder::name` already requests with the
`BitFlags<RequestNameFlags>` default, which is `AllowReplacement | ReplaceExisting | DoNotQueue`
(zbus 5.19.0 `fdo::dbus.rs`), so the Client's claim replaces the fallback the moment it starts.
Handoff has no publisher gap: the fallback keeps serving until `NameLost`, and the Client's
interfaces are live before it claims.

Alternative considered: the Owner always publishing and Clients attached to the local Owner not
publishing at all. Rejected: it would change the Client's behavior for every session and still
needs the same claim flags for the direct-remote case, so it buys nothing over the fallback.

### D2 — The fallback reads the attached-Client cell, polled on the existing tick

The claim loop runs on the fallback's own poll thread (500 ms, the same cadence the Client's MPRIS
poll uses). Each tick it loads `AttachedClientsCell`: when the value is `None` and the fallback
does not hold the name, it claims; when a Client is attached (`Unpinned`/`Pinned`), it does
nothing — the Client's own claim does the replacing. While holding, the loop publishes exactly as
the Client publisher does. The cell is the same one the ctrl registry already refreshes on every
connect and disconnect and the Tray already reads; no new notification machinery.

The cell counts only local-transport ctrl connections to this Owner. A Client publishing a
remote target while the home Owner is idle is therefore not counted, and per D1 the fallback
cannot take the name from it anyway; the two rules compose into the specified behavior.

Alternative considered: watching `NameLost`/`NameAcquired` signals only. Rejected: signal-only
watching misses the initial state (a Client already attached when the Owner starts) and adds
stream plumbing for no observable gain over the polled cell.

### D3 — Claim policy is a pure state machine

The decision core is a pure function over `(state, cell value, claim outcome)`:
`Waiting --(cell is None)--> claim; claim PrimaryOwner --> Holding; claim Exists --> Waiting;
Holding --NameLost--> Waiting`. Holding never releases eagerly: eager release would open a
no-publisher window while a starting Client has not yet claimed. The state machine lives beside
the existing `effective_status`/`resolve_art_url` pure helpers and is unit-tested without any
D-Bus (test builds never claim the real bus, issue #757's rule).

### D4 — The fallback serves the same interfaces with the Owner as the source

The fallback reuses the existing `MediaPlayer2`/`MediaPlayer2Player` interfaces and
`poll_status` publisher unchanged in behavior: status source = the `DaemonPlayerHandle`'s
`Arc<Mutex<PlayerStatus>>` (the Owner's own state, so no rebind is ever needed), command sender =
its `transport_tx`, disconnect signal = `None` (the Owner cannot be disconnected from itself),
art resolution = the same `image_disk_cache_path` lookup the Client publisher uses, satisfying
the existing `queue-artwork-title-overlay` art-key requirement for this publisher too.

The Client-side `start` stays exactly as it is. The shared parts (interface structs, publisher
loop, metadata builder) are already in `mbv-desktop/src/mpris.rs`; the fallback entry is a sibling
function in the same module, so no new crate or public API beyond one
`mbv_desktop::mpris::start_owner_fallback` entry point.

### D5 — Spawned from the Local entry's existing hook; packaged entry untouched

The fallback starts in `src/local_daemon.rs`'s `on_player_ready`, which already receives the
status, the transport sender, and the attached-Client cell. `mbvd`'s entry never runs this code
path, so its exclusion needs no hook change — the same structural trick the tray uses
(`local-daemon-tray`'s "The packaged system daemon has no tray").

### D6 — Failures stay silent and non-fatal

A missing session bus, a failed claim, or a poisoned status lock is logged through `tracing`
and the loop continues or parks, never warns on the terminal, and never affects playback — the
"missing tray is not an error" rule from `local-daemon-tray`, restated in the spec for MPRIS.
Test builds keep the whole fallback compiled out (`#[cfg(not(test))]`), the same guard the
Client publisher uses.

## Risks / Trade-offs

- [Two publishers could race while a Client starts] → The flags make the race safe by
  construction: the fallback claims only a free name, and a Client claim replaces a fallback
  holder. No ordering between the processes matters.
- [`AttachedClientsCell` counts only local ctrl connections, so the fallback may try to claim
  while a direct-remote Client holds the name] → The D1 claim fails harmlessly (`Exists`, no
  queueing) and retries on the next tick after that Client exits.
- [`mpris.rs` grows past the file-size limit with the fallback added] → Split along the existing
  seam if needed: keep interfaces and the publisher in `mpris.rs`, move the fallback entry and
  its claim state machine into `mpris/fallback.rs` with tests in `mpris/tests.rs`, per the
  one-file-per-module rule.
- [The Owner process gains a D-Bus connection it did not have before] → It is one user-session
  connection behind the same silent-failure rule as the tray; headless hosts are unaffected.

## Migration Plan

One release; no state or protocol to migrate. Rollback is reverting the spawn call — the Client
publisher is untouched, so pre-change behavior returns intact.

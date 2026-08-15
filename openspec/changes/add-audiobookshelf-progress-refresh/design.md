## Context

Audiobookshelf's server pins `socket.io@^4.5.4` (Engine.IO v4 / Socket.IO v4
framing). The `auth` client event takes the same API-key Bearer token already
used for REST. `user_item_progress_updated` carries `{id, data}` where `data`
is a full Media Progress object matching the shape mbv already decodes via
`AudiobookshelfProgress` — the event is self-sufficient, no follow-up REST
call needed. `stream_progress` is a separate, unrelated event reporting HLS
transcode chunk-encode completion.

mbv already has one Socket/WebSocket client (`crates/mbv-core/src/ws.rs`,
built on the already-installed `tungstenite` dependency) for Emby's
proprietary WebSocket protocol, connected from the interactive process
(`src/app/emby_service_actions.rs`, `app_struct.rs`) and, separately, from
Local daemon (`daemon_run.rs`, `daemon_reconciliation.rs`) because Emby's
socket also carries remote-control commands that daemon-owned playback must
receive. mbv already has a browse-refresh reaction to a socket-pushed event:
`WsEvent::UserDataChanged` triggers `self.fetch_home()`. `daemon_ws.rs`
already no-ops that same event (`WsEvent::UserDataChanged => {}`) since the
daemon has no browse UI. See `proposal.md` - Why for the user-facing gap this
closes.

## Goals / Non-Goals

**Goals:**
- Push-driven browse/queue progress refresh for Audiobookshelf, replacing
  "stale until next REST load" with near-real-time updates.
- Reuse the existing API key; no new credential storage.
- Reuse the existing sync-threaded WebSocket architecture; no new async
  runtime or long-lived dependency.

**Non-Goals:**
- No Audiobookshelf remote-control command handling (unlike Emby's ws
  channel, this milestone handles exactly one event class).
- No Local daemon or packaged `mbvd` involvement — they render no
  Audiobookshelf browse UI and have no use for this event.
- No ctrl protocol change.
- No change to how the actively owned playback session reports its own
  progress (REST sync/close stays exactly as milestone 3/4 left it).

## Decisions

**Hand-roll a minimal Engine.IO v4 / Socket.IO v4 client on `tungstenite`,
rather than add a `socketio`/`rust_socketio` crate.** The wire framing needed
is small and fixed for protocol version 4: an Engine.IO packet-type prefix
(open/ping/pong/message), and, inside a message packet, a Socket.IO
packet-type prefix (connect-ack/event) wrapping a JSON array. Connecting
directly with `transport=websocket` skips the HTTP long-polling handshake
entirely. `rust_socketio` and similar crates pull in their own async runtime
(tokio) and HTTP client, which would sit awkwardly next to mbv's existing
sync-threaded, `mpsc`-based connection model and duplicate machinery
`ws.rs` already has proven in production. Alternative considered: add
`rust_socketio` for a "standard" client — rejected for the runtime mismatch
and because the actual protocol surface needed (open/ping/pong, one client
emit, one server event) is small enough that a minimal implementation is
less code and less risk than wiring a second async runtime into a sync
application.

**Mirror `ws.rs`'s shape** (background thread, `mpsc` outbound channel,
exponential backoff with jitter capped at 60s, `WsSender`/event-channel
pattern) for the new client rather than inventing a different concurrency
model. Alternative considered: share a single generic "socket client" module
between Emby and Audiobookshelf — rejected because the two protocols'
framing is different enough (Emby's is a flat JSON envelope; Audiobookshelf's
is Engine.IO/Socket.IO framed) that a shared abstraction would mostly be
indirection around two thin, protocol-specific parsers; the concurrency
*shape* is what's worth copying, not a shared implementation.

**Place the connection only in the interactive bare-mode process**, hooked to
Audiobookshelf Service Ready/replace/remove exactly where `ws_send_tx` is
today in `emby_service_actions.rs`. Alternative considered: also connect from
Local daemon, for symmetry with Emby — rejected because daemon renders no
Audiobookshelf browse/catalog UI (confirmed by `daemon_ws.rs` already
no-op'ing Emby's equivalent event) and this milestone adds no remote-control
event for the daemon to act on; adding an unused daemon-side connection would
be a live outbound connection with no consumer.

**Merge the event's payload directly into cached progress** rather than
triggering a full refetch (the pattern Emby's `UserDataChanged` uses).
Audiobookshelf's event already carries the complete changed `MediaProgress`
object, unlike Emby's bare notification; a refetch would be strictly more
expensive for no additional correctness.

**Exclude the actively Player-owned slot from Socket.IO merges**, matched by
provider-qualified identity at merge time (not at event-receipt time) against
the Player owner's current active slot. This keeps REST as the single writer
of the active session's own progress, avoiding a race between an in-flight
REST sync response and any echo of mbv's own write arriving back over the
socket.

## Risks / Trade-offs

[A hand-rolled Engine.IO/Socket.IO client could drift from a future protocol
version] → Mitigated by targeting only the fixed, currently-pinned protocol
version 4 surface (open/ping/pong/message, connect-ack/event); a server-side
major-version bump is not observed in Audiobookshelf's pinned
`socket.io@^4.5.4` dependency and would need its own follow-up change, the
same exposure any hand-rolled protocol client carries.

[A second always-on background connection per Audiobookshelf Service
lifecycle doubles the reconnect-loop pattern already run for Emby] →
Mitigated by reusing the identical, already-proven thread/backoff shape from
`ws.rs` rather than introducing a new concurrency model to reason about.

[Merging at the wrong moment during a slot transition could apply a foreign
event to a slot that is about to become active, or skip a merge for a slot
that just became inactive] → Mitigated by evaluating "is this the active
slot" against current Player-owner state at merge time rather than caching
that decision when the event is received.

## Migration Plan

Purely additive: a new module, two new call sites in the existing
Audiobookshelf Service lifecycle handlers, and a new merge path alongside the
existing REST-fed ones. No persisted data format changes. Rollback is
reverting the change; REST-only progress display continues to work exactly
as it does today since no existing REST path is altered.

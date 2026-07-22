# mbv Domain Glossary

mbv is a terminal (TUI) client for Emby. This glossary covers the domain vocabulary specific to mbv, grouped by area.

## Playback

The subsystem that drives mpv, tracks play position, and hands control of playback back and forth between local and remote players.

### Language

**Session**:
A running playback of a queue of media items (one item or many), owning one mpv instance for as long as that queue plays.
_Note_: currently implemented as two separate, non-unified state machines — `SingleSession` and `QueueSession` — which duplicate almost all of their fields and logic (intro-skip markers, resume position, next-up handling, pause state, etc.), differing only in whether there's one item or an ordered list with a cursor. This is one domain concept split by implementation history (single-item playback existed first; playlist support was added as a parallel path rather than a generalization), not two domain concepts. Treat "Session" as a single term when talking about the domain; the two-struct split is an implementation debt, not a modeling decision.

**PlaybackSession**:
The target implementation shape for **Session**: one running playback authority over a **PlaybackQueue**, backed by an mpv adapter and responsible for reporting progress to Emby. A standalone item is a one-slot queue, not a separate session kind.
_Avoid_: preserving separate single-item and queue session concepts in the domain model. The current `SingleSession`/`QueueSession` split is implementation history to collapse.

**PlaybackQueue**:
The core model for ordered queue slots, slot identity, active playback slot, queue revision, cursor-independent queue mutations, and progress merge/protection rules. It does not own mpv process lifecycle, network retry scheduling, rendering, or client-specific queue cursor state.
_Avoid_: putting UI focus/scroll/cursor concerns or mpv adapter mechanics inside the queue model.

**Remote slot state**:
A derived display classification — recomputed on demand, never stored — of which kind of control relationship the app currently has to a player: none, attached to another session, directly remote, or acting as a local daemon.
_Avoid_: implying it's a field that gets set, or using it as authority for what a user action is allowed to disconnect; it's computed fresh from other state each time it's read.

**Thin client**:
A TUI instance whose authoritative playback state comes from a daemon-backed player/queue instead of a locally-owned player. This describes the control model, not where the daemon runs.
_Avoid_: treating this as synonymous with **Local daemon** — a thin client can target either a same-machine daemon or a remote daemon.

**Suspended local session**:
The local player and its event channels, parked in place when control is handed off to a direct remote, so that local playback can be resumed later without rebuilding it from scratch. Used by two independent callers: the Sessions-panel direct-remote upgrade (`switch_to_direct_remote`) and library-scoped daemon routing (`switch_to_library_route`, #223) -- both restore through the same `restore_local_mode`.
_Avoid_: conflating with remote slot state — that's a classification of the current relationship; this is a stashed resource that exists only during part of one such relationship (direct remote or library route).

**Sessions-panel connection**:
A playback/control relationship created from F3 by selecting a discovered session. It may attach to an Emby session or directly control another mbv daemon, and F3's disconnect command may disconnect it.
_Avoid_: using this for route-owned daemon transport just because that transport reaches the same device or endpoint.

**Route-owned transport**:
The remote daemon connection created as an implementation detail of a library route. It is governed by library route policy, not by F3 session controls.
_Avoid_: treating it as a Sessions-panel connection or letting F3 disconnect it merely because routed playback is remote.

**Library route** / **Route table** (`library_routes`):
A persistent playback policy in `[library_routes]` mapping library name (matched case-insensitively, same convention as `hidden_libraries`/`feed_view_libraries`) to an endpoint-cached `tcp://host:port` value. F2 is the normal way to select a friendly live device and persist its resolved endpoint. Runtime play/enqueue resolution is a pure config read: it performs no `/Sessions` lookup or endpoint rediscovery. A stale or unreachable endpoint falls back to local playback without retry or rediscovery. A routed action swaps the active player via `switch_to_library_route`, a sibling to `switch_to_direct_remote` using the same **Suspended local session** mechanism -- tracked by its own `active_route` field, kept independent of the Sessions-panel direct-remote's `connected_session_id`/`direct_remote_label`. No wildcard. See #223, #256.
_Avoid_: conflating a library route with a Sessions-panel **Thin client** connection -- they are two independent ways to end up thin-client, and connecting to either takes driving-client authority over that daemon (ADR 0007) as an accepted consequence, not a hidden side effect.

**Routed queue**:
A queue whose route (local, or a specific library route) was decided once, from the item(s) that started it, and is fixed for that queue's lifetime (`App::active_route`). Enqueuing an item that resolves to a *different* route than the current queue's is rejected outright with a toast; no auto-clear, no auto-swap. Starting a brand-new queue (replace, not append) re-evaluates the route from scratch. Mid-queue per-track route swapping is explicitly out of scope. See #223.

## Queue

One of the central components of mbv's user experience: the subsystem that holds the ordered list of media items driving local (or locally-controlled remote) playback, independent of Emby's own data model.

### Language

**Queue**:
mbv's own core playback concept — an ordered, session-scoped list of media items driving playback. It is not a TUI convenience or merely the currently selected rows on screen. Emby has no equivalent object; a queue may be populated *from* a saved Emby Playlist, but it is not one, and can just as easily be built ad hoc (enqueue actions, "play these items") with no Playlist backing it at all.
_Avoid_: treating "queue" as an Emby API concept, a UI selection/list widget, or assuming every queue traces back to a saved Playlist entity — see **Saved-playlist queue** vs **ad-hoc queue** below.

**Saved-playlist queue** vs **ad-hoc queue**:
Whether the current queue's `QueueSource` is a named Emby `Playlist` entity (`is_saved_playlist` true) or was assembled by enqueue/"play these items" actions with no backing Playlist. Changes two behaviors: whether consume also pushes the reduced list back to Emby (`save_playlist_on_consume` for video, `save_playlist_on_consume_audio` for audio), and whether quitting with unsaved changes prompts to save.

**Consume**:
Automatic removal of an item from the queue once it finishes playing (natural end, near-end, or a next-up jump). Gated separately per track type: `consume_videos` for video items, `consume_audio` for audio items — the two flags are independent, so a user can enable one without the other. Modeled after mpd/ncmpcpp's "consume mode" (remove a track from the playlist once it's been played), which applies uniformly regardless of track type; mbv keeps the two flags separate rather than unifying them, since the two media types have different playback-completion semantics (audio has no near-end detection, only natural end and next-up jumps trigger it — see `is_near_end` in `player.rs`). See #101.
_Avoid_: confusing this with a user-initiated removal (Delete key) or a full queue clear — consume is specifically the automatic, playback-driven kind.

**`save_playlist_on_consume`** ("autosave on consume") **/ `save_playlist_on_consume_audio`**:
Real, wired-up config flags: push the queue back to the saved Emby playlist immediately after each consume. `save_playlist_on_consume` gates this for video consumes, `save_playlist_on_consume_audio` gates it for audio consumes — independently toggleable, mirroring the `consume_videos`/`consume_audio` split. Replaced an earlier `autosave_playlist` key, which was removed entirely (not just renamed) in favor of these flags.

**On-disk queue state** (`queue_state.json`) vs **in-memory queue**:
The on-disk snapshot should only matter at the two boundary moments — startup restore and quit-time save. It must never be read or re-synced mid-session; the in-memory queue is the sole source of truth while mbv is running. (#99 was a violation of this: quit could delete a valid on-disk snapshot based on incidental in-memory emptiness.)

**Local queue** vs **remote queue** (`QueueScope`):
Local is *this* mbv instance's own queue (`player_tab`); remote is the queue belonging to a directly-controlled other mbv instance/daemon (`remote_player_tab`). `QueueScopeResolution` is the source of truth for the boundary: `playback_target_queue_scope` targets Remote whenever a direct remote queue exists, `visible_queue_scope` shows Remote only when a direct remote queue exists and the user selected Remote, and `local_queue_metadata_applies` gates local-only bookkeeping (dirty flag, undo stack, saved-playlist source, on-disk persistence) so it applies to Local scope, or to any effective scope when no direct remote queue exists. See #103 (harden the local/remote queue boundary).
_Avoid_: assuming "local" vs "remote" describes where an Emby *server* session runs — both scopes are about which mbv-side queue object is authoritative, not about the media server.

**Authoritative playback queue**:
The one queue that currently owns playback authority for a given playback authority: the local queue in normal local playback, the remote queue during direct-remote control, or the daemon queue for a thin client. During remote control, the local queue may still exist and remain labeled as the local queue, but normal play routing sends local-queue playback into the remote queue, and autoload similarly loads the local queue through its autoload path, rather than making the local queue a second authority.
_Avoid_: describing local and remote queues as simultaneous playback authorities. They may coexist as stored/displayable queue objects, but only one queue drives playback for a playback authority at a time.

**Power View queue card**:
The large artwork card shown above the queue in Power View. It represents the active playback slot when playback is active, otherwise the selected slot in the currently visible queue; it is not a library preview surface.
_Avoid_: letting library focus, selected library item, or library drill depth influence this card.

During remote control, the local queue remains a first-class editable queue: the user can tailor it locally, then play/autoload it into the remote queue. Playback-derived mutations such as active item changes, progress reconciliation, consume, and next/previous belong to the authoritative remote queue, not to the local queue being edited.

**Queue cursor**:
The client-specific position or selected row within a queue. It belongs to UI/navigation state, even when it is persisted for restore, and is distinct from the active playback slot.
_Avoid_: using queue cursor as a fallback for playback identity. The currently selected row and the currently playing slot are separate concepts.

**Queue slot** / **QueueSlotId**:
A single occurrence in a queue that holds a media item, with stable identity distinct from both the Emby item ID and the slot's current index. A media item is not a queue slot; a queue slot contains a media item. Duplicate item IDs are valid in a queue, and remove/reorder/consume operations shift indices, so playback events and progress reconciliation need occurrence identity rather than raw item ID or index alone.
Operations that target a specific queue occurrence should prefer `QueueSlotId`; UI indices, mpv playlist positions, and wire-compatible index commands are adapter coordinates that should resolve to slot identity as early as possible. Slot IDs are runtime identities and are regenerated when a queue is restored from disk.
_Avoid_: treating `item_id` as queue identity, treating an index as stable across async event boundaries, or persisting slot IDs as cross-run identity.

**Slot-targeted progress**:
Playback progress and stop/completion events apply to a queue slot, not to a raw queue index. If the slot still exists after reorder or removals before it, the event updates that slot at its current position. If the slot no longer exists because it was removed, consumed, or the queue was replaced, the event must not mutate the current queue.
For the active slot, local playback progress is authoritative: mbv has mpv's live position, while Emby userdata may lag or be stale. Server refresh/enrichment must not overwrite active-slot local progress.
_Avoid_: rejecting a valid progress event only because the queue index changed, or applying an old progress event to whatever item currently occupies the old index.

**Pending progress sync**:
Local playback progress for a queue slot that mbv has reported to Emby but has not yet seen confirmed in fetched server userdata. While progress sync is pending, stale server progress must not overwrite the local value; if Emby does not catch up, mbv should treat that as a sync problem rather than accepting the stale server value. Resume position can be considered confirmed when server userdata matches the locally reported position within a small tolerance, such as a few seconds; watched/completed state should match exactly.
_Avoid_: treating an HTTP success from the stopped/progress endpoint as proof that a later server refresh will immediately contain the reported userdata.

**Queue refresh/enrichment**:
A server fetch that updates queue slot metadata and userdata. Refresh/enrichment may prune inactive slots that the server no longer returns, but active slots and slots with pending progress sync are protected: mbv must not remove or overwrite them solely because one server fetch missed them or returned stale userdata.
_Avoid_: wholesale replacement of queue slots from server results, especially for active or locally pending slots.

**mpv playlist position**:
An adapter observation from mpv, not authoritative queue state. mbv owns the queue; mpv should reflect the queue and report signals such as playlist position changes, which mbv validates against the expected queue transition and resolves to queue slot identity.
_Avoid_: silently assigning active playback identity from raw mpv `playlist-pos` without validating that mbv expected the transition.

**Active-slot removal**:
Manual removal of the currently playing queue slot is a combined queue deletion and playback stop, so it requires confirmation. Ordinary queue removals are lower risk because they are undoable; removing the active slot also stops playback, which makes it a distinct destructive action. Once confirmed, the stop/removal flow must still target the original queue slot.
_Avoid_: silently treating active-slot removal like a normal remove, or advancing playback automatically unless that behavior is intentionally redesigned.

**Move** (`Shift+Up`/`Shift+Down`, #105):
Single-step adjacent reorder of the item at the queue cursor, local-queue-only. Modeled as its own `UndoEntry::Move { from, to, item_id }` variant alongside `UndoEntry::Remove` — the undo stack is not exclusively "removals to re-insert"; it's a small set of reversible queue edits, and a move reverses by swapping the item back rather than re-inserting it. The cursor always follows the moved item, both on the initial move and when that move is undone. `item_id` is checked against whatever now sits at `to` before reversing — if a later edit shifted things around, undo refuses instead of swapping the wrong pair. Remote-queue reorder is out of scope here — see #93.

_Avoid_: assuming mpv's `playlist-move <i1> <i2>` command means "move the entry at i1 so it ends up at index i2" — that's only true when i1 > i2. When i1 < i2 the entry actually lands at i2 - 1 (mpv's own manual calls this out as a "paradox": i2 names the pre-move slot, not the post-move index). `QueueSession`'s mpv command call compensates by passing `to + 1` in that direction (verified against a live mpv instance) so it lands where the app's own `items`/`current_idx` bookkeeping expects.

## Daemon/TUI control seam

The boundary where the TUI process and a long-running daemon process exchange player commands and status over a socket (Unix or TCP), as JSON lines.

### Language

**Wire command**:
The serialized representation of a player command as it crosses the daemon/TUI process boundary. Currently the *same* Rust type as the in-process channel message (`PlayerCommand`, derived `Serialize`/`Deserialize` with no explicit tag renaming, so the JSON tag is the Rust variant identifier verbatim) — see #81, which introduces a dedicated `WireCommand` adapter type with pinned, stable serde tags and an exhaustive conversion match, so a purely in-process rename or a new variant can no longer silently reshape or break the wire protocol.
_Avoid_: assuming the wire representation and the in-process message are — or should stay — the same type; that coupling is the specific problem #81 addresses.

**Capability negotiation**:
The connection-time exchange of feature-support strings (`CtrlHello.capabilities`) between daemon and TUI. Currently all-or-nothing at the whole-connection level: `validate_peer` rejects the entire connection if any capability in a fixed required list is missing, and the negotiated list is otherwise discarded (not stored for later query). It is not yet a per-command runtime check.
_Avoid_: assuming a capability maps to a specific `PlayerCommand` variant today — no such mapping exists yet; capabilities currently describe coarser daemon-side features (queue state, start-index play, status-only mode).

**Command rejection** (#90):
A runtime, per-request refusal by the daemon to act on a command it received over the ctrl socket, distinct from connection-level capability negotiation above. First (and currently only) instance: a daemon running in audio-only mode silently dropping a play request for a non-audio item — today this is a server-side log line only, with no signal to the requesting client. #90 narrows this to a reactive design: the daemon remains the sole source of truth (no proactive client-side mirroring of daemon mode/state) and reports a rejection back over the wire instead of dropping it silently, scoped to the ctrl-socket path only (not the Emby-websocket-driven remote-control path, which has no TUI on the other end to show anything to).
_Avoid_: conflating this with per-command capability gating (the broader, still-undesigned idea of the TUI knowing ahead of time which commands a connected daemon supports) — command rejection is a narrower, already-scoped mechanism for one concrete, reproducible case; general capability-aware gating remains unspecified.

**Driving client** (#97/#107):
The single ctrl-socket TUI attached to a daemon and holding authority over its queue/session view. Settled via #107's grilling: the daemon's real deployment is one central server with one physical audio output, so observer clients are not a domain concept. The daemon accepts **at most one ctrl connection at a time**, and that connection *is* the driving authority — connection and driver are one concept, not two. **Connecting is itself the takeover**: when a new TUI connects, the daemon evicts the incumbent connection before the newcomer becomes authoritative, then the newcomer is the sole ctrl driver. "Driving" is the authority to issue *new* commands, not proof that media is actively playing and not a claim on the daemon's ongoing behavior: a daemon whose sole connection has dropped has *no driver*, yet it keeps discharging its **Daemon contract** — playing the retained queue unless the last driver left it paused or stopped. A daemon with no queue/session at all has neither driver nor contract. See ADR 0003 for the exclusive ctrl-connection rule and ADR 0007 for the separate ctrl-vs-Emby authority model.
_Avoid_: reintroducing a "pending connection" that stays live without evicting the incumbent (a same-session #107 doc edit did exactly this and it silently reversed the eviction decision — see ADR 0003 and #119); and treating ctrl connection presence, Emby remote activity, and tray commands as one undifferentiated authority mechanism — ADR 0007 and ADR 0004 separate those cases on purpose.

**Daemon contract**:
The standing playback intent the last driver left the daemon under, which the daemon continues to fulfill *autonomously* after that driver disconnects — playing the retained queue unless and until a future driver commands otherwise. Disconnecting does not release the contract: if the driver left a queue, the daemon remains bound to play it. Distinct from **driving authority** (the right to issue *new* commands, held only by the current sole connection — absent while nobody is connected) and from **playback state** (playing / paused / stopped, which merely reflects how far along the contract is, set by the last driver's last command). So a daemon with no connection still plays on; pausing or stopping is a *commanded* term of the contract that likewise persists across disconnect, not a consequence of the client leaving.
_Avoid_: treating disconnect as a reason to stop playback or discard the queue, or coupling "is anyone driving?" to "is the daemon doing anything?" — the contract outlives the connection.

**Cold daemon**:
A daemon process between startup and its first successful queue/session-shaping command: it holds no queue at all — not an empty snapshot restored from disk, an actual absence of one — until a client (a ctrl-socket TUI via an adopt/play command, or an Emby remote-control websocket command) gives it one. Cold means *never yet driven* — no contract has ever been established — which is distinct from a daemon whose driver has since disconnected: that one has no current driver either, but it retains its queue and its **Daemon contract** (above). A queue exists only once some driver has shaped one.
_Avoid_: treating an empty queue on a freshly started daemon as a missing "restore my last queue from disk" step, or flagging the daemon's lack of self-persistence/self-restore as a bug — that's the intended shape of the client-driven model above, not a gap in it.

**Local daemon**:
A daemon deployment relationship where the daemon is running on the same machine as the TUI instance. This describes location, not whether the TUI is operating as a thin client or in the separate direct-remote-queue model.
_Avoid_: using this as a full substitute for **Thin client** — same-machine placement and queue/control semantics are different axes.

**Lazy daemon route connect** (#222):
The connect-timing rule for the daemon-route lifecycle mbv builds beyond the existing startup-only `--connect-daemon`/`daemon_client_endpoint` **Thin client** path (unaffected — see ADR 0010): by default, mbv never attempts a daemon connection at startup for a route; when `auto_reconnect` is enabled (#236), it additionally makes one startup attempt to restore whichever remote connection was active at last exit, via `App::try_auto_reconnect` (`src/app/mod.rs`) — see ADR 0010's "Correction (#236)" section. The first play/enqueue action that resolves to a configured route (the wildcard "route everything" case, or a per-library entry — see #223) is what triggers the first connect attempt, via `App::try_daemon_route_connect` (`src/app/mod.rs`).
_Avoid_: confusing this with the existing `explicit_daemon_endpoint` branch in `main.rs`, which still connects (or hard-exits) at startup and is untouched by this rule — the two are separate, additive mechanisms per #222.

**Fallback to local playback** (#222):
The on-failure behavior of a lazy daemon route connect attempt: stay on (or return to) the local `Player` rather than hard-failing/exiting. `App::try_daemon_route_connect` (`src/app/mod.rs`) always logs the raw failure (`log::warn!`) and returns a fully-formatted, ready-to-display warning as its `Err` payload (e.g. "⚠ Music route unreachable, using local playback (mbv.log)"); the caller decides how to surface it -- a direct `App::flash_status_high`, or threaded through a state-teardown path -- since only the caller knows its own routing state. Distinct from **Local daemon** — that term is about deployment location, not this failure-mode policy.
_Avoid_: treating a failed route connect as fatal, or falling back with no user-visible signal — both were true of the pre-#222 startup-time behavior this replaces for the new mechanism. Also avoid assuming `try_daemon_route_connect` itself calls `flash_status_high` — it deliberately does not (see ADR 0010).

**No background retry** (#222):
After a failed daemon route connect attempt, mbv does not schedule another attempt on a timer or in the background. The next attempt happens only on the next natural trigger — the next play/enqueue action that resolves to that route (which, for the wildcard case, in practice means the next mbv restart). Not to be confused with `DaemonEndpoint::connect_stream`'s existing bounded retry loop for `DaemonEndpoint::Local` (`crates/mbv-core/src/remote_player.rs`, `LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT`), which waits out a same-machine daemon's startup race *within* one connect attempt and is unrelated/unaffected.
_Avoid_: conflating the existing intra-attempt local-daemon retry loop with this rule — this rule is about not scheduling a *new* attempt after a whole connect attempt has already failed.

**No connection parking** (#222):
When mbv swaps away from a daemon route back to local, it disconnects cleanly (drops the `RemotePlayer`, taking no action to keep it or its socket alive) rather than parking the connection the way **Suspended local session** parks a local `Player` during a direct-remote takeover. The next time that route is needed, it reconnects fresh. Chosen so mbv does not silently continue holding a daemon's **Driving client** authority (ADR 0003, ADR 0007) on a route that is not actively in use.
_Avoid_: reusing the `SuspendedLocalSession` pattern (or inventing an equivalent) to keep a disconnected daemon-route `RemotePlayer` alive for reuse — that is the "connection parking" this rule rules out.

**Daemon responsibility boundary** (#97/#73):
What belongs in the daemon vs. what's "Emby-specific weight" to push elsewhere turned out narrower than first assumed. Stays daemon-side, unconditionally: the queue (`items`/`cursor` — this *is* the daemon's job, not Emby-specific), Emby progress/watched-state sync (pinned there by physics — only the process actually running mpv can observe playback events in real time to report them), and the Emby remote-control websocket (just another command source hitting the same queue, no different from a ctrl-socket client). Moves out entirely, unconditionally: `mpris` (client-side desktop integration; a daemon has no desktop session, regardless of deployment mode). The actual "thin daemon" goal is a build-dependency problem, not a runtime-responsibility redesign — see #97 for the `mbvd` binary-target split and #73 for the underlying crate extraction both depend on.
_Avoid_: assuming "Emby-specific" and "should move out of the daemon" are the same test — queue/progress-sync/remote-control-input are all Emby-shaped but stay, precisely because they're tied to the daemon's actual job (driving mpv) rather than being incidental weight.

## Process lifecycle & residency

How an mbv process's life relates to the terminal it was launched from. Distinct from the **Daemon/TUI control seam** above: that seam is about a *separate* daemon process a TUI talks to over a socket; this is about a *single* mbv process outliving its terminal with no second process and no socket.

### Language

**Stay alive** (mode; `--alive`, config `stay_alive`, replacing the old `mbv -d` — see #156):
An opt-in mode in which mbv **survives its controlling terminal being closed** — the mbv process keeps its in-process `Player`/mpv, queue, tray, and MPRIS running, and can be reattached later. Persistence is provided by a pty **relay** that holds a terminal for mbv so it always believes it has one; "closing" detaches the head, "reopening" reattaches one. (An external `abduco`/`dtach` was used to *prototype* this; the shipping design is an **owned relay** so mbv can control detach itself — see #156.) **mbv remains the single owner** of playback, queue, and state throughout — no daemon, no ctrl socket, no `RemotePlayer`, no queue handoff/merge — so none of the **Daemon/TUI control seam** machinery is involved and the `is_local_daemon` unified-vs-split-queue fork never arises. A stay-alive session is always **attended**: it always has at least a tray head. Default off; bare `mbv` keeps the ordinary behavior where quitting stops mpv.
_Avoid_: calling this "headless", "daemon", or "background mode" — all three describe `mbvd` (unattended, no head, systemd, central-server), the deliberate opposite of stay-alive (attended, always ≥ a tray, your desktop session); "daemon" is reserved for `mbvd`. Also avoid treating *process count* as the invariant: the backend may run several processes (the relay, plus an ephemeral terminal-client while attached) for execution isolation and crash recovery — that plumbing is invisible to the user and is a different axis from the daemon control seam (the relay is a dumb byte pipe, not a `Player`-holding daemon). The invariant is **single ownership by mbv + transparency to the user**, not one OS process.

**Alive session**:
The running mbv process while it is detached from any real terminal but still playing — the state between a detach and the next reattach. It is never truly headless: the tray is its minimal head.
_Avoid_: equating an alive session with a **Cold daemon** or any daemon state — mbv still owns the queue in its own process, exactly as before the head detached. (A pty relay process and its terminal-relay socket may exist as backend plumbing, but that socket carries raw terminal bytes, not the ctrl/queue protocol a daemon speaks.)

**Detach / reattach**:
The head-off / head-on transitions of a stay-alive session, performed by the pty relay. With the **owned relay**, mbv can initiate a detach itself (e.g. `q` → a detach command over a control channel) — something stock `abduco`/`dtach` cannot do, which is the reason for owning the relay (#156). Detach = the real terminal is released (or its window closed) while mbv keeps running against the relay's pty; reattach = a terminal-client binds a real terminal back to it. On reattach mbv must force a full repaint, re-enable mouse capture, and re-emit already-drawn images (mbv currently drops the `Event::Resize` that would trigger this — #156).
_Avoid_: modeling detach/reattach as a control client connecting to a daemon — the relay carries raw terminal bytes between the ephemeral terminal-client and the persistent mbv process; it is not the ctrl/queue protocol of the **Daemon/TUI control seam**, and the terminal-client is not a `Thin client`.

**Owned relay**:
The thin, persistent process mbv spawns to provide **Stay alive** persistence: it creates the pty, holds the master fd, serves a unix socket, relays raw terminal bytes to a **Terminal-client**, and runs the mbv app under it on the pty slave. "Owned" because mbv writes it rather than shelling out to `abduco`/`dtach` — only an owned relay can offer both graphics passthrough and program-initiated detach (see ADR 0005). It is a dumb byte pipe plus a two-message out-of-band control channel (`client attached`, `detach now`), never a `Player`-holding daemon.
_Avoid_: calling it a daemon or attributing queue/playback ownership to it — it owns a pty and a socket, nothing about mbv's state; mbv (the app) remains the single owner.

**Terminal-client**:
The ephemeral process a bare `mbv` becomes when it attaches to an alive session: it raw-modes the real terminal and relays real-terminal↔relay-socket bytes, exiting on detach. A pure *terminal relay*, deliberately dumb so an mbv/libmpv crash can't wedge the terminal it owns.
_Avoid_: conflating it with a `Thin client` — it speaks raw terminal bytes over the relay socket, not the `RemotePlayer`/queue ctrl protocol, and it holds no queue or player state.

## Library browsing

The subsystem that fetches, sorts, and displays Emby library contents — artists, albums, tracks — in navigable list views.

### Language

**SortName**:
An Emby-provided per-item metadata field (e.g. an album title with leading articles normalized) used as the API's own alphabetization key. Refers to one item's own sort key, never to the order of a list.
_Avoid_: using "SortName" as shorthand for a resulting list order — see API order and Display order below, which are the actual orders a list can be in.

**API order**:
The order albums arrive in from Emby when queried with `SortBy=SortName` — alphabetical by each album's own title, not grouped or sorted by artist.
_Avoid_: "raw order", "SortName order" — both describe this same concept but obscure that it's an ordering, not a field.

**Resolved artist**:
The artist attributed to an album for grouping and header-display purposes, computed via a fallback chain: Emby's own `AlbumArtist`/`Artists` tag first, then a background-fetched majority vote over the album's first few tracks' tags (for tag-less albums), then a folder-name heuristic, then the literal "Unknown Artist".
_Avoid_: "artist" alone when what's meant is only the item's own (possibly empty) tag — that's one input to resolution, not the resolved value.

**Display order**:
The order items actually appear on screen in a power-list view, whenever that differs from API order — e.g. albums resorted by resolved artist in a grouped album view, or items resorted by `effective_sort_str` for letter-bucket grouping. Published as `left_sorted_indices` so keyboard and mouse navigation stay consistent with what's rendered, regardless of which view produced the reordering.
_Avoid_: "sorted order" — ambiguous with API order, which is also a sort result. Also avoid treating this as specific to grouped album views — any power-list view that reorders for display produces one.

**Display row**:
A row occupied in a rendered library list. A display row may correspond to a selectable media item, or it may be a structural/detail row such as an artist header, inline album-track row, separator, or loading placeholder.
_Avoid_: treating display rows and media items as interchangeable; one media item can occupy multiple display rows when inline detail is expanded, and some display rows are not selectable items at all.

**Library position**:
The per-library, per-view restoration target for returning to a library: the library's drill-down path, the active selector/pill group when that path has one, then the last focused item within that group or level. Default library view and Power View are completely isolated scopes with independent saved positions, and the position persists across app restarts so browsing resumes where the user left off, rather than repeatedly biasing discovery toward the top of alphabetically sorted lists; viewport scroll is secondary and may be clamped to keep that focused item visible. If saved content no longer exists, restore the deepest valid path prefix and focus the nearest sensible fallback without treating the stale position as a user-facing error. Lazy restore should avoid a root-first flash or delayed jump: a library/view with saved position should enter a restoring/loading state until the restored path or fallback is ready to render.
Manual refresh/rescan is an explicit reset boundary for the active view only: clear that library's saved position for the view where the user requested the reload, rather than trying to carry it across the user-requested reload. The other view's saved position remains isolated and intact.
_Avoid_: reducing this to raw scroll offset, global tab index, search state, detail-panel state, album track-selection focus, one shared cursor across all libraries, one shared position across default and Power View, or bootstrapping one view's position from the other; "sticky" means restart-persistent in this context, not merely preserved while the process is running.

**Grouped album view**:
A display mode for an album listing that inserts an artist header row at each artist boundary, built from resolved artist rather than API order. One of several power-list view modes that produce a display order (another being the letter-grouped view, which buckets by first letter instead of artist).
_Avoid_: conflating with "album level" (whether a navigation level shows albums at all) — grouping is about whether those albums are clustered by resolved artist within that level, an orthogonal concern.

**Recursive configured-music album search**:
When `music.levels` has at least one ancestor level and ends in `album`, each configured music library eagerly builds an in-memory index of leaf albums and their configured ancestor paths on a background worker. Standard Library and Power View search the same local album-only corpus by album or ancestor label; activating a match reconstructs that view's normal album destination. Explicit library refresh rebuilds the relevant index, including one coalesced replacement when refresh overlaps an in-flight build.
_Avoid_: applying this behavior to absent or album-only `music.levels`, indexing tracks, traversing the server while the user types, persisting the index, polling, or sharing Standard and Power navigation state during activation.

## UI layout

The subsystem that divides terminal space between mbv's browsing, queue, playback, and overlay surfaces.

### Language

**Power View left column**:
The left column in Power View: the column that contains the media card at the top and the queue below it. This is the "left panel" meant by #111's user-resizable panel-width work. Its width is stored as a persistent absolute column count, not a terminal-width percentage or ratio. The default and minimum width are both 40 columns, preserving today's layout until the user explicitly widens it; restored preferences above 60% of the current terminal width are normalized down and saved so the right-side main content remains usable, except that the 40-column minimum always wins on very narrow terminals.
The column can be collapsed for the current app session with lowercase `h` while Power View is active. Collapsed means the whole physical column disappears: media card, queue title/scope pills, and queue list are not rendered, and the library side expands to the full terminal width. Collapsing while the queue side has focus moves focus to the library side; reopening keeps that library-side focus instead of restoring queue focus. The collapsed/expanded state is not persisted across restarts, and the configured width is preserved for reopening.
Its resize controls are keyboard-only: `Shift+Left`/`Shift+Right`, active only while Power View itself is active and the left column is expanded, regardless of which Power View sub-panel has focus; outside Power View or while collapsed those keys are ignored silently rather than acting as global shortcuts that invisibly change a later Power View layout. Mouse resizing is not part of this interaction.
The queue always remains in the physical left column when expanded, even on short terminals; it is not relocated under the library side as a responsive fallback.
Power View text should be laid out from the current frame width after each resize: text that no longer fits must be abbreviated or ellipsized, and text should expand again when there is room.
Power View is a formal view setting, not a transient keyboard toggle. The `v` key is reserved for the audio visualizer toggle; Power View belongs in the F2 settings surface. The ordinary non-Power View app state is the default view.
_Avoid_: using "left panel" without qualification when discussing #111, since other views also have left/right areas; the resize target is specifically the Power View left column, not the normal library table split or any modal/sidebar. Also avoid reusing `v` as a Power View toggle.

**Power View panel focus**:
The restart-persistent focus choice between Power View's queue side and library side. It is separate from **Library position**: restoring the library side's focused item does not imply that the library side itself should have focus, and restoring queue-side focus does not change the saved library position.
_Avoid_: relying on the current `PowerFocus::Left` name as domain language — visually, Power View has a queue side and a library side, and those are the user-facing concepts to persist.

**Toast**:
A transient status message rendered over the TUI for short-lived feedback, such as confirming an action or explaining why a keypress had no effect. Power View left-column resize actions should use this existing feedback channel.
_Avoid_: introducing a separate notification surface for routine in-app feedback; use the existing toast/status mechanism.

**Audio visualizer**:
An optional TUI visualization of mbv's own playback audio, scoped to PulseAudio/PipeWire-pulse capture. When enabled, mbv must capture mbv-owned audio only, typically by routing mpv output through a dedicated Pulse-compatible sink and capturing that sink's monitor, but the routing must not introduce noticeable audio latency or A/V sync regression. If the first routing strategy causes latency, the implementation should troubleshoot and pivot to a cleaner capture path rather than accept degraded playback. In Power View, the visualizer renders at the bottom of the library list and `v` enables/disables that embedded surface without moving focus or scroll; that Power View visualizer preference may persist. Outside Power View, pressing `v` enters a transient fullscreen visualizer surface with no surrounding tab UI even when nothing is playing; pressing `v` again or Backspace leaves it and returns to the exact prior tab/view state. The non-Power-View fullscreen visualizer is tab-like navigation state, not a persisted preference.
_Avoid_: promising generic ALSA support, capturing the default system monitor, treating visualizer failure as a normal user-facing state instead of a defect to diagnose from logs, exposing a second visualizer toggle in F2 settings, persisting the non-Power-View fullscreen visualizer, or making it a permanent ordinary tab.

**Audio visualization subsystem**:
The subsystem that owns visualizer audio routing, capture, analysis, and bar-state lifecycle. Playback owns mpv/session semantics, and UI owns rendering; the audio visualization subsystem sits between them and turns mbv-owned audio into renderable visualization state shared by the Power View embedded surface and the fullscreen visualizer surface. The capture/analysis pipeline runs only while at least one visualizer surface is visible. Synthetic audio input is part of the design toolbox: it should support routing/capture spikes and deterministic visualizer rendering tests.
_Avoid_: spreading Pulse/PipeWire setup through playback/session code, making the renderer responsible for audio capture, running the visualization pipeline merely because a persisted preference is enabled while no visualizer surface is visible, or depending only on live media playback for testing visualizer behavior.

## Input handling

The subsystem that turns key and mouse events into app behavior. Keyboard precedence is centralized: `App::handle_key` (`src/app/input.rs`) is a loop over the `CONTEXT_STACK` context-priority registry in `src/app/input_resolver.rs` (#129, see `docs/adr/0002-centralized-input-handling.md`). Mouse (`App::handle_mouse`) routes command-like clicks through the same `Command`/`dispatch` seam but keeps genuinely spatial logic (hit-testing, drag-seek, hover) local, by design. Help (`render/overlays/help.rs`) renders its `[playback]` section from the registry's binding data; other sections are still hand-written pending further migration.

**Guardrail**: new shortcuts go through this registry, not through a new raw key/click check in view or panel code — see AGENTS.md's "Rules" section for the enforceable version and its exceptions (text entry, external setup).

### Language

**Command**:
A semantic input intent (e.g. `TogglePlayPause`, `QueuePlayCursor`, `LibrarySearchStart`), decoupled from the physical key or mouse region that triggered it. The target superset of today's `Action` enum. Both keyboard and mouse resolve to `Command`s and share one executor (`dispatch`), which keeps the stateful/branchy execution (session-vs-local, scope/empty checks).
_Avoid_: treating a `Command` as a keybinding — the same command can be bound to a chord, a mouse region, or (future) a user override; the binding is data, the command is the intent.

**Context priority stack**:
The ordered, first-match list of active input contexts (overlays, confirms, text entry, context menu, playback, view) that decides which handler consumes a given event. For keyboard this is the `CONTEXT_STACK` table in `src/app/input_resolver.rs` — the old implicit `handle_key` branch order, made explicit and assertable; mouse has the spatial equivalent in `handle_mouse`. Contexts share one taxonomy across keyboard and mouse.
_Avoid_: modeling input as a flat global shortcut table — that loses precedence; and avoid computing a single "active context" — contexts stack, and some (e.g. settings) have their own ordered sub-stack.

**Key resolution** (`Command` / `Swallow` / `FallThrough`):
The three outcomes of resolving a chord in a context. `Command` dispatches an intent; `Swallow` consumes the key with no action (e.g. an overlay eating unknown keys); `FallThrough` defers to a lower-priority context (today only the playback layer genuinely falls through). This trichotomy is made explicit by the resolver's return type, `KeyResolution`, in `src/app/input_resolver.rs`.
_Avoid_: conflating `Swallow` and `FallThrough` — an overlay swallowing a key is not the same as a gated playback key declining it.

**Input snapshot**:
The plain-data view of app state the pure resolver reads (~20 fields, one behind a lock), rather than borrowing `&App`. Keeping it a data snapshot is what makes the resolver unit-testable and doubles as the written-down answer to "what does input depend on?".
_Avoid_: passing `&App` into resolution or reaching into `App` fields mid-resolve — that defeats the pure, testable seam.

**Text-entry context**:
An input context (search boxes, the save-name dialog) that owns local state and captures printable characters via a catch-all `Char` binding, plus a small set of editing keys (`Esc`, `Backspace`, `Enter`, arrows). The registry *routes to* these contexts but does not express their internals as bindings.
_Avoid_: trying to model text entry or modal state machines (save-playlist stages, settings sub-modes) as chord-to-command rows — they are local state machines behind a context.

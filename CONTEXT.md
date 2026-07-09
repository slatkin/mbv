# mbv Domain Glossary

mbv is a terminal (TUI) client for Emby. This glossary covers the domain vocabulary specific to mbv, grouped by area.

## Playback

The subsystem that drives mpv, tracks play position, and hands control of playback back and forth between local and remote players.

### Language

**Session**:
A running playback of a queue of media items (one item or many), owning one mpv instance for as long as that queue plays.
_Note_: currently implemented as two separate, non-unified state machines — `SingleSession` and `QueueSession` — which duplicate almost all of their fields and logic (intro-skip markers, resume position, next-up handling, pause state, etc.), differing only in whether there's one item or an ordered list with a cursor. This is one domain concept split by implementation history (single-item playback existed first; playlist support was added as a parallel path rather than a generalization), not two domain concepts. Treat "Session" as a single term when talking about the domain; the two-struct split is an implementation debt, not a modeling decision.

**Remote slot state**:
A derived classification — recomputed on demand, never stored — of which kind of control relationship the app currently has to a player: none, attached to another session, directly remote, or acting as a local daemon.
_Avoid_: implying it's a field that gets set; it's computed fresh from other state each time it's read.

**Thin client**:
A TUI instance whose authoritative playback state comes from a daemon-backed player/queue instead of a locally-owned player. This describes the control model, not where the daemon runs.
_Avoid_: treating this as synonymous with **Local daemon** — a thin client can target either a same-machine daemon or a remote daemon.

**Suspended local session**:
The local player and its event channels, parked in place when control is handed off to a direct remote, so that local playback can be resumed later without rebuilding it from scratch.
_Avoid_: conflating with remote slot state — that's a classification of the current relationship; this is a stashed resource that exists only during part of one such relationship (direct remote).

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
The single ctrl-socket TUI currently attached to a daemon and holding authority over its queue/session view. Settled via #107's grilling: the daemon's real deployment is one central server with one physical audio output, so observer clients are not a domain concept; a takeover evicts other ctrl-socket connections rather than demoting them to observers. Before eviction, the daemon sends the old connection an explicit takeover-disconnect event so the TUI can turn its remote/session indicator off and notify the user; then the daemon closes that ctrl socket. A newly connected TUI may exist as a pending connection long enough to receive initial state and send a command, but it does not become the driver or evict the current driver merely by connecting. A client becomes the driver only after one of its control commands successfully exerts will over daemon playback or queue state; a command that is rejected or ignored does not make that client the driver. "Driving" is authority over the daemon's queue/session, not proof that media is actively playing: a stopped daemon with a retained queue still has a driver, while a daemon with no queue/session has none. A successful Emby remote-control websocket command also evicts the current ctrl-socket TUI with the same explicit disconnect-before-close behavior; the daemon may still have queue/session state afterward, but no TUI is driving it until one successfully executes a later ctrl command.
_Avoid_: treating the current `ctrl_clients: Vec<_>` implementation as a settled multi-observer model — for #107 it is implementation history to replace or constrain, not desired semantics.

**Cold daemon**:
A daemon process between startup and its first successful queue/session-shaping command: it holds no queue at all — not an empty snapshot restored from disk, an actual absence of one — until a client (a ctrl-socket TUI via an adopt/play command, or an Emby remote-control websocket command) gives it one. Follows directly from **Driving client**: since the queue belongs to whichever client currently holds driving authority, a daemon with no driver yet has no queue to hold.
_Avoid_: treating an empty queue on a freshly started daemon as a missing "restore my last queue from disk" step, or flagging the daemon's lack of self-persistence/self-restore as a bug — that's the intended shape of the client-driven model above, not a gap in it.

**Local daemon**:
A daemon deployment relationship where the daemon is running on the same machine as the TUI instance. This describes location, not whether the TUI is operating as a thin client or in the separate direct-remote-queue model.
_Avoid_: using this as a full substitute for **Thin client** — same-machine placement and queue/control semantics are different axes.

**Daemon responsibility boundary** (#97/#73):
What belongs in the daemon vs. what's "Emby-specific weight" to push elsewhere turned out narrower than first assumed. Stays daemon-side, unconditionally: the queue (`items`/`cursor` — this *is* the daemon's job, not Emby-specific), Emby progress/watched-state sync (pinned there by physics — only the process actually running mpv can observe playback events in real time to report them), and the Emby remote-control websocket (just another command source hitting the same queue, no different from a ctrl-socket client). Moves out entirely, unconditionally: `mpris` (client-side desktop integration; a daemon has no desktop session, regardless of deployment mode). The actual "thin daemon" goal is a build-dependency problem, not a runtime-responsibility redesign — see #97 for the `mbvd` binary-target split and #73 for the underlying crate extraction both depend on.
_Avoid_: assuming "Emby-specific" and "should move out of the daemon" are the same test — queue/progress-sync/remote-control-input are all Emby-shaped but stay, precisely because they're tied to the daemon's actual job (driving mpv) rather than being incidental weight.

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

**Grouped album view**:
A display mode for an album listing that inserts an artist header row at each artist boundary, built from resolved artist rather than API order. One of several power-list view modes that produce a display order (another being the letter-grouped view, which buckets by first letter instead of artist).
_Avoid_: conflating with "album level" (whether a navigation level shows albums at all) — grouping is about whether those albums are clustered by resolved artist within that level, an orthogonal concern.

## UI layout

The subsystem that divides terminal space between mbv's browsing, queue, playback, and overlay surfaces.

### Language

**Power View left column**:
The left column in Power View: the column that contains the media card at the top and the queue below it. This is the "left panel" meant by #111's user-resizable panel-width work. Its width is a persistent user layout preference, not a transient per-session adjustment.
Its resize controls are active only while Power View itself is active; they are not global shortcuts that invisibly change a later Power View layout from another tab.
_Avoid_: using "left panel" without qualification when discussing #111, since other views also have left/right areas; the resize target is specifically the Power View left column, not the normal library table split or any modal/sidebar.

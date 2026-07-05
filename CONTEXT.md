# mbv Domain Glossary

mbv is a terminal (TUI) client for Emby. This glossary covers the domain vocabulary specific to mbv, grouped by area.

## Playback

The subsystem that drives mpv, tracks play position, and hands control of playback back and forth between local and remote players.

### Language

**Session**:
A running playback of a queue of media items (one item or many), owning one mpv instance for as long as that queue plays.
_Note_: currently implemented as two separate, non-unified state machines — `SingleSession` and `PlaylistSession` — which duplicate almost all of their fields and logic (intro-skip markers, resume position, next-up handling, pause state, etc.), differing only in whether there's one item or an ordered list with a cursor. This is one domain concept split by implementation history (single-item playback existed first; playlist support was added as a parallel path rather than a generalization), not two domain concepts. Treat "Session" as a single term when talking about the domain; the two-struct split is an implementation debt, not a modeling decision.

**Remote slot state**:
A derived classification — recomputed on demand, never stored — of which kind of control relationship the app currently has to a player: none, attached to another session, directly remote, or acting as a local daemon.
_Avoid_: implying it's a field that gets set; it's computed fresh from other state each time it's read.

**Suspended local session**:
The local player and its event channels, parked in place when control is handed off to a direct remote, so that local playback can be resumed later without rebuilding it from scratch.
_Avoid_: conflating with remote slot state — that's a classification of the current relationship; this is a stashed resource that exists only during part of one such relationship (direct remote).

## Daemon/TUI control seam

The boundary where the TUI process and a long-running daemon process exchange player commands and status over a socket (Unix or TCP), as JSON lines.

### Language

**Wire command**:
The serialized representation of a player command as it crosses the daemon/TUI process boundary. Currently the *same* Rust type as the in-process channel message (`PlayerCommand`, derived `Serialize`/`Deserialize` with no explicit tag renaming, so the JSON tag is the Rust variant identifier verbatim) — see #81, which introduces a dedicated `WireCommand` adapter type with pinned, stable serde tags and an exhaustive conversion match, so a purely in-process rename or a new variant can no longer silently reshape or break the wire protocol.
_Avoid_: assuming the wire representation and the in-process message are — or should stay — the same type; that coupling is the specific problem #81 addresses.

**Capability negotiation**:
The connection-time exchange of feature-support strings (`CtrlHello.capabilities`) between daemon and TUI. Currently all-or-nothing at the whole-connection level: `validate_peer` rejects the entire connection if any capability in a fixed required list is missing, and the negotiated list is otherwise discarded (not stored for later query). It is not yet a per-command runtime check. A follow-up to #81 explores per-command capability gating with graceful UI degradation (e.g. hiding/disabling a control the connected daemon doesn't support) — that needs its own design pass before it's buildable.
_Avoid_: assuming a capability maps to a specific `PlayerCommand` variant today — no such mapping exists yet; capabilities currently describe coarser daemon-side features (queue state, start-index play, status-only mode).

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

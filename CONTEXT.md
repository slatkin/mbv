# Playback

The subsystem that drives mpv, tracks play position, and hands control of playback back and forth between local and remote players.

## Language

**Session**:
A running playback of a queue of media items (one item or many), owning one mpv instance for as long as that queue plays.
_Note_: currently implemented as two separate, non-unified state machines — `SingleSession` and `PlaylistSession` — which duplicate almost all of their fields and logic (intro-skip markers, resume position, next-up handling, pause state, etc.), differing only in whether there's one item or an ordered list with a cursor. This is one domain concept split by implementation history (single-item playback existed first; playlist support was added as a parallel path rather than a generalization), not two domain concepts. Treat "Session" as a single term when talking about the domain; the two-struct split is an implementation debt, not a modeling decision.

**Remote slot state**:
A derived classification — recomputed on demand, never stored — of which kind of control relationship the app currently has to a player: none, attached to another session, directly remote, or acting as a local daemon.
_Avoid_: implying it's a field that gets set; it's computed fresh from other state each time it's read.

**Suspended local session**:
The local player and its event channels, parked in place when control is handed off to a direct remote, so that local playback can be resumed later without rebuilding it from scratch.
_Avoid_: conflating with remote slot state — that's a classification of the current relationship; this is a stashed resource that exists only during part of one such relationship (direct remote).

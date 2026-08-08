## Context

See `proposal.md` — Why, and ADR 0017 for the model.

The constraints that shape the approach:

- `audio_only_rejection` (`daemon_core.rs:565`) is load-bearing. `PlayItems`
  hands the whole fetched list to `play_queue` (`daemon_control.rs:396`), which
  loads it into mpv as an mpv playlist that mpv advances through unaided. The
  rejection is the only thing keeping video away from a player with no display.
- It is reached from three places: `daemon_control.rs:361` (ctrl play),
  `daemon_run.rs:559` (playback intents), and `daemon_ws.rs:35` (Emby-started).
- `all_audio` (`daemon_ws.rs:172`) is already a thin wrapper over
  `MediaItem::is_audio` (`api_types.rs:115`). That predicate is the single
  audio test and stays so.

## Goals / Non-Goals

**Goals:**

- One admission point per submission path, so the owner's queue and its mpv
  playlist can never diverge.
- No behavior change for a wholly non-audio submission.

**Non-Goals:**

- Any index mapping between the owner's queue and mpv's playlist. The design
  exists to avoid needing one.
- Reporting owner-side discards over ctrl.
- Any client change, any protocol change.

## Decisions

### Filter at admission, not at advance

`audio_only_rejection` becomes an admission filter returning the admitted items
and a discard count, applied where the item list is resolved and before it
reaches `play_queue`/`play`. The owner's `items`, its cursor, and mpv's playlist
stay one list.

*Alternative considered:* keep non-audio items in the owner's queue, mark them
unplayable, and skip at advance. Rejected in ADR 0017 — it splits the owner's
list from mpv's playlist permanently, for visibility better provided at the
client.

### Wholly non-audio still rejects

Filtering to nothing and starting nothing would be a silent no-op for a client
that has no idea the owner is audio-only. Today that submission is rejected with
a structured reason, and it stays rejected: mixed becomes admissible, wholly
non-audio does not. This keeps the `AudioOnly` rejection a real path rather than
dead code kept "as a backstop", and it is what
`audio-only-owner-fall-through` leans on for any client that predates the
capability.

### One filter, three call sites

The filter is a single pure function over `&[MediaItem]`, called from the ctrl
play path, the intent path, and the ws path, mirroring how `audio_only_rejection`
is called today. Keeping it pure preserves the existing property that it is
testable without a live `Player` or `EmbyClient` — the reason the doc comment
above the current function gives for its shape.

### Start index is remapped, not clamped

Filtering shifts positions, so a start index computed against the unfiltered
list is wrong. It is remapped to the first admitted item at or after the
original position, falling back to the last admitted item. Clamping to
`len - 1` after filtering, which is what `PlayItems` does today, would silently
start the wrong track.

## Risks / Trade-offs

- **Discarding is silent at the owner.** → Accepted, per ADR 0017. Nothing on
  the daemon side is told; a log line is the whole report. Once
  `audio-only-owner-fall-through` lands, a capability-aware client strips first
  and tells the user, so an owner-side discard means the client's type
  information was wrong or no client was involved.
- **Playback started from Emby has no client at all.** → Accepted. Non-audio
  items are discarded with a log line. This is strictly better than today, where
  the whole request was refused with nobody told.

## Migration Plan

No migration and no protocol change. Every existing client submits exactly as it
does now; the only difference is that a mixed submission plays its audio instead
of playing nothing.

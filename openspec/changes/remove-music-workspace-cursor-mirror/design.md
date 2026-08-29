## Context

The Music workspace is the inverse of the Audiobookshelf surfaces. Its
component→shell direction is already spec-compliant (a resolved `target`,
applied directly); its shell→component direction is not. `set_content` decides
whether to adopt the pushed cursor by comparing the component's current value
against a stored copy of the last pushed value:

```
push N  ──▶ album_cursor = N, last_mirrored = N
user moves ──▶ album_cursor = N+1        (last_mirrored still N)
push N  ──▶ album_cursor != last_mirrored  ──▶ keep N+1   (echo suppressed)
push M  ──▶ album_cursor != last_mirrored  ──▶ keep N+1   (real change LOST)
```

The last line is the defect. The equality test cannot distinguish "this push is
my own value coming back" from "this push is a genuine shell re-anchor",
because both arrive through the same per-push channel. Whether a real
re-anchor lands depends on whether the user moved since the previous push.

## Decisions

### D1 — The component owns the cursor; the shell re-anchors by event

`set_content` stops carrying cursor, scroll, and track focus altogether. The
three places where the shell legitimately needs to move the component's cursor
are discrete navigation events, already identifiable in the source because they
are the same three places that write `level.cursor` from the grouping catalog
(`music_grouping.rs:296-309`):

1. Group (album-artist) switch
2. Recursive-album activation
3. Saved-position restore on library entry

Each gets an explicit re-anchor call. Everything else — arrow keys, paging,
scrolling — is component-local and never round-trips.

*Rationale:* this is the discipline the spec already permits ("it MAY push
shell-owned navigation state into the component only at the discrete event
where the visible level changes"). Making the re-anchor a named call at three
sites, rather than an equality test on eleven push sites, is what turns an
order-dependent behaviour into a deterministic one.

*Rejected:* keeping the per-push adoption and fixing the equality test (e.g.
with a generation counter or a dirty flag). Any such fix is still echo
detection; it makes the mirror more reliable instead of removing it, and it
leaves a field whose meaning is "am I in sync with the other owner".

### D2 — Deleting `last_mirrored_*` is the acceptance test

The change is done when `rtk grep -c "last_mirrored" src/` returns zero and
the workspace still passes its component and shell tests. A diff that keeps
either field under a new name has not landed this change.

### D3 — `MusicWideRenderCtx` sheds what it no longer projects

Once `set_content` reads none of them, `list.cursor`, `list.scroll`, and
`track_cursor` are dead weight on the context struct for this consumer. Remove
them from `MusicWideRenderCtx` **only if** no other consumer reads them —
`LibraryListRenderCtx` is shared, so the likely outcome is that `track_cursor`
goes and `list` stays. Confirm before deleting; a shared render context is not
this change's to reshape.

### D4 — Track focus keeps its identity reset

`last_album_id` looks superficially like `last_mirrored_*` but is not: it
detects that the *content* changed (a different album is selected), which
genuinely invalidates a track index into the previous album's track list. That
is an event-driven reset on content identity, which D1 permits. It stays.

## Risks

- **A re-anchor site is missed.** If one of the three events does not get an
  explicit call, its symptom is a cursor that fails to move on group switch or
  fails to restore on library entry — silent, and easy to miss without a test.
  Task 1 enumerates the sites from the `level.cursor` writers before any
  removal, so the list is derived from the source rather than from this
  design's summary of it.
- **Order-dependence may be load-bearing somewhere.** A current behaviour may
  depend on the suppression accidentally doing the right thing. Task 1.2 pins
  the "real change lost" case as a failing-today test, so the fix is proven to
  change behaviour in the intended direction only.

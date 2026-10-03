# Invariant 6 — mpv's one-shot resume start

**Scope:** mpv playlist entry resume options, native playlist navigation,
and owner-side relative steps and jumps (`ProgressObservation` in
`crates/mbv-queue` and `PlayerOwnerState::relative_step_target` in
`crates/mbv-player`).

## The invariant

mpv bakes a playlist entry's `start=<seconds>` resume option in once, at
`loadfile` time, and never re-reads or updates it afterward. Navigating back to
that entry with `playlist-pos`—including navigation initiated natively in
mpv—reopens it from that frozen value, not from its latest recorded position.
A re-visited entry therefore needs an explicit absolute seek after mpv has
loaded it; an immediate seek after `playlist-pos` is rejected because no file
is loaded yet. The Playback run's one-shot first-restart handling is the
loaded-entry point for this seek.

The seek target is not the canonical queue's stored position for every kind.
For an Emby video slot, `cmd_jump_to` (`run/commands/queue.rs`) refreshes that
item from Emby at jump time (`refresh_emby_resume`) and arms the jump with the
refreshed `resume_ticks_for_item`, so the re-visit seeks to the server's
current position rather than the owner's loaded value. The canonical-queue
position is used for the seek only for Feed entries; Audiobookshelf slots
take their position from the item's open session and are skipped by
`override_jump_resume`.

Progress positions are interpreted by the shared `ProgressObservation`, and
relative navigation targets are resolved by `PlayerOwnerState::relative_step_target`.
There remains a narrow race: a jump dispatched before the owner applies that
item's preceding `TrackCompleted` observation can carry the prior position.

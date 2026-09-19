# Invariant 6 — Progress must be applied at every queue mirror, in order

**Scope:** the daemon's canonical Bound queue (`PlayerOwnerState.queue`,
`crates/mbv-core/src/player/owner_state.rs`), the Playback run's own queue
mirror (`ExecutionSequence`, `crates/mbv-core/src/playback/execution_sequence.rs`),
each connected shell's `PlaybackQueue` mirror (`src/app/player_event.rs`), the
shared write path (`crate::playback::queue::apply_progress_to_queue_item`),
resume resolution (`crate::player::resume_start_pos` /
`resume_ticks_for_item` / `resume_ticks_for_slot`, `crates/mbv-core/src/player/mod.rs`),
jump dispatch (`crates/mbv-core/src/daemon_core.rs`, `src/app/action.rs`,
`crates/mbv-core/src/playback/transition.rs`), and the Playback run's
forced-jump/re-seek state (`crates/mbv-core/src/player/run/{types,commands,events,queue}.rs`).

## The invariant

1. Stay-Alive mode has **three independent copies of "the queue,"** each
   with its own item-position field, and none is automatically kept in sync
   with the others: the daemon's canonical `PlaybackQueue` (what every client
   projection/broadcast is built from), the Playback run's own
   `ExecutionSequence` (what actually drives mpv — resume lookups for
   `JumpTo`'s active-file branch and for relative `Next`/`Previous`), and each
   connected shell's own `PlaybackQueue` mirror (what the QueueList paints
   from).
2. A `PlayerEvent::TrackCompleted`/`Stopped` carrying `position_ticks`/
   `played` must be applied to **every** copy that a later read can reach, not
   just the one nearest the code being edited — otherwise the untouched
   copies stay stale, and a later broadcast/resync from a stale copy silently
   reverts a copy that *was* corrected.
3. Each application site currently re-derives its own "is this position worth
   recording" gate by hand (`played` → 0; `is_audio` or below a minimum →
   keep the prior value; otherwise → record) rather than sharing one
   function. They must be kept bit-for-bit identical by hand; nothing enforces
   that.
4. mpv bakes a playlist entry's `start=<seconds>` resume option in **once, at
   `loadfile` time**, and never re-reads or updates it afterward. Navigating
   back to that entry via `playlist-pos` reopens it from that frozen value,
   not from wherever it was last watched to. Anything that wants a re-visited
   entry to resume correctly must issue an explicit `seek absolute` — and
   only *after* mpv has actually loaded that entry, never immediately after
   the `playlist-pos` write (confirmed empirically: an immediate seek is
   rejected with `"error running command"` because no file is loaded yet;
   polling mpv's `seekable` property first works, and in practice
   `PlaybackRestart`'s one-shot "first restart of a freshly-loaded track"
   branch is a reliable proxy for "loaded and seekable").

## Why it matters

The user-visible shape of violating this: a video is watched to 86%, a
different item is played, and switching back to the first item starts it over
at 0% — both in what mpv actually plays and in what the QueueList displays.
Two independent defects produced exactly this symptom for Emby-only Stay-Alive
playback (fixed in 238ed4d8, f91120e2): the daemon discarding `TrackCompleted`/
`Stopped` position data instead of applying it to its canonical queue, and
mpv's one-shot `start=` option being replayed unchanged on every later jump
back to an entry regardless of what was actually watched.

## What breaks if it is violated

- **Canonical queue never advances.** `daemon_run.rs`'s `TrackCompleted` and
  `Stopped` handlers used to destructure `position_ticks`/`played` and drop
  them on the floor. The daemon's canonical queue stayed at its
  submission-time snapshot forever; a client's own locally-applied progress
  got silently overwritten by the *next* broadcast, which was still built
  from the stale canonical copy.
- **Fixing the canonical queue alone is not enough.** `PlaybackRun`'s own
  `ExecutionSequence` is a *third*, separate copy — `JumpTo`'s non-`active_file`
  handler used to resolve resume position by reading straight off it, so even
  after the canonical queue was correct, a `JumpTo` would still resume from
  whatever `ExecutionSequence` had captured at the last wholesale replace
  (`cmd_submit_queue`/`replace_with_queue_items`/etc — the only ways it is
  ever written). Fixed by having the *dispatcher* (which does have canonical-queue
  access) resolve `resume_ticks` and carry it on the command, rather than
  having the run trust its own copy for this.
- **Relative nav has no dispatcher to lean on.** `PlayerCommand::Next`/
  `Previous` (`step_to_index`, `commands.rs`) never round-trips through the
  daemon owner at all — it is handled entirely inside `PlaybackRun`. It
  therefore does not inherit the `JumpTo` fix and needs its own, independent
  progress application on `ExecutionSequence` so its local resume lookup has
  something current to read.
- **An un-armed seek lies to Emby.** Any seek that doesn't set `last_seek_at`
  lets `on_playback_restart` immediately report progress at the *pre-seek*
  position — for the resume-seek case, that means reporting position 0 to
  Emby in the same breath as resuming the item to 86%.
- **A stale pending re-seek lands on the wrong track.** The armed resume
  value (`forced_resume_ticks`) must be cleared everywhere `forced_slot_id`/
  `forced_transition` are cleared: a failed `playlist-pos` write,
  `QueueRemove` of the jump's target, `begin_item_lifecycle` (fresh
  submission), and `adopt_mpv_entry` (mpv diverging to an entry the run did
  not ask for). Missing any one of these lets a value armed for one entry get
  consumed by a different entry's first restart.

## How the code maintains it today

- **Progress-application call sites:** `daemon_run.rs`'s `TrackCompleted` arm
  and the generic `Stopped`-handling arm apply to the canonical queue via
  `PlayerOwnerState::apply_completion_progress`; `src/app/player_event.rs`'s
  `Stopped`/`TrackCompleted` arms apply to the shell's own mirror; `events.rs`'s
  `on_end_file` applies to the run's own `ExecutionSequence` via
  `ExecutionSequence::apply_progress`, at the same point it computes
  `completed_pos` for that occurrence.
- **One shared write path, several independent gates.**
  `crate::playback::queue::apply_progress_to_queue_item` is the single place
  that knows how to write a position/played pair into each `QueueItem` kind;
  both `PlaybackQueue`'s `apply_progress`/`ProgressState::apply_to_item` and
  `ExecutionSequence::apply_progress` call through it, so the *mechanics* of
  writing cannot diverge. The *decision* of what to write (the
  meaningful-progress gate) is still duplicated by hand in `daemon_run.rs`
  (x2) and `src/app/player_event.rs` (x2).
- **One resume gate, reused both ways.** `resume_start_pos`/
  `resume_ticks_for_item`/`resume_ticks_for_slot` (`player/mod.rs`) are the
  same gate `mpv_load_opts` bakes into a fresh `loadfile`'s `start=` option,
  so a resume computed at jump time always agrees with what a *fresh* load of
  the same item would have done.
- **`JumpTo` carries its own resume value.** `PlayerCommand::JumpTo` has a
  `resume_ticks: Option<i64>` field, resolved by the dispatcher
  (`daemon_core.rs`'s `dispatch_slot_jump`/`settle_and_redispatch`/
  `expire_and_redispatch`, `src/app/action.rs`'s `dispatch_jump` for Bare
  mode) from the *canonical* queue — never from `PlaybackRun`'s own copy.
- **`forced_resume_ticks` lifecycle.** Armed alongside `forced_slot_id` in
  `commands.rs`'s `JumpTo` and `step_to_index` handlers; taken and applied as
  an absolute `seek` inside `events.rs`'s `on_playback_restart`, specifically
  inside the one-shot `!tracks_initialized` branch (first restart of a
  freshly-loaded track); the same call also arms `last_seek_at` so the
  existing seek-settle guard suppresses a stale progress report until the
  seek actually lands; cleared in lockstep with `forced_slot_id`/
  `forced_transition` at every site those are cleared.

## Where it currently fails

1. **The meaningful-progress gate is triplicated by hand** (`daemon_run.rs`
   x2, `player_event.rs` x2) with no shared function and no compiler check
   that a future edit to one copy keeps the others in step. Only the
   `TrackCompleted` pair's *threshold* is shared today
   (`crate::api::MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS`, extracted after a
   review caught a comment on the `Stopped` arm falsely claiming "same
   reasoning as `TrackCompleted`" — a stray `Stopped`-carries-a-floor edit
   would have silently regressed resume accuracy for exactly the case this
   invariant exists to protect). The asymmetry itself is intentional and
   pre-existing: `TrackCompleted` fires mid-queue and needs a floor to reject
   startup noise, `Stopped` fires on a deliberate user action and records
   whatever position that happened at, no floor. `daemon_run.rs`'s copies
   also skip the shell's `mark_progress_sync_pending` call — harmless today
   only because the daemon never runs `merge_refresh` against its own queue;
   a latent trap if that ever changes.
2. **A narrow race remains:** dispatching a `JumpTo` back to an item before
   the daemon has processed that item's own preceding `TrackCompleted` reads
   the canonical queue's pre-completion value and resumes from there — the
   original symptom, narrowed to back-to-back fast switching between two
   items rather than the general case.
3. **`ExecutionSequence` and the canonical `PlaybackQueue` remain two
   independent copies with two independent progress-application paths** (one
   feeding `JumpTo`'s command-carried resume value, one feeding
   `step_to_index`'s local lookup). They agree today because both ultimately
   derive from the same `PlayerEvent`, but nothing enforces that a future
   change to one call site keeps them in step.

## Cheapest strengthening (not done here)

- Lift each gate's *full logic* (not just its threshold constant) into one
  `pub` function per event kind in `mbv-core` that both `daemon_run.rs` and
  `src/app/player_event.rs` call, so the daemon and the shell cannot drift on
  what counts as "worth recording" — today only the `TrackCompleted` pair's
  numeric floor is shared; the branching itself is still copy-pasted four
  times.
- Reconsider whether `ExecutionSequence` needs to carry progress at all:
  `step_to_index` is the one remaining place that trusts the run's own copy
  for a progress decision; everywhere else (`JumpTo`) now gets its resume
  value supplied by a dispatcher with canonical-queue access instead.

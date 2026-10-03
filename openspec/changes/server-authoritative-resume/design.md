# Design

## Context

See proposal.md, "Why". Here is what the code does today:

- **Every start position goes through one function.** `resume_start_pos(item)`
  (`crates/mbv-player/src/lib.rs:49`) is a pure function of the item. Its
  callers:
  - `mpv_load_opts` bakes `start=` for each entry when a whole queue is
    loaded (`run/commands/load.rs:143`) or appended
    (`run/commands/queue.rs:179`).
  - `PreparedSource::plain` (`sources.rs:129`) handles the thread-start load
    (`submit.rs` `load_queue_sources`) and active-slot preparation.
  - `resume_ticks_for_slot` runs on the daemon. It computes the `resume_ticks`
    that `JumpTo` carries to `cmd_jump_to` (`run/commands/queue.rs:12`). That
    value feeds `override_jump_resume` for active-file jumps and the
    re-visit seek in `apply_forced_resume` (`run/events/restart.rs`,
    invariant 6) for playlist jumps.
- **Audiobookshelf already resumes from the server.** `prepare_episode_source`
  and `prepare_book_source` use `session.current_time_seconds` from the opened
  session (`sources.rs:225`, `:284`). `override_jump_resume` skips
  Audiobookshelf.
- **Owner restore never asks Emby.** The Owner restores
  `stay_alive_queue_state.json` in `initialize_queue`
  (`crates/mbv-daemon/src/run.rs:323`). The Emby enrichment fetch
  (`start_queue_enrichment`, `control/queue_setup.rs:107`) runs only on
  client adoption and on a manual queue refresh, so the restored position was
  trusted as-is.
- **Shutdown persists before stopping.** `handle_shutdown`
  (`event_loop/control_events.rs:229`) persists, then calls `player.stop()`,
  then returns `SHUTDOWN`. The loop then calls `process::exit(0)`. The stop
  report runs on a detached thread that races the exit.
  `reconciliation.rs:32` already shows the bounded alternative:
  `stop_for_shutdown(remaining(deadline))` followed by `join_or_timeout`.
- **`get_items_by_ids`** (`mbv-emby/src/client_playlists.rs:284`) already
  requests `Fields=UserData`. It returns `Result`, so transport and parse
  failures are already distinct from "item has no position". A missing
  `PlaybackPositionTicks` parses as 0 (`types_parsing.rs:324`), which means
  "start from the beginning".

## Goals / Non-Goals

**Goals:**
- Use one rule for every Emby start-position decision on the Player thread.
- Keep the fetch cheap: one batched request per load or append, and skip audio
  entirely.

**Non-Goals:**
- Changing the shared Emby parser or the browse and enrichment paths. Those
  only feed progress bars now.
- Re-fetching when mpv advances on its own to an entry baked at load time (see
  Risks).
- Suppressing progress reports for a run that started from 0:00 after fetches
  failed. The user accepted that trade-off.
- An Audiobookshelf queue-row refresh on restore (see Risks).

## Decisions

### D1. The Player thread overwrites Emby positions with server values before deciding start positions

Add one helper to `mbv-player`, `refresh_emby_resume`. It takes the items whose
start position is about to be decided, collects the Emby non-audio item ids,
and calls `get_items_by_ids` once. It returns copies of those items with
`playback_position_ticks` (and `played`) set from the response. If every
attempt fails, those fields are set to 0/unplayed. All existing readers then
call `resume_start_pos` on the refreshed copies unchanged, so the 6% rule and
every downstream path stay as they are.

The helper is called on the Player thread in four places:

1. **Thread start:** `submit.rs` `load_queue_sources`, on `start.items`
   before the load loop.
2. **Queue load command:** `run/commands/load.rs`, on the items before the
   `loadfile` loop.
3. **Append:** `run/commands/queue.rs`, on `new_items` before the `loadfile`
   loop.
4. **Jump:** `cmd_jump_to`. For an Emby video slot, the fetched value (after
   `should_resume`) replaces the incoming `resume_ticks`. This one change
   covers active-file jumps (`override_jump_resume`) and playlist re-visits
   (`apply_forced_resume`). Feed slots keep the owner's `resume_ticks`.

The run's Emby client (the one `SessionReporter` holds) makes the call.

*Alternative considered:* fetch inside `resume_start_pos`. Rejected because it
is pure and is also called on the daemon event-loop thread, which must not
block on HTTP with retries.

*Alternative considered:* refresh the owner's canonical queue before
dispatching. Rejected because the daemon loop would block, and the value
could still be stale by the time the run uses it.

### D2. Retry with injected delays

`refresh_emby_resume` takes the fetch as a closure,
`FnMut(&[String]) -> Result<Vec<EmbyItem>, EmbyError>`, and takes the delay
schedule `[500ms, 2s]` as a parameter. That gives 3 attempts. An `Ok` that
omits a requested id counts as a failed attempt. Ids that succeed in one
attempt are not re-fetched; only the missing ids are retried. Production passes
`client.get_items_by_ids` and the real schedule. Tests pass a scripted closure
and zero delays, per the "no real sleeps" rule. This copies the shape of
`retry_mark_played` (`proxy.rs:167`), but runs inline because play needs the
answer before it starts.

### D3. Clear provider positions at the persistence boundary

`mbv-config`'s `save_queue_state`/`save_stay_alive_queue_state` and their
`load_*` counterparts map items through one `mbv-queue` function. That
function sets the Emby `playback_position_ticks` and the Audiobookshelf
`position_ticks` to 0, and leaves Feed entries untouched. Clearing on load
too means files written before this change cannot leak a stale position into
the display or into the feed-only paths. `played` is kept: it is watched-state
metadata, the next refresh corrects it, and clearing it could change consume
or prune behaviour, which is out of scope.

### D4. Refresh Emby progress on Owner restore

After the daemon loop is built (`run.rs`, after `initialize_queue`), if the
restored queue has Emby slots, run the same enrichment that adoption runs.
Expose `start_queue_enrichment` for this rather than adding a second fetch path.

### D5. Remove pending-sync and the max merge

Delete the following:
- `QueueSlot::pending_sync` and `SlotProgress` confirmation.
- `StopReportOutcome`.
- The `record_reported_progress` arming. Callers use the plain progress apply.
- The pending branch and the `max(stored)` clamp in `merge_fetched_slot`.
- `should_protect_missing_slot`'s pending clause.
- `progress_report_accepted` on `PlayerEvent::Stopped`, `TrackCompleted` and
  the ctrl wire.

The active-slot branch, which keeps the live position, stays. The ctrl field
was `#[serde(default)]` and ctrl does not deny unknown fields, so mixed-version
peers still parse each other.

### D6. Shutdown: bounded stop, then persist

In `handle_shutdown`:
1. Notify clients and flush writers, as now.
2. `stop_for_shutdown(remaining(deadline))` and
   `join_or_timeout(remaining(deadline))`.
3. Flush the persist queue and `persist_owner_queue()`.
4. Return `SHUTDOWN`.

Reuse an existing shutdown bound constant if one fits (for example the one
`reconciliation.rs` uses). Otherwise add one named constant of about 5s,
matching today's join timeout. Persisting after the stop matters less now,
because positions are no longer persisted, but it is free and keeps the
snapshot's active slot consistent with the stopped player.

## Risks / Trade-offs

- **A natural advance uses the position fetched when the queue was loaded.**
  When mpv moves on to an entry whose `start=` was baked at load, it uses the
  position fetched then. If that same item was played on another machine
  during this run, the start is stale. → Accepted (`ponytail:` comment at the
  load site). The upgrade path is to fetch and seek in the entry-activation
  handler.
- **Going back right after leaving an Emby entry.** The stop/completion report
  is asynchronous, so a jump back within milliseconds could read Emby's older
  value. → Accepted. Human-speed navigation on a reachable server is orders of
  magnitude slower than the report, and the 10s progress pings bound the gap.
- **Three failed fetches, then a stop report.** A run that started from 0:00
  after three failed fetches reports normally and can overwrite Emby's position.
  → Accepted by the user: it needs three consecutive failures against a
  reachable server.
- **Audiobookshelf queue rows lose their progress bars after a restart.** They
  show no progress until Audiobookshelf reports some (a play, or Socket.IO in
  the TUI). → Accepted; display only. Follow-up if it bothers.
- **Play waits on an extra request.** Each load, append or jump of an Emby
  video waits for one more round trip (about 3ms on LAN). → Batched per load,
  and skipped for audio.

## Migration Plan

None. Old state files load with positions cleared (D3). Rollback is a revert.

## 1. New state module

- [ ] 1.1 Create `crates/mbv-core/src/player_session_state.rs` with
      `LoadState`/`DrainResult`, `StopReport`, `NextUp`, `IntroState`,
      `StartupPause` per design.md's Decisions section (types + the listed
      methods only - no extra methods beyond what call sites need).
- [ ] 1.2 Register the new module (`mod player_session_state;`) alongside
      the existing `player_session_*` module declarations.

## 2. Struct and construction

- [ ] 2.1 In `player_session_types.rs`, change `PlaybackSession`'s field
      types: `pending_load: LoadState`, `stop_reported`+
      `stop_report_accepted` → single `stop_report: StopReport`,
      `next_up_fired`+`next_up_armed` → `next_up: NextUp`,
      `queue_next_up_fired`+`queue_next_up_armed` → `queue_next_up: NextUp`,
      `intro_show`+`intro_hide` → `intro: IntroState`,
      `startup_pause_release_pending`+`startup_pause_events_to_skip` →
      `startup_pause: StartupPause`.
- [ ] 2.2 In `player_session_queue.rs::new`, construct the new fields:
      `LoadState::Ready`, `StopReport::NotSent`, `NextUp::Idle` (both
      instances), `IntroState` from `past` (Dismissed if true else Pending),
      `StartupPause::holding(2)` / `StartupPause::none()` based on
      `startup_pause_for_pipe`.
- [ ] 2.3 In `player_session_queue.rs::set_intro`, replace the
      `intro_show`/`intro_hide` assignment with
      `self.intro = if past { IntroState::Dismissed } else { IntroState::Pending }`.
- [ ] 2.4 In `player_session_queue.rs::load_active_item_state`'s early-return
      branch (no active item), replace the `intro_show`/`intro_hide` reset
      with `self.intro = IntroState::Pending`.

## 3. Reset-site consolidation

- [ ] 3.1 Add `begin_item_lifecycle(&mut self)` to `PlaybackSession` (in
      `player_session_commands.rs` or `player_session_queue.rs`, whichever
      keeps `impl PlaybackSession` blocks organized similarly to today)
      covering the group identified in design.md: `pending_initial_jump =
      false`, `tracks_initialized = false`, `forced_slot_id = None`,
      `next_up = NextUp::Idle`, `queue_next_up = NextUp::Idle`,
      `next_up_jump = false`, `stopped_event_sent = false`,
      `mark_played_id = None`, `stopped_near_end = false`. Do **not**
      include `StopReport` or `LoadState` - both vary per call site.
- [ ] 3.2 Update `cmd_replace_queue`'s empty-items branch: call
      `self.stop_report = StopReport::NotSent` is wrong here - this branch
      reports and marks sent (`self.stop_report =
      StopReport::Sent { accepted: self.reporter.report_stopped(...) }`),
      sets `self.pending_load = LoadState::Ready`, and calls
      `begin_item_lifecycle()` for the rest of the group.
- [ ] 3.3 Update `cmd_replace_queue`'s non-empty branch similarly:
      `self.stop_report = StopReport::Sent { accepted: ... }`,
      `self.pending_load = LoadState::begin_replace(start_idx)`, and
      `begin_item_lifecycle()` for the rest.
- [ ] 3.4 Update `cmd_load_new`: `self.stop_report = StopReport::NotSent`,
      `self.pending_load = LoadState::begin_single()`, and
      `begin_item_lifecycle()` for the rest.

## 4. Event-loop call sites

- [ ] 4.1 `on_playlist_pos_changed`'s guard: replace `self.pending_load > 0`
      with `self.pending_load.is_pending()`.
- [ ] 4.2 `on_playback_restart`'s pending-initial-jump branch: replace
      `self.pending_load += 1` with
      `self.pending_load = LoadState::begin_single()`.
- [ ] 4.3 `on_playback_restart`'s `startup_pause_release_pending` check:
      replace with `if self.startup_pause.take_release() { ... }`.
- [ ] 4.4 `on_playback_restart`'s standalone-origin branch: replace
      `self.next_up_fired = false; self.next_up_armed = false;` with
      `self.next_up = NextUp::Idle`.
- [ ] 4.5 `on_time_pos`'s queue next-up block: replace the
      `queue_next_up_fired`/`queue_next_up_armed` reads/writes with
      `self.queue_next_up` state checks (`is_fired`, `arm`, `fire`, reset to
      `Idle`), preserving the exact same condition structure.
- [ ] 4.6 `on_time_pos`'s standalone next-up block: same substitution for
      `self.next_up`.
- [ ] 4.7 `on_time_pos`'s `handle_intro` call: pass `&mut self.intro`
      instead of `&mut self.intro_show, &mut self.intro_hide`.
- [ ] 4.8 `on_end_file`'s pending-load guard: replace the
      `if self.pending_load > 0 { ... }` block with a match on
      `self.pending_load.drain()` - `StillPending` returns `true` as today;
      `JustCompleted` sets `self.stop_report = StopReport::NotSent` and
      returns `true`; `NotPending` falls through to the rest of the
      function unchanged.
- [ ] 4.9 `on_end_file`'s remaining `stop_reported`/`stop_report_accepted`
      reads and writes (Queue+Quit branch, Standalone branch, the two
      `progress_report_accepted: self.stop_report_accepted` event fields,
      the out-of-bounds `completed_idx` branch, the advance-path
      `stop_report_accepted` local): replace with
      `self.stop_report.is_sent()` / `self.stop_report.mark_sent(accepted)`
      / `self.stop_report.accepted()` as appropriate. Where the code
      currently assigns a local `let stop_report_accepted = ...` for the
      advance path (not the session-level field), leave that local as a
      plain bool - it's a different value than the struct field.
- [ ] 4.10 `on_end_file`'s queue_next_up reset (`queue_next_up_fired =
      false; queue_next_up_armed = false;` near the track-transition path):
      replace with `self.queue_next_up = NextUp::Idle`.
- [ ] 4.11 `on_shutdown`: replace the `stop_reported`/`stop_report_accepted`
      reads/writes with the `StopReport` equivalents.

## 5. Startup-pause skip counter

- [ ] 5.1 In `player_session_run.rs`'s `pause` `PropertyChange` handler,
      replace the `startup_pause_events_to_skip > 0` / `-= 1` pair with
      `if self.startup_pause.skip_event() { continue; }`.

## 6. `handle_intro` signature

- [ ] 6.1 In `player_runtime.rs`, change `handle_intro`'s `show_fired: &mut
      bool, hide_fired: &mut bool` params to a single `intro: &mut
      IntroState`, and rewrite its body as the `Pending`/`Shown`/`Dismissed`
      transitions described in design.md (including the direct
      `Pending → Dismissed` seek-past-intro case, which must not emit
      `IntroStarted`/`IntroEnded`).

## 7. Tests

- [ ] 7.1 Update `player_tests_session.rs:45` (and any other direct
      `pending_load = 1` / `stop_reported = ...` / flag-field assignments in
      tests) to use the new constructors/methods instead of raw field
      writes.
- [ ] 7.2 `rtk cargo test -p mbv-core` passes unchanged - no test content
      changes beyond construction/assertion call sites touched by the type
      change.

## 8. Verify enum-extraction commit

- [ ] 8.1 `rtk cargo check -p mbv-core` clean.
- [ ] 8.2 `rtk cargo test -p mbv-core` passes.
- [ ] 8.3 `rtk cargo clippy --workspace --all-targets` clean.
- [ ] 8.4 `make check-code-file-lines` passes (watch
      `player_session_types.rs`, which loses lines, and the new
      `player_session_state.rs`).
- [ ] 8.5 Commit the enum/state-machine extraction as its own commit.

## 9. Mechanical rename

- [ ] 9.1 Rename `PlaybackSession` → `PlaybackRun` and `MpvSessionConfig` →
      `MpvRunConfig` (all references, including
      `player_runtime_controller.rs` and `player_tests_basic.rs`).
      `SessionReporter` keeps its name.
- [ ] 9.2 Rename files: `player_session_types.rs` → `player_run_types.rs`,
      `player_session_commands.rs` → `player_run_commands.rs`,
      `player_session_queue.rs` → `player_run_queue.rs`,
      `player_session_run.rs` → `player_run_run.rs` (or a clearer name if
      `run_run` reads badly - e.g. `player_run_loop.rs`; use judgment, this
      is naming, not behavior), `player_session_events.rs` →
      `player_run_events.rs`, `player_session_state.rs` →
      `player_run_state.rs`. Update `mod` declarations accordingly.
- [ ] 9.3 Add "Playback run" to `CONTEXT.md`'s glossary, immediately after
      the existing "Playback continuity" entry: local mpv playback loop, one
      per mpv invocation, distinct from Session; `_Avoid_: session, playback
      session`. Glossary entry only, no implementation detail.

## 10. Verify rename commit

- [ ] 10.1 `rtk cargo check -p mbv-core` clean.
- [ ] 10.2 `rtk cargo test -p mbv-core` passes.
- [ ] 10.3 `rtk cargo clippy --workspace --all-targets` clean.
- [ ] 10.4 `make check-code-file-lines` passes.
- [ ] 10.5 Commit the rename as its own commit, separate from Section 8's
      commit.

## 11. Manual verification

- [ ] 11.1 Play a queue, skip mid-item, stop near the end; confirm the
      `stopped_near_end` → `played`/`consume` path
      (`player_session_events.rs` / renamed `player_run_events.rs`) still
      drives Emby watched-status correctly. No automated test covers this
      server round-trip.

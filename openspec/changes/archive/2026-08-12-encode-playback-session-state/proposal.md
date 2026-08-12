## Why

`PlaybackSession` (`crates/mbv-core/src/player_session_types.rs`) carries 42
fields, 15 of which are booleans/counters that together encode an implicit
state machine (load-pending count, stop-report handshake, next-up arming,
intro visibility, startup-pause holdoff). The type permits every combination,
so the legal ones are asserted only in comments at the reset sites — e.g.
"Use `=` not `+=` so a stale `pending_load` never stacks"
(`player_session_commands.rs:235`) and "draining to 0 re-arms stop
reporting" (`player_session_events.rs:238-244`). Comments rot silently;
nothing currently stops a new reset site from setting one flag in a pair and
forgetting its partner. The reset sites already show the real grouping —
`stopped_event_sent`+`stopped_near_end` reset together in three places
(`player_session_commands.rs:192-194`, `253-255`, `339-345`),
`next_up_fired`+`next_up_armed` together (`player_session_events.rs:207-208`),
`intro_show`+`intro_hide` always assigned identically (three sites in
`player_session_queue.rs`) — encoding these groups as enums with transition
methods makes the compiler enforce what the comments currently only claim.

Separately, `PlaybackSession` is also a misnomer that collides with the
Emby-tracked "Session" (see `CONTEXT.md`'s glossary, which reserves that word
for the server-side record that exists independently of mbv). The local mpv
playback loop needs its own name so the two concepts stop reading as one.

## What Changes

- Add `crates/mbv-core/src/player_session_state.rs` with five small enums
  replacing the flag groups above, each exposing transition methods instead
  of public fields:
  - `LoadState { Ready, Pending(NonZeroU8) }` — replaces `pending_load: u8`,
    with `begin_replace`, `begin_single`, and `drain` (whose return value
    tells the caller whether the count hit zero, so the stop-report reset
    can't be forgotten).
  - `StopReport { NotSent, Sent, Accepted }` — replaces
    `stop_reported` + `stop_report_accepted`.
  - `NextUp { Idle, Armed, Fired }` — replaces `next_up_fired` +
    `next_up_armed` (and the equivalent `queue_next_up_*` pair).
  - `IntroState { Pending, Shown, Dismissed }` — replaces `intro_show` +
    `intro_hide`.
  - `StartupPause { None, Holding { events_to_skip: u8 } }` — replaces
    `startup_pause_release_pending` + `startup_pause_events_to_skip`.
- Update `PlaybackSession`'s field list and every read/write site
  (`player_session_commands.rs`, `player_session_events.rs`,
  `player_session_queue.rs`, `player_session_run.rs`, `player_proxy.rs:408`,
  `player_runtime.rs:592` `handle_intro`) to use the new types.
- Collapse the three duplicated reset sites in `player_session_commands.rs`
  into one `begin_item_lifecycle()` method.
- **BREAKING** (internal API only, no external interface affected): rename
  `PlaybackSession` → `PlaybackRun`, `MpvSessionConfig` → `MpvRunConfig`, and
  the five `player_session_{types,commands,queue,run,events}.rs` files to
  `player_run_*.rs`, as a separate commit from the enum work.
  `SessionReporter` (`player_runtime.rs:195`) keeps its name — it reports to
  the Emby Session, the one place "Session" is the correct word.
- Add a **Playback run** glossary entry to `CONTEXT.md` (the local mpv
  playback loop, one per mpv invocation, distinct from Session), with
  `_Avoid_: session, playback session`.

## Capabilities

No spec-level behavior changes — playback, queueing, and reporting to Emby
behave identically before and after. This is an internal refactor: the same
transitions become compiler-checked instead of comment-documented, and two
files/types get clearer names. `skip_specs: true` is set in
`.openspec.yaml`.

## Impact

- **Primary files**: `player_session_types.rs` (struct → enums),
  `player_session_events.rs` (most transitions), `player_session_commands.rs`
  (reset sites → `begin_item_lifecycle()`), `player_session_queue.rs`
  (construction), `player_session_run.rs` (event loop). New:
  `player_session_state.rs` (pre-rename) / `player_run_state.rs`
  (post-rename).
- **Call sites taking these flags as parameters**: `player_proxy.rs:408`
  (`quit_timeout_stop_flags`), `player_runtime.rs:592` (`handle_intro`).
- **Tests**: `player_tests_session.rs` and `player_tests_status.rs` are the
  regression net and must pass unchanged.
  `player_tests_session.rs:45` sets `pending_load = 1` directly and needs a
  constructor call instead.
- **Docs**: `CONTEXT.md` gains one glossary entry.
- **No protocol, wire-format, or user-facing behavior change.** `ctrl.rs` and
  `api_types.rs` are untouched.
- **Not in this change** (tracked as follow-ups, not started here): ID
  newtypes for `ItemId`/`MediaSourceId`/`EmbySessionId`; deduplicating
  `is_queue_mode` against `origin == PlaybackOrigin::Queue`; a shared
  lang-code table; a `render_power_queue*` → `render_queue*` rename; a
  post-tool-use `rustfmt` hook.

## Context

See `proposal.md` - Why. Current code (verified against `crates/mbv-core/src/
player_session_{types,commands,events,queue}.rs` and `player_runtime.rs` at
the time of writing this design):

- `pending_load: u8` gates `on_playlist_pos_changed`/`on_end_file` and is
  assigned (not incremented) at three sites, and incremented once in
  `on_playback_restart` when `pending_initial_jump` fires (guaranteed 0 at
  that point, so an unconditional "set to 1" is equivalent to `+= 1` there).
- `stop_reported`/`stop_report_accepted` are always assigned together, but
  the assigned value differs by call site: `cmd_load_new` resets to
  `(false, false)`; `cmd_replace_queue` sets `(true, <computed accepted>)`
  immediately and relies on `pending_load` draining to 0 (in `on_end_file`)
  to reset them back to `(false, false)`.
- `next_up_fired`/`next_up_armed` (standalone next-up) and
  `queue_next_up_fired`/`queue_next_up_armed` (queue next-up) are two
  independent instances of the same 3-state shape (idle → armed → fired),
  not one combined group. `reset_next_up_state` resets both instances plus
  the unrelated one-shot `next_up_jump` bool together, but they progress
  independently at runtime (`on_time_pos` drives the queue instance,
  `on_playback_restart` drives the standalone one).
- `intro_show`/`intro_hide` are always assigned identically at the three
  reset/init sites, and `handle_intro` (`player_runtime.rs:518`) drives them
  through `&mut bool` params in a real 3-state progression: both false
  (pending) → show true, hide false (intro showing) → both true
  (dismissed). A direct seek past the intro window can jump straight from
  pending to dismissed without an intermediate "showing" state.
- `startup_pause_release_pending` and `startup_pause_events_to_skip` are set
  together at construction (both derived from `startup_pause_for_pipe`), but
  clear at **different, unrelated call sites**:
  `startup_pause_release_pending` clears unconditionally on the first
  `PlaybackRestart` (`player_session_events.rs:174-175`);
  `startup_pause_events_to_skip` decrements independently on each `pause`
  `PropertyChange` event (`player_session_run.rs:73-74`), and is not touched
  by the `PlaybackRestart` handler at all. The issue's proposed
  `enum StartupPause { None, Holding { events_to_skip } }` does not fit this:
  collapsing both into one enum would make clearing `release_pending` also
  clear `events_to_skip`, silently changing behavior (events meant to be
  skipped would stop being skipped one call early).
- `player_proxy.rs:408` (the issue's citation) is a `stopped_near_end: bool`
  parameter of `quit_timeout_stop_flags` — a plain derived bool, not one of
  the five flag groups below. It needs no signature change; the issue's
  file list overstates its involvement.

## Goals / Non-Goals

**Goals:**
- Every flag group identified in the issue becomes a type whose only
  reachable operations are the transitions the current comments describe.
- `cargo test -p mbv-core` passes unchanged - this is a behavior-preserving
  refactor.
- The rename (`PlaybackSession` → `PlaybackRun`, file renames) lands as a
  separate commit from the enum extraction.

**Non-Goals:**
- Newtyping `stopped_near_end`, `pending_initial_jump`, `forced_slot_id`, or
  any other single-instance flag not identified as a *group* in the issue.
- Touching `is_queue_mode`/`origin` duplication, ID newtypes, the lang-code
  table, or the `power` prefix - explicitly deferred follow-ups in the
  issue.
- Changing `on_end_file`/`on_shutdown` control flow beyond substituting
  enum-method calls for raw field reads/writes.

## Decisions

### `LoadState` (replaces `pending_load: u8`)

```rust
enum LoadState {
    Ready,
    Pending(NonZeroU8),
}
impl LoadState {
    fn begin_single() -> Self;               // was: = 1 (cmd_load_new, on_playback_restart)
    fn begin_replace(start_idx: usize) -> Self; // was: = if start_idx > 0 { 2 } else { 1 }
    fn is_pending(&self) -> bool;             // was: pending_load > 0 (on_playlist_pos_changed guard)
    fn drain(&mut self) -> DrainResult;       // was: -= 1; check == 0 (on_end_file)
}
enum DrainResult { NotPending, StillPending, JustCompleted }
```
`on_playback_restart`'s `pending_load += 1` becomes `self.pending_load =
LoadState::begin_single()` - safe because `pending_initial_jump` being true
at that point guarantees `pending_load` is currently `Ready`; reusing the
same constructor avoids introducing a second "increment" method for one call
site. Alternative considered: add `bump()` for that one call site - rejected
as an extra public operation for a single, provably-zero-start case.

### `StopReport` (replaces `stop_reported: bool` + `stop_report_accepted: bool`)

```rust
enum StopReport {
    NotSent,
    Sent { accepted: bool },
}
impl StopReport {
    fn mark_sent(&mut self, accepted: bool); // was: stop_report_accepted = x; stop_reported = true;
    fn is_sent(&self) -> bool;
    fn accepted(&self) -> bool;              // was: reading stop_report_accepted directly
}
```
`LoadState::drain` reaching `JustCompleted` resets this to `NotSent` (was:
the two-line reset inside `on_end_file`'s `pending_load == 0` branch) -
`DrainResult` is consumed by the caller, which calls
`self.stop_report = StopReport::NotSent` explicitly; `LoadState` itself
stays ignorant of `StopReport` to keep the two types independent, matching
how `QueueSlotId` doesn't know about the queue it indexes.

### `NextUp` (replaces both next-up flag pairs - two instances, not one type)

```rust
enum NextUp { Idle, Armed, Fired }
impl NextUp {
    fn arm(&mut self);   // was: next_up_armed = true (only from Idle in practice)
    fn fire(&mut self);  // was: next_up_fired = true
    fn is_fired(&self) -> bool;
    fn is_armed(&self) -> bool;
}
```
`PlaybackRun` gets two fields, `next_up: NextUp` and `queue_next_up: NextUp`
- not one shared type used once, because the two progress independently
(confirmed above). `next_up_jump: bool` is unrelated to this enum (a
one-shot "was this advance triggered by next-up" flag consumed via
`mem::replace` in `on_end_file`) and stays a plain bool; it is reset
alongside both `NextUp` instances in the lifecycle-reset call, but that is a
call-site grouping, not a type-level one.

### `IntroState` (replaces `intro_show: bool` + `intro_hide: bool`)

```rust
enum IntroState { Pending, Shown, Dismissed }
```
`handle_intro` (`player_runtime.rs:518`) changes from two `&mut bool`
params to one `&mut IntroState`, matching the three real transitions:
`Pending → Shown` (intro window entered, not yet skipped-through),
`Shown → Dismissed` (intro window ended), and `Pending → Dismissed` directly
(a seek lands past the intro in one step - no `IntroStarted`/`IntroEnded`
event pair fires, matching current behavior). Construction and the two reset
sites (`player_session_queue.rs:134-135`, `259-260`, `272-273`) collapse to
`IntroState::Dismissed` when `past` is true, `IntroState::Pending`
otherwise.

### `StartupPause` (replaces `startup_pause_release_pending: bool` + `startup_pause_events_to_skip: u8`)

The issue's single-enum sketch doesn't fit (see Context) - the two pieces
clear independently. Model as a struct, not an enum, with two separately
consumable pieces:

```rust
struct StartupPause {
    release_pending: bool,
    events_to_skip: u8,
}
impl StartupPause {
    fn holding(events_to_skip: u8) -> Self;  // was: both fields set from startup_pause_for_pipe
    fn none() -> Self;                        // was: both fields false/0
    fn take_release(&mut self) -> bool;       // was: `if release_pending { release_pending = false; ... }`
    fn skip_event(&mut self) -> bool;         // was: `if events_to_skip > 0 { events_to_skip -= 1; true } else { false }`
}
```
Alternative considered: two independent types (`StartupPauseRelease` enum +
a `SkipBudget(u8)` newtype). Rejected - both are only ever constructed
together from the same `startup_pause_for_pipe` bool, so a single struct
keeps that pairing visible at the construction site while still letting the
two consumers clear independently, which is the actual invariant.

### Reset-site consolidation

The three reset sites in `player_session_commands.rs` (lines ~188-199,
~237-255, ~331-339) assign the same group - `pending_initial_jump`,
`LoadState`, `tracks_initialized`, `forced_slot_id`, `NextUp` (both
instances) + `next_up_jump`, `stopped_event_sent`, `mark_played_id`,
`stopped_near_end` - become one method, e.g. `begin_item_lifecycle(&mut
self)`, called from all three sites. `StopReport` is deliberately **not**
included in this method: `cmd_load_new` resets it to `NotSent` while
`cmd_replace_queue`'s two sites set it to `Sent{..}` - different values per
call site, so it stays an explicit assignment at each site rather than a
hidden default inside the shared method.

### File split

New `crates/mbv-core/src/player_session_state.rs` (pre-rename) /
`player_run_state.rs` (post-rename) holds all five types plus their impls.
Kept under the 800-line cap trivially (~150-200 lines expected).

### Rename ordering

Enum extraction commit first, then the `PlaybackSession` → `PlaybackRun`
rename commit, matching the issue's stated order ("so the mechanical rename
doesn't bury the semantic change in review") - the enum commit is the one
worth reading closely; doing it first means the rename commit is a pure
diff-of-renames with no logic changes to double-check.

## Risks / Trade-offs

- [`LoadState::begin_single()` reused for both `cmd_load_new` and the
  `on_playback_restart` increment site could mask a future bug if
  `pending_load` is ever non-zero at the restart site] → mitigated by
  `NonZeroU8`/`Ready` making `begin_single()` an unconditional overwrite
  either way (matches current `+=` behavior only because it's provably 0
  there today; if that invariant ever breaks, the overwrite silently drops a
  pending count instead of panicking - same risk profile as today's `+=`
  bug class, not worse).
- [Collapsing `next_up_fired`/`next_up_armed` into `NextUp` could tempt a
  future edit to merge the standalone and queue instances into one field]
  → mitigated by naming (`next_up` vs `queue_next_up`, not `next_up[0]`/
  `next_up[1]`) and this design doc recording why they're separate.
- [`cargo check -p mbv-core` will show many errors at once during the
  transition] → expected per the issue; work file-by-file
  (`player_session_state.rs` first, then each caller file) rather than
  attempting a single atomic edit.

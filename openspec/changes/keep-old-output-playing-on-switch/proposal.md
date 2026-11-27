# Proposal

## Why

The "Just switch" answer from `switch-output-without-hand-off` stops the old output. A user who
switches away from a Cast receiver or an Emby Session does not ask to stop it. The user asks to
reach the new target. Today the switch also stops the device the user leaves, so audio that is
playing there ends as a side effect of a selection. Switching sessions must never stop the output
that the user switches away from.

## What Changes

- **A switch releases a remote output and does not stop it.** When the Owner performs an output
  switch away from a Cast receiver or an Emby Session, it stops driving that device and sends it
  no stop: the media is not stopped and the receiver app is not torn down. The device keeps
  playing what it plays. The Owner stops tracking it.
- **The Owner still has one output.** The target becomes the output, the queue is unchanged, and
  nothing starts on the target. Only the old device is left alone.
- **Local playback is the one exception.** The Owner hosts the local run, so it cannot leave it
  playing while another output is active. A switch away from Local still stops the local run.
- **Unchanged.** The continue answer (a hand-off) still stops the old output, because the slot
  moves to the target. A selection with no prompt, Stop, and every Client sever are unchanged.
  The ctrl command and the `output-switch` capability are unchanged: no protocol change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `owner-playback-output`: an output switch away from a Cast receiver or an Emby Session leaves
  that device playing. This supersedes the "old output stops" outcomes that the unmerged
  `switch-output-without-hand-off` change adds for a switch.

## Impact

- **Code:** `mbv-daemon` only: the old-output stop on an output swap
  (`output/handoff.rs`, `cast.rs`, `cast/connect.rs`, `emby_session/connect.rs`) learns a release
  mode that a `SwitchOnly` intent selects. The Client and ctrl crates do not change.
- **Docs:** invariant 31, `CONTEXT.md` (*Output switch*), and the prompt text of the owner spec.
- **Ordering:** this change builds on PR #1072. It must land after that change merges, and its
  delta syncs after that change's delta.
- **Not in scope:** Local-to-remote switches keep stopping the local run; the continue answer; a
  way to resume control of a released device.

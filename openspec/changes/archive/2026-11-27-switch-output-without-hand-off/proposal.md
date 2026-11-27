# Proposal

## Why

A user who selects a playback target while a slot plays cannot switch to that target without
moving the playing slot there. The Client asks "continue playback on the target?", and for a Cast
receiver, an Emby Session with no direct endpoint, and the local target, the No answer cancels the
whole selection. The user never reaches the target. Only an Emby Session that advertises a direct
endpoint has a fork where both answers act. The earlier `confirm-output-handoff` change added the
question because audio moved with no question, and it made No a cancel. The result is a gate that
protects against a hand-off and blocks every other use of the target.

## What Changes

- **A second answer that acts.** Every selection that would hand off an active slot (a Cast
  receiver, an Emby Session with no direct endpoint, the local target while a remote output plays)
  offers two answers that both act: continue the slot on the target, or switch to the target and
  hand nothing off. The old output stops, the target becomes the output, the queue is unchanged,
  and nothing starts playing on the target. The prompt has no cancel answer, as the direct-endpoint
  fork has none today.
- **An Owner switch with no hand-off.** The Owner gets a new ctrl command that selects an output
  and captures no hand-off. A Client-only Stop followed by a normal selection is not enough: the
  local run reports its status to the Owner asynchronously, so the selection can still read the
  slot as active and continue it on the target, which is the outcome the user declined.
- **Capability gate.** The new command is additive and capability-gated. A Client that finds the
  capability missing on the local Owner keeps today's prompt, where No cancels.
- **Unchanged.** The direct-endpoint fork keeps its answers. Queue Hand off and Take are untouched.
  A selection with no active slot, or of the device that already is the output, still runs with no
  prompt. The Owner's hand-off semantics for an unasked or confirmed selection are unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `owner-playback-output`: the Client prompt for a selection that would hand off gets a second
  answer that acts, and the Owner gains an output switch that hands nothing off.
- `ctrl-protocol`: a new additive command and capability for that switch, with no protocol-version
  bump.

## Impact

- **Code:** `mbv-ctrl` (the command, the capability, the Hello accessor), `mbv-daemon` (the switch
  handler: skip the hand-off capture on the three selection paths and the remote-to-local path),
  `mbv-remote-player` (the capability record and the send), `mbv-client-core` (the shared prompt,
  its confirm answer, and the three selection sites), `mbv-ui-model` (the confirm action).
  Both Clients share the App dispatch, so neither needs a second arm.
- **Protocol:** one additive `CtrlCmd` variant and one capability within protocol version 10. The
  v10 fixtures and schemas are regenerated.
- **Not in scope:** the "play on this machine instead?" prompt for an audio-only owner, the Queue
  Hand off and Take refusals, and the "replace the queue?" prompts. They are a different relation
  (queue content, not output selection) and are not traced here.

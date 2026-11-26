# Design

## Context

See `proposal.md` for the motivation. The facts that shape the approach:

- The Client gate is one shared prompt in `mbv-client-core` (`dispatch/session/output_handoff.rs`).
  Three sites raise it: the Cast selection (`dispatch/cast.rs`), an Emby Session with no direct
  endpoint (`dispatch/session/switch.rs`, `activate_session_target`), and the local target
  (`state/remote_slot.rs`, `detach_to_local`). Its answer type is
  `ConfirmAction::ContinueOutputHandOff`, where Enter runs the selection and every other key
  cancels. The GUI Client and the TUI both reach it through `App`, so no Client needs its own arm.
- An Emby Session with a direct endpoint already has a fork whose two answers both act
  (`ConfirmAction::HandOffPlaybackToSession`: Enter hands off, Esc switches). It is the model for
  the new prompt.
- The Owner already has a "no hand-off" swap: when a selection captures nothing active, it stops
  the old output, installs the target, and starts nothing (`owner-playback-output`, "Nothing is
  active"). The Owner decides what to continue from `capture_handoff_for_selection`, which reads
  the local run's status. That status updates asynchronously after `Stop`.
- Two Owner guards reject a swap that has no captured hand-off while the old output is still
  active: `apply_connected_output` for Cast (`mbv-daemon/src/cast/connect.rs`) and
  `install_output` for an Emby Session (`emby_session/connect.rs`). Their message is "playback
  started while connecting; stop playback before moving it". They protect a selection made while
  idle from discarding playback that started a moment later.
- `docs/invariants/31-severing-a-target-stops-no-playback.md`: a sever sends the outgoing owner no
  playback command, and a hand-off is the owner's own output selection that the user answered
  first. `docs/invariants/25-observed-active-slot-follows-the-output.md`: every path that changes
  what an output plays records the observed slot and publishes it.
- No new dependency and no version change: the change uses crates that already build.

## Goals / Non-Goals

**Goals:**

- The user can switch to a Cast receiver, an Emby Session, or local playback while a slot plays,
  and decline to move the slot.
- The decline answer is race-free: it never moves the slot by accident.

**Non-Goals:**

- The Emby Session direct-endpoint fork keeps its answers and its code.
- Queue Hand off and Take, the audio-only "play on this machine" prompt, and the queue
  replacement prompts keep their behavior. The proposal names them out of scope.
- No change to the Owner's hand-off for an unasked or a confirmed continue selection.
- No new protocol version.

## Decisions

**D1. A decline stops the old output, makes the target the output, and plays nothing.** An Owner
has exactly one output (`owner-playback-output`), so "leave the playing slot where it is" and "the
target is the output" cannot both hold. The user chose the target, so the target wins and the old
output stops. The queue and its order stay; the target holds no slot and the user plays one from
the queue. *Alternative:* leave the old output playing and only change the Client's view of the
target. That is the direct-endpoint fork's behavior and it works there only because a direct
endpoint is another Owner the Client can attach to. A plain Emby Session or a Cast receiver has no
Owner to attach to.

**D2. Both answers act, and Esc is the switch answer.** The new prompt mirrors the direct-endpoint
fork: Enter continues the slot on the target, Esc switches only. There is no cancel. Picking a
row is already a deliberate act, and a mis-selection costs one more selection. *Alternatives:* a
three-button prompt with a cancel (rejected: it keeps the extra decision the user objected to);
no prompt at all (rejected: the `confirm-output-handoff` change exists because audio moved with no
question). The button labels are Continue and Just switch, and the title keeps naming the
target. The labels are not specified.

**D3. The Owner performs the switch. The Client does not compose `Stop` then `SelectPlaybackOutput`.**
`Stop` on the local output calls `player.stop()` and the run's status falls to inactive later.
A selection sent right after still reads the slot as active and carries it to the target, which
is the outcome the user declined. The Owner's connect-time guards would also reject the swap, so
the failure would show as a rejection toast instead. A Client wait that holds the selection
until the status falls would add a timeout and a settlement state to the shared core for a race
the Owner can remove by itself.

**D4. A new `CtrlCmd::SwitchPlaybackOutput(OutputSelection)` with the capability
`output-switch`, not a flag on `SelectPlaybackOutput`.** `SelectPlaybackOutput` is a tuple variant
of one field. A field cannot be added without making it a struct variant, which changes the wire
form of every existing selection. A new variant is additive, so there is no version bump
(`docs/recipes.md`, "Add a ctrl protocol variant", step 1). The command carries the same
`OutputSelection`, so the target shapes, the fixtures for them, and the Client's target
construction are shared. The Owner advertises the capability when it advertises `playback-output`
or `emby-session-output`: a runtime that hosts a remote output can switch to it. Packaged `mbvd`
hosts none and advertises neither.

**D5. The Owner threads a `SelectionIntent` through the selection paths.** A two-valued enum
(`Continue`, `SwitchOnly`) goes into `handle_select_playback_output`, `emby_session::connect::select`,
and the pending-select records that both outputs keep while their connect runs. With
`SwitchOnly`: the capture step returns no hand-off; the two connect-time guards do not fire,
because the swap is meant to stop the old output; the Local arm calls the remote-to-Local path
with no hand-off. Every swap already stops the old output whether or not a hand-off was captured
(`stop_old_output_for_handoff`), so the switch adds a branch and no new stop logic. After the
swap the Owner clears the observed active slot and publishes it (invariant 25), so no Client
highlights the slot that stopped.

**D6. The Client capability gate keeps today's prompt.** The prompt carries a flag, set when it is
raised, that says the local Owner advertised the output switch. With the flag, the prompt has
Continue and Just switch, and Esc switches. Without it, the prompt is unchanged: Yes and No, and
No cancels. A new Client against an Owner started before an upgrade therefore degrades to
today's behavior and sends no command that Owner cannot decode. The flag is read when the prompt
is raised and stored in `ConfirmAction::ContinueOutputHandOff`, so the answer matches the
buttons the user saw. The capability read goes through the local Owner link, as the existing
`local_owner_supports_emby_session_output` does: a peer this Client controls does not take part.

**D7. One Client send for all three sites.** `select_playback_output` in
`state/remote_slot.rs` gets a sibling, `switch_playback_output`, which sends the new command. The
three sites call the sibling from the switch answer, after the same
`sever_connections_without_output_change` they call today. A sever sends no playback command
(invariant 31). The switch is not a side effect of a sever: it is the user's own answer to the
prompt, as the hand-off fork's answer is. Invariant 31's text names `SelectPlaybackOutput` as the
only output-selection send, so the change updates it.

**D8. One new domain term.** `CONTEXT.md` gets *Output switch*: the selection of a playback
output that hands nothing off. Its *Avoid* line names *Queue handoff*, *Take*, and a bare
"detach", which already name other relations.

## Risks / Trade-offs

- [An Esc press stops what plays] → The same binding exists in the direct-endpoint fork, the
  queue and the slot's recorded progress stay, and the user restarts the slot from the queue.
- [A new Client meets an Owner without the capability] → D6: the prompt keeps its cancel. The
  Owner restarts with the app, so the state ends when the Owner restarts.
- [A switch races a later selection] → The pending records already follow "last selection wins".
  The intent sits in the pending record, so a displaced switch is discarded with its record.
- [Cast or Session connect fails] → The old output stops only after the connect succeeds, so a
  failed switch leaves it playing. The Owner rejects with a reason, as a failed selection does.
- [The two guards no longer fire for a switch] → The guards protect a selection that captured
  nothing while the output was idle. A switch is the explicit request to stop the old output, so
  the guard's reason does not apply. A guard test pins that a `Continue` selection still trips
  them.

## Migration Plan

No data migration. The command and the capability are additive within protocol version 10. The v10
fixtures and schemas gain the new command shapes and are regenerated, not hand-edited. Rollback
is to remove the variant: no stored state names it.

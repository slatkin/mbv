## 1. Remote Observation Data

- [x] 1.1 Extend attached-session parsing to preserve raw position and runtime ticks without changing existing second-based presentation behavior.
- [x] 1.2 Add focused coverage for below, exactly at, and above the inclusive 95 percent near-end threshold.
- [x] 1.3 Assign monotonic generations to attached-session polls and ignore reconciliation results older than or equal to the last accepted generation.
- [x] 1.4 Distinguish failed polls, successful present-but-stopped sessions, and confirmed disappearance under the existing consecutive-miss policy.

## 2. Occurrence-Aware Reconciliation Model

- [x] 2.1 Define immutable Submitted-sequence occurrences with runtime occurrence identity, media identity, optional playlist-entry identity, and source association.
- [x] 2.2 Define process-local Tracking states, Position candidates, Tracking epochs, typed remote intents, timestamped observations, and reconciliation effects.
- [x] 2.3 Implement Tracking-session creation in `STARTING`, compatible start confirmation, stale-observation tolerance, command failure, and start expiry.
- [x] 2.4 Implement same-occurrence progression, adjacent forward advancement, below-threshold incomplete exit, and qualifying near-end Occurrence completion.
- [x] 2.5 Implement Unprompted non-adjacent invalidation and mbv-commanded non-adjacent automatic epoch anchoring.
- [x] 2.6 Implement candidate evolution through same-occurrence continuity, immediate-successor advancement, live intent targets, and explicit re-anchor only.
- [x] 2.7 Implement duplicate Position candidates, non-adjacent duplicate exclusion, ambiguity retention, later candidate collapse, and consecutive-duplicate reset ambiguity without retroactive completion.
- [x] 2.8 Implement final-occurrence completion on qualifying unprompted stop and suppression for explicit Stop or session disappearance.
- [x] 2.9 Invalidate unexplained backward transitions and material same-occurrence resets while preserving explicit re-anchor recovery.
- [x] 2.10 Implement `SUSPENDED` entry, normal intent expiry during suspension, exact and adjacent automatic return, ambiguous return, reanchorable invalid return, and incompatible invalid return.
- [x] 2.11 Implement explicit re-anchor, duplicate occurrence selection, Bypassed occurrence marking, and unavailable re-anchor for out-of-sequence items.
- [x] 2.12 Keep retained observations and intent evidence bounded to data that can still affect current reconciliation or pending effects.
- [x] 2.13 Add table-driven trace coverage for startup, adjacency, poll gaps, stale generations, duplicate candidates, invalidation, suspension returns, re-anchor, stopped state, and completion scenarios.

## 3. Attached-Session Command Correlation

- [x] 3.1 Add one App-level attached-session sequence-submission path that snapshots occurrences, starts tracking, sends `session_play_items`, and correlates the result.
- [x] 3.2 Route every existing attached-session multi-item submission call site through the centralized submission path.
- [x] 3.3 Record typed Expected transitions before dispatching Next, Previous, direct occurrence selection, restart, Seek, and Stop commands.
- [x] 3.4 Implement target anchoring and source-completion suppression for Next, Previous, and direct selection; same-occurrence preservation for Seek/restart; and completion suppression for Stop.
- [x] 3.5 Implement contradiction fallback to Unprompted-transition rules plus intent supersession and expiry, including expiry while suspended, without delaying command dispatch.
- [x] 3.6 Correlate asynchronous submission and command outcomes to the current Tracking-session and epoch identities so stale outcomes are inert.
- [x] 3.7 Feed accepted connected-session observations, stopped state, and confirmed temporary disappearance into the reconciliation model.
- [x] 3.8 Update queue cursor projection from resolved occurrence identity instead of first matching media ID while tracking is active.
- [x] 3.9 Terminate active tracking before attached-session single-item playback and preserve current untracked behavior after dispatch and after Stop Tracking.

## 4. Tracking Lifecycle and Queue Editing

- [x] 4.1 Add process-local tracker ownership to App construction, target switching, disconnect, and shutdown paths without adding persisted queue-state fields.
- [x] 4.2 Terminate tracking on disconnect, connection to another session, process exit, and explicit Stop Tracking while leaving remote attachment intact for Stop Tracking.
- [x] 4.3 Keep tracking associated with an idle or stopped remote session and reconcile compatible later playback within the same process run.
- [x] 4.4 Add a first-edit confirmation covering enqueue, remove, reorder, and undo whenever a tracker exists in any health state.
- [x] 4.5 On edit confirmation, terminate tracking, apply the requested edit, and suppress further tracking warnings until another sequence submission.
- [x] 4.6 On edit cancellation, preserve both the Submitted sequence and the queue unchanged.

## 5. Occurrence Consume

- [x] 5.1 Map a completed occurrence to its queue slot through the remote queue projection, guarded by session, epoch, and queue lineage.
- [x] 5.2 Remove the mapped slot from the queue and route through `on_video_consumed`/`on_audio_consumed`, the same path local playback uses.
- [x] 5.3 Leave playlist persistence entirely to the separate Save on consume setting.
- [x] 5.4 Re-check tracker, epoch, and queue lineage when the completion is applied; abort stale applications.
- [x] 5.5 Gate consume by the media-specific consume setting, valid resolved tracking, and at-most-once occurrence emission — never by playlist association.
- [x] 5.6 Discard consume eligibility after tracker replacement, epoch change, disconnect, Stop Tracking, or process shutdown.
- [x] 5.7 Add application coverage for consume on an ad-hoc queue, at-most-once emission, and stale queue lineage.

## 6. Queue and Sessions UI

- [x] 6.1 Add compact remote target, Tracking state, and resolved or candidate position presentation to the queue panel's existing title/source area.
- [x] 6.2 Keep normal `TRACKING` presentation quiet and show concise reasons for `AMBIGUOUS`, `INVALID`, and `SUSPENDED`.
- [x] 6.3 Add queue-context actions for re-anchor and Stop Tracking, including an occurrence picker for duplicate re-anchor targets.
- [x] 6.5 Mark active tracking in the Sessions panel without adding a second tracking-management surface.
- [x] 6.6 Preserve usable narrow and wide queue layouts and add keyboard and mouse hit targets through the existing input-resolution paths.
- [x] 6.7 Add render and input coverage for normal, starting, ambiguous, invalid, suspended, re-anchor, Stop Tracking, and edit-confirmation states.

## 7. End-to-End Verification

- [x] 7.1 Verify all existing local, local-daemon, direct-remote, playlist-save, queue-edit, and attached-session tests remain green.
- [ ] 7.2 Exercise representative attached-client traces for natural advance, each mbv command intent, client-side Next, non-adjacent jump, duplicate ambiguity, backward transition, same-item reset, final stop, present-but-stopped, failed poll, disappearance, each return class, and re-anchor.
- [x] 7.3 Verify consume removes the completed occurrence from an ad-hoc queue as well as a saved-playlist queue.
- [ ] 7.4 Verify tracking terminates before single-item replacement and that tracking state disappears on every disconnect, target replacement, and quit path and is absent after restart.
- [ ] 7.5 Verify stale poll generations and stale command outcomes cannot mutate current tracking or queue state.
- [x] 7.6 Run formatting, linting, focused tests, the full project test suite, and strict OpenSpec validation.

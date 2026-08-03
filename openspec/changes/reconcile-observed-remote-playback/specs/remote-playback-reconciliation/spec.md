## Purpose

Reconciles observable Emby session playback with multi-item sequences submitted by mbv while making uncertainty visible and preventing unsafe playlist consumption.

## ADDED Requirements

### Requirement: Multi-item remote submissions start tracking
When mbv submits a multi-item sequence to an attached Emby session, mbv SHALL create an in-memory Tracking session for that Submitted sequence in the `STARTING` state. Single-item playback SHALL NOT create a Tracking session and SHALL terminate any Tracking session already associated with that remote target before dispatch.

#### Scenario: Multi-item sequence is submitted
- **WHEN** mbv submits two or more ordered occurrences to an attached Emby session
- **THEN** mbv starts tracking the Submitted sequence at the requested occurrence
- **AND** tracking initially reports `STARTING`

#### Scenario: Single item is submitted
- **WHEN** mbv submits one item to an attached Emby session
- **THEN** mbv does not start sequence tracking

#### Scenario: Single item replaces tracked playback
- **WHEN** mbv submits one item while a Tracking session is active for that remote target
- **THEN** mbv terminates the Tracking session before dispatching the single-item request

### Requirement: Starting requires compatible remote confirmation
mbv SHALL enter `TRACKING` only after a Remote observation is compatible with the requested starting occurrence. mbv SHALL tolerate apparently stale observations while the initial Expected transition remains live. If the command fails, mbv SHALL terminate tracking. If the Expected transition expires without compatible confirmation, mbv SHALL enter `INVALID`.

#### Scenario: Requested occurrence is confirmed
- **WHEN** a starting Tracking session observes the requested occurrence
- **THEN** mbv enters `TRACKING` at that occurrence

#### Scenario: Stale prior item is reported before confirmation
- **WHEN** mbv observes the pre-submission item while the initial Expected transition remains live
- **THEN** mbv remains `STARTING`
- **AND** mbv does not treat that observation alone as contradictory playback

#### Scenario: Different playback advances during startup
- **WHEN** an incompatible item shows meaningful post-submission progress while the initial Expected transition remains live
- **THEN** mbv enters `INVALID`
- **AND** reports that the remote played a different item

#### Scenario: Submission is not confirmed
- **WHEN** the initial Expected transition expires without a compatible observation
- **THEN** mbv enters `INVALID`
- **AND** reports that the remote did not confirm the Submitted sequence

### Requirement: Tracking exposes bounded health states
mbv SHALL expose `STARTING`, `TRACKING`, `AMBIGUOUS`, `INVALID`, and `SUSPENDED` as the Tracking session health states. `TRACKING` SHALL mean that at least one permitted reconciliation path remains and the current or most recent occurrence is resolved. `AMBIGUOUS` SHALL mean that multiple Position candidates remain plausible. `INVALID` SHALL mean that no permitted reconciliation path explains the observations. `SUSPENDED` SHALL mean that the attached Emby session is temporarily unavailable. Remote playback status, including idle or stopped, SHALL remain separate from Tracking health.

#### Scenario: Normal tracking is quiet
- **WHEN** tracking has one resolved occurrence and no exceptional condition
- **THEN** the queue panel reports `TRACKING` without an intrusive prompt

#### Scenario: Exceptional state is explained
- **WHEN** tracking enters `AMBIGUOUS`, `INVALID`, or `SUSPENDED`
- **THEN** the queue panel reports the state and a concise reason

### Requirement: Tracking uses bounded reconciliation evidence
mbv SHALL retain the Submitted sequence, current Position candidates, observations and Expected transitions still capable of affecting reconciliation, completed or consumed occurrences, bypassed occurrences, and the current health reason. mbv SHALL NOT require a user-facing observation journal.

#### Scenario: Old evidence can no longer affect reconciliation
- **WHEN** retained evidence can no longer change current Position candidates, completion decisions, or recovery
- **THEN** mbv may discard that evidence
- **AND** current tracking behavior remains unchanged

### Requirement: mbv-issued commands create expected transitions
An mbv-issued command that should change the current occurrence SHALL create an Expected transition. Later observations SHALL classify that transition as confirmed, contradicted, expired, or superseded. An Expected transition SHALL be treated as evidence and not as proof that the command was applied.

#### Scenario: Next command is observed
- **WHEN** mbv issues Next from a resolved occurrence and observes its immediate successor
- **THEN** mbv confirms the Expected transition
- **AND** does not classify the prior occurrence as naturally completed solely from that transition

#### Scenario: Previous or direct selection reaches its target
- **WHEN** mbv issues Previous or selects a Submitted-sequence occurrence and then observes that exact target
- **THEN** mbv confirms the Expected transition
- **AND** anchors tracking at the target without inferring completion for the source occurrence

#### Scenario: Seek explains position regression
- **WHEN** a same-occurrence position regression is compatible with a live mbv Seek, restart, or seek-to-start intent
- **THEN** mbv keeps the resolved occurrence
- **AND** does not infer completion or invalidate tracking from that regression

#### Scenario: Stop explains stopped playback
- **WHEN** mbv issues Stop and the remote subsequently reports stopped or no current item
- **THEN** mbv confirms the Expected transition
- **AND** does not infer completion solely from that stop

#### Scenario: Command target is contradicted
- **WHEN** the remote changes to an occurrence incompatible with every target of a live Expected transition
- **THEN** mbv marks the Expected transition contradicted
- **AND** reconciles the observation under the Unprompted-transition rules

#### Scenario: Command expires without item change
- **WHEN** an Expected transition expires while the same occurrence remains observed
- **THEN** mbv marks the Expected transition expired
- **AND** keeps the current Position candidate without inferring completion

### Requirement: Adjacent forward observations preserve tracking
An Unprompted transition from a resolved occurrence to its immediate successor SHALL preserve valid tracking. If the prior occurrence was below the completion threshold, mbv SHALL classify it as exited incomplete and SHALL NOT consume it.

#### Scenario: Client skips an item before completion
- **WHEN** a resolved occurrence below 95 percent is followed by its immediate successor without an applicable Expected transition
- **THEN** mbv remains `TRACKING` at the successor
- **AND** does not consume the prior occurrence
- **AND** does not report an unresolved playlist outcome solely for the skipped occurrence

### Requirement: Unexplained transitions invalidate tracking
An Unprompted transition that skips one or more Submitted-sequence occurrences, moves backward, or materially resets the same non-duplicate occurrence SHALL enter `INVALID`. mbv SHALL infer no completion or traversal across the invalidating transition.

#### Scenario: Remote jumps over an occurrence
- **WHEN** mbv tracks `B` in `A B C D` and next observes `D` without an applicable Expected transition
- **THEN** tracking becomes `INVALID`
- **AND** mbv does not consume `B` or infer what happened to `C`

#### Scenario: Remote moves backward without an expected transition
- **WHEN** mbv tracks `C` in `A B C D` and next observes `B` without an applicable Expected transition
- **THEN** tracking becomes `INVALID`
- **AND** mbv infers no completion from the backward transition

#### Scenario: Same occurrence materially resets
- **WHEN** a resolved non-duplicate occurrence materially resets toward its beginning without an applicable restart, seek, or direct-play intent
- **THEN** tracking becomes `INVALID`
- **AND** re-anchoring remains available at that occurrence

#### Scenario: mbv intentionally selects a non-adjacent occurrence
- **WHEN** mbv commands playback of a non-adjacent occurrence and observes the commanded target
- **THEN** mbv starts a new Tracking epoch at that occurrence
- **AND** does not invalidate tracking

### Requirement: Duplicate occurrences preserve occurrence identity
When a Remote observation matches multiple plausible occurrences in the Submitted sequence, mbv SHALL retain each plausible occurrence as a Position candidate and enter `AMBIGUOUS`. While occurrence identity remains ambiguous, mbv SHALL NOT consume any candidate occurrence.

Position candidates SHALL be derived only from the prior candidates through same-occurrence continuity, immediate-successor advancement, the exact target of a live Expected transition, or explicit re-anchor selection. A matching media ID elsewhere in the Submitted sequence SHALL NOT become a candidate merely because its ID matches. After each accepted observation, zero candidates SHALL produce `INVALID`, one candidate SHALL produce `TRACKING`, and multiple candidates SHALL produce `AMBIGUOUS`.

#### Scenario: Duplicate item has multiple plausible positions
- **WHEN** an observed media item matches two plausible Submitted-sequence occurrences
- **THEN** mbv enters `AMBIGUOUS`
- **AND** retains both occurrences as Position candidates
- **AND** pauses consume

#### Scenario: Non-adjacent duplicate is not automatically plausible
- **WHEN** an observed media ID also occurs elsewhere in the Submitted sequence but that occurrence is not reachable from a prior candidate or live Expected transition
- **THEN** mbv does not add the non-adjacent occurrence as a Position candidate

#### Scenario: Later observation resolves duplicate ambiguity
- **WHEN** later adjacent observations eliminate every Position candidate except one
- **THEN** mbv resumes `TRACKING` at the resolved occurrence

#### Scenario: Consecutive duplicates cannot be distinguished
- **WHEN** the same media ID materially resets and both the current occurrence and its immediate duplicate successor remain plausible
- **THEN** mbv enters `AMBIGUOUS`
- **AND** consumes neither occurrence

#### Scenario: No candidate explains an accepted observation
- **WHEN** candidate elimination leaves no permitted occurrence
- **THEN** mbv enters `INVALID`

### Requirement: Completion requires occurrence-level evidence
Media completion SHALL NOT by itself authorize consuming an occurrence when more than one occurrence could explain playback. Only Occurrence completion for a resolved Submitted-sequence occurrence SHALL authorize consume.

#### Scenario: Played media has duplicate occurrences
- **WHEN** Emby reports a media item played but tracking cannot resolve which duplicate occurrence played
- **THEN** mbv records no consumable Occurrence completion
- **AND** consumes neither duplicate

### Requirement: Near-end adjacent advancement infers completion
For audio and video, a resolved occurrence last observed at or beyond 95 percent of a known runtime SHALL receive Inferred completion when the next compatible observation is its immediate successor and no skip-like Expected transition explains the transition. Near end SHALL require runtime ticks greater than zero and an overflow-safe comparison equivalent to `position_ticks * 20 >= runtime_ticks * 19`; exactly 95 percent SHALL qualify.

#### Scenario: Natural-looking adjacent advancement
- **WHEN** a resolved occurrence is observed at or beyond 95 percent and is next followed by its immediate successor without a skip-like Expected transition
- **THEN** mbv infers Occurrence completion for the prior occurrence

#### Scenario: Runtime is unknown
- **WHEN** runtime is unknown
- **THEN** percentage-based Inferred completion is unavailable

#### Scenario: Next command explains advancement
- **WHEN** an mbv-issued Next command explains the adjacent transition
- **THEN** mbv does not infer completion solely from the prior near-end position

### Requirement: Final occurrence can complete on an unprompted stop
The resolved final occurrence SHALL receive Inferred completion when it was last observed at or beyond 95 percent and the remote subsequently reports stopped or no current item without an applicable mbv Stop, skip, seek, or restart intent. Session disappearance alone SHALL NOT infer completion.

#### Scenario: Final occurrence stops near end
- **WHEN** the final resolved occurrence is observed at or beyond 95 percent and the remote then reports stopped without an applicable stop-like intent
- **THEN** mbv infers Occurrence completion

#### Scenario: mbv explicitly stops near end
- **WHEN** mbv issues Stop and the remote stops while the final occurrence is at or beyond 95 percent
- **THEN** mbv does not infer completion solely from the stop

#### Scenario: Session disappears near end
- **WHEN** the Emby session disappears while the final occurrence is at or beyond 95 percent
- **THEN** mbv suspends tracking
- **AND** does not infer completion from disappearance

### Requirement: Session disappearance suspends tracking
When the attached Emby session temporarily disappears, mbv SHALL enter `SUSPENDED`, preserve current reconciliation evidence, expire Expected transitions when appropriate, and make no new traversal or completion inference while suspended.

#### Scenario: Attached session disappears
- **WHEN** a tracked Emby session is no longer observable while the mbv attachment remains logically active
- **THEN** tracking enters `SUSPENDED`
- **AND** pending uncertain completion is not promoted

#### Scenario: Poll request fails
- **WHEN** an attached-session poll fails without a successful response establishing that the session is absent
- **THEN** mbv retains the current Tracking state
- **AND** does not treat the transport failure as stopped playback or session disappearance

#### Scenario: Session remains present with no current item
- **WHEN** a successful poll includes the attached session but reports no current item
- **THEN** mbv treats the session as present with stopped playback
- **AND** does not enter `SUSPENDED`

#### Scenario: Expected transition expires during suspension
- **WHEN** an Expected transition reaches its normal deadline while tracking is `SUSPENDED`
- **THEN** mbv marks it expired
- **AND** the expired transition cannot justify a later returning state

### Requirement: Remote observations are applied in poll order
mbv SHALL order Remote observations by a monotonic local poll generation. An observation older than or equal to the last accepted generation SHALL NOT change candidates, completion, or Tracking health. Repeated accepted observations with no meaningful item or position change SHALL NOT create an Observed transition. A gap between accepted observations SHALL NOT imply unseen traversal.

#### Scenario: Stale poll result arrives late
- **WHEN** a poll result arrives with a generation older than or equal to the last accepted observation
- **THEN** mbv ignores it for reconciliation

#### Scenario: Repeated poll reports the same state
- **WHEN** a newer poll repeats the same media item and materially unchanged position
- **THEN** mbv retains the current candidates
- **AND** creates no item transition or completion

#### Scenario: Poll gap hides intermediate playback
- **WHEN** the next accepted observation is non-adjacent to every permitted candidate after an observation gap
- **THEN** mbv enters `INVALID`
- **AND** does not infer intermediate traversal

### Requirement: Returning state leaves suspension deterministically
A suspended Tracking session SHALL resume automatically only when the returning observation is `EXACT` or `ADJACENT` to an expected state. Once the session is observable again, mbv SHALL leave `SUSPENDED`: a uniquely identifiable non-adjacent occurrence SHALL enter `INVALID` with re-anchor available; multiple plausible occurrences SHALL enter `AMBIGUOUS` with occurrence selection available; and an out-of-sequence or otherwise incompatible item SHALL enter `INVALID` without re-anchor.

#### Scenario: Exact state returns
- **WHEN** the same candidate occurrence returns at a plausibly unchanged or advanced position
- **THEN** mbv resumes tracking automatically

#### Scenario: Adjacent successor returns
- **WHEN** the immediate expected successor returns
- **THEN** mbv resumes tracking automatically at the successor
- **AND** infers completion for the prior occurrence only when the normal completion rules are satisfied

#### Scenario: Position regresses materially
- **WHEN** the same occurrence returns at a materially earlier position without a matching intent
- **THEN** mbv enters `INVALID`
- **AND** offers re-anchoring when occurrence identity is otherwise resolvable

#### Scenario: Unique non-adjacent occurrence returns
- **WHEN** a returning observation uniquely identifies a non-adjacent occurrence in the Submitted sequence
- **THEN** mbv enters `INVALID`
- **AND** requires explicit re-anchoring

#### Scenario: Ambiguous occurrence returns
- **WHEN** a returning observation has multiple permitted Position candidates
- **THEN** mbv enters `AMBIGUOUS`
- **AND** requires occurrence selection before re-anchoring

#### Scenario: Incompatible item returns
- **WHEN** the returning observation cannot map to any Submitted-sequence occurrence
- **THEN** mbv enters `INVALID`
- **AND** does not offer re-anchor

### Requirement: Re-anchoring starts a new tracking epoch
When the user re-anchors at a uniquely selected occurrence, mbv SHALL start a new Tracking epoch there. Unresolved earlier occurrences SHALL become Bypassed occurrences, remain in the saved playlist, and become ineligible for automatic consume in the new epoch. Previously confirmed and applied consumes SHALL remain applied.

#### Scenario: User re-anchors after invalidation
- **WHEN** tracking is invalid and the user re-anchors at a unique observed occurrence
- **THEN** mbv resumes `TRACKING` in a new epoch at that occurrence
- **AND** leaves unresolved earlier occurrences untouched

#### Scenario: Re-anchor target is duplicated
- **WHEN** the observed media item has multiple selectable occurrences
- **THEN** mbv requires the user to select an occurrence before re-anchoring

#### Scenario: Observed item is outside the sequence
- **WHEN** the observed item does not occur in the Submitted sequence
- **THEN** re-anchoring is unavailable

### Requirement: Tracking lifecycle is process-local
Tracking sessions SHALL NOT persist across mbv process exit or restart. Tracking SHALL terminate on disconnect, connection to another Emby session, process exit, explicit Stop Tracking, or manual queue editing confirmed by the user. Remote playback stopping by itself SHALL leave tracking idle and available for compatible continuation.

#### Scenario: mbv exits
- **WHEN** mbv exits for any reason
- **THEN** all Tracking sessions and unresolved playlist outcomes are discarded

#### Scenario: Remote playback stops
- **WHEN** the remote reports stopped but mbv remains attached
- **THEN** Tracking health remains `TRACKING` at the most recent resolved occurrence
- **AND** remote playback status becomes idle

#### Scenario: User stops tracking
- **WHEN** the user chooses Stop Tracking
- **THEN** mbv ends reconciliation and consume inference without disconnecting from the Emby session

### Requirement: Manual queue edits terminate tracking after confirmation
The first enqueue, remove, reorder, or undo action while a Tracking session exists in `STARTING`, `TRACKING`, `AMBIGUOUS`, `INVALID`, or `SUSPENDED` SHALL require confirmation that the edit will stop tracking. On confirmation, mbv SHALL terminate tracking and permit queue editing. Further edits SHALL NOT repeat the warning until mbv submits another sequence.

#### Scenario: User cancels first edit
- **WHEN** the user attempts the first manual queue edit during active tracking and cancels the warning
- **THEN** mbv leaves both the queue and Tracking session unchanged

#### Scenario: User confirms first edit
- **WHEN** the user confirms the first manual queue edit
- **THEN** mbv terminates tracking
- **AND** applies the edit
- **AND** remains attached to the Emby session

### Requirement: Reconciliation applies to every submitted multi-item source
mbv SHALL reconcile every multi-item Submitted sequence regardless of whether its source is a saved playlist, album, series, collection, or ad hoc queue. Automatic playlist consumption SHALL apply only when the Submitted sequence remains associated with a saved Emby playlist.

#### Scenario: Album is submitted remotely
- **WHEN** mbv submits a multi-item album to an attached Emby session
- **THEN** mbv tracks its remote position
- **AND** does not perform saved-playlist mutation

#### Scenario: Saved playlist is submitted remotely
- **WHEN** mbv submits a saved Emby playlist
- **THEN** mbv tracks its remote position
- **AND** may apply safe consume behavior when configured

### Requirement: Safe occurrence completion is consumed promptly
When media-type consume is enabled, mbv SHALL promptly apply a resolved Occurrence completion to the associated saved playlist only while tracking is active and valid, the occurrence has not already been consumed, and the intended server playlist occurrence can be identified safely. These conditions SHALL be checked when completion emits the consume request and checked again after the playlist reload immediately before deletion.

#### Scenario: Completed occurrence is safely identifiable
- **WHEN** a valid Tracking session establishes Occurrence completion and the exact saved-playlist occurrence remains safely identifiable
- **THEN** mbv removes that occurrence automatically when consume is enabled for its media type

#### Scenario: Tracking is ambiguous or invalid
- **WHEN** tracking is `AMBIGUOUS`, `INVALID`, or `SUSPENDED`
- **THEN** mbv does not initiate a new automatic consume from uncertain evidence

### Requirement: Playlist mutation preserves unrelated external edits
Before applying remote consume, mbv SHALL reload current server playlist state and verify that the stable playlist-entry identity still exists and still identifies the expected media item. A missing entry SHALL count as already applied; an entry mapped to different media SHALL be unresolved and SHALL NOT be deleted. mbv SHALL attempt at most one exact-entry deletion per completed occurrence, preserve unrelated server-side additions, removals, and reordering, and SHALL NOT fall back to replacing the playlist. When the intended occurrence cannot be verified or the deletion result cannot be established, mbv SHALL report an unresolved outcome passively.

#### Scenario: Playlist changed but occurrence identity remains stable
- **WHEN** the server playlist changed externally and the completed occurrence remains safely identifiable
- **THEN** mbv removes only that occurrence
- **AND** preserves unrelated external changes

#### Scenario: External changes make occurrence identity unsafe
- **WHEN** current playlist state cannot safely identify the completed occurrence
- **THEN** mbv does not mutate the playlist automatically
- **AND** reports an unresolved playlist outcome

#### Scenario: Stable entry identity maps to different media
- **WHEN** the current playlist contains the target entry identity but it no longer identifies the expected media item
- **THEN** mbv does not delete the entry
- **AND** reports an unresolved playlist outcome

#### Scenario: Exact deletion outcome is uncertain
- **WHEN** mbv cannot establish whether the one exact-entry deletion attempt succeeded
- **THEN** mbv does not retry the deletion automatically
- **AND** reports an unresolved playlist outcome

#### Scenario: External playlist edit occurs during playback
- **WHEN** the associated saved playlist changes externally
- **THEN** playback tracking remains valid against the immutable Submitted sequence

### Requirement: Unresolved playlist outcomes remain passive
An unresolved playlist outcome SHALL remain process-local, SHALL NOT trigger automatic retries or interactive repair, and SHALL NOT block playback, queue replacement, disconnect, or process exit. mbv SHALL NOT create unresolved outcomes for ordinary incomplete skips, Bypassed occurrences, unobserved items, suspension, or invalidation without completion evidence.

#### Scenario: Unresolved outcomes accumulate during playback
- **WHEN** one or more unresolved playlist outcomes exist
- **THEN** the queue panel shows a passive unresolved count
- **AND** does not open a prompt automatically

#### Scenario: User quits with unresolved outcomes
- **WHEN** mbv exits with unresolved playlist outcomes
- **THEN** exit is not blocked
- **AND** the outcomes are not persisted across restart

### Requirement: Queue panel is the primary tracking surface
The queue panel SHALL present the active Tracking session's health, remote target, current resolved or candidate position, concise exceptional-state reason, unresolved playlist-outcome count, and available re-anchor action. The Sessions panel SHALL only indicate that the attached session has active tracking.

#### Scenario: Tracking is valid
- **WHEN** tracking is valid and unambiguous
- **THEN** the queue panel identifies the remote target and current sequence position without an intrusive explanation

#### Scenario: Tracking is invalid and re-anchorable
- **WHEN** tracking is invalid and the observed item can be mapped to one or more occurrences
- **THEN** the queue panel explains the mismatch
- **AND** offers re-anchoring, including occurrence selection when needed

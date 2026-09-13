## REMOVED Requirements

### Requirement: Multi-item remote submissions start tracking
**Reason**: mbv cannot observe a generic Emby client's queue or occurrence identity reliably enough to reconstruct its submitted sequence.

**Migration**: Multi-item submission remains available, but mbv observes only the Session's current playback state afterward.

### Requirement: Starting requires compatible remote confirmation
**Reason**: Sequence startup confirmation exists only to support the removed Tracking model.

**Migration**: Command success or failure remains the only acknowledgement of an attached-session submission.

### Requirement: Tracking exposes bounded health states
**Reason**: STARTING, TRACKING, AMBIGUOUS, INVALID, and SUSPENDED describe removed inferred state.

**Migration**: The UI continues to show directly observed Session availability and playback status.

### Requirement: Tracking uses bounded reconciliation evidence
**Reason**: Reconciliation evidence is no longer retained for generic Emby Sessions.

**Migration**: No user action is required because the evidence was process-local.

### Requirement: mbv-issued commands create expected transitions
**Reason**: Correlating transport commands with later observations adds complexity without establishing remote queue authority.

**Migration**: Supported transport commands continue to be dispatched without creating inferred sequence transitions.

### Requirement: Adjacent forward observations preserve tracking
**Reason**: Adjacency cannot be established from the generic Emby Session API.

**Migration**: A changed current item is displayed as an observed Session change only.

### Requirement: Unexplained transitions invalidate tracking
**Reason**: Tracking validity is removed with sequence reconciliation.

**Migration**: Non-adjacent, backward, and reset observations require no recovery action and do not mutate mbv's queue.

### Requirement: Duplicate occurrences preserve occurrence identity
**Reason**: Generic Emby observations expose content identity but not occurrence identity.

**Migration**: Duplicate items remain distinct only in queues owned by mbv Player owners; Session watch does not map observations to either occurrence.

### Requirement: Completion requires occurrence-level evidence
**Reason**: mbv no longer infers completion for submitted generic-session occurrences.

**Migration**: Completion reported and applied by an authoritative Player owner remains unchanged.

### Requirement: Near-end adjacent advancement infers completion
**Reason**: Near-end position plus an observed item change is insufficient proof of occurrence completion.

**Migration**: No completion or consume effect is produced from generic-session advancement.

### Requirement: Final occurrence can complete on an unprompted stop
**Reason**: A generic Session stopping is insufficient proof that a particular submitted occurrence completed.

**Migration**: Stopped status remains visible but does not mutate mbv's queue.

### Requirement: Session disappearance suspends tracking
**Reason**: There is no Tracking state to suspend.

**Migration**: Existing Session disappearance and attachment handling remains responsible for the observed connection state.

### Requirement: Remote observations are applied in poll order
**Reason**: Poll generations used specifically for reconciliation no longer affect inferred queue state.

**Migration**: Session polling continues to update directly observed current playback state.

### Requirement: Returning state leaves suspension deterministically
**Reason**: Recovery from suspended inferred sequence state is removed.

**Migration**: A returning Session resumes ordinary read-only observation without reconciliation or re-anchoring.

### Requirement: Re-anchoring starts a new tracking epoch
**Reason**: Re-anchoring asks users to repair a sequence projection mbv cannot make authoritative.

**Migration**: Users may issue ordinary playback commands; there is no Tracking epoch to repair.

### Requirement: Tracking lifecycle is process-local
**Reason**: The process-local Tracking lifecycle is removed.

**Migration**: No persisted-data migration is required.

### Requirement: Manual queue edits terminate tracking after confirmation
**Reason**: Queue edits no longer conflict with inferred Tracking state.

**Migration**: Queue edits proceed under their ordinary queue and unsaved-playlist confirmation rules.

### Requirement: Reconciliation applies to every submitted multi-item source
**Reason**: Reconciliation is removed for every source rather than retained selectively.

**Migration**: Albums, series, collections, playlists, and ad hoc queues may still be submitted to supported Sessions without subsequent sequence Tracking.

### Requirement: Safe occurrence completion is consumed promptly
**Reason**: Generic-session observations cannot safely prove occurrence completion or authorize canonical queue mutation.

**Migration**: Consume remains available only through authoritative local, Local daemon, or directly controlled Player-owner playback lifecycle events.

### Requirement: External playlist edits do not affect tracking
**Reason**: There is no immutable Submitted sequence or Tracking state to protect from external playlist edits.

**Migration**: Existing explicit playlist load and save behavior remains unchanged.

### Requirement: Queue panel is the primary tracking surface
**Reason**: Tracking health, recovery, and management UI are removed.

**Migration**: The queue panel retains ordinary queue source, playback target, and directly observed Session presentation without Tracking labels or actions.

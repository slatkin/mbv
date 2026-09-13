## Context

See `proposal.md` - Why and `specs/remote-playback-reconciliation/spec.md` for the removed behavioral contract.

An attached generic Emby Session is not a Player owner under mbv's queue model. Emby's Session API reports current playback state and accepts transport or play commands, but does not expose a canonical queue, queue slot identities, or occurrence positions. The current implementation nevertheless snapshots submitted items into a `ReconciliationTracker`, correlates later commands and observations, projects inferred occurrences back to queue slots, and can consume those slots. It also independently refreshes the previously observed Emby item and replaces the first local queue occurrence with matching content identity.

The useful Session path is separable: discovery and attachment, polling directly observed state, position presentation, disappearance handling, and supported commands all use `connected_session_id` / `connected_session_state` and do not require sequence Tracking. Direct remote control uses mbv's ctrl protocol and an authoritative Player-owner queue; it is a different path and must remain intact.

## Goals / Non-Goals

**Goals:**

- Restore the authority boundary: observation of a generic Emby Session cannot mutate mbv queue membership, ordering, slot snapshots, cursor, dirty state, or playlist state.
- Delete the reconciliation model and every App, command, lifecycle, input, and rendering seam that exists only for it.
- Keep attached-session play and transport commands simple and independent of later poll observations.
- Preserve the existing TuiRealm ownership and central routing boundaries while deleting Tracking UI.

**Non-Goals:**

- Diagnose or improve latency in local, Local daemon, or directly controlled Player-owner queues.
- Change local or owner-authoritative progress reporting, watched state, consume, queue persistence, or unified queue snapshots.
- Remove Session discovery, Session watch, attached-session transport controls, or Direct remote control.
- Add a replacement heuristic for generic-client queue position or completion.

## Decisions

### 1. Treat generic Emby Sessions as observation and command targets, never queue authorities

A successful Session poll updates only directly observed Session presentation state. It does not locate a matching local content ID, move the queue cursor, refresh a queue item, infer an occurrence, or remove a slot.

This removes both the explicit Tracking projection and the older first-content-match watched/progress refresh. Keeping the refresh was considered and rejected because duplicate content occurrences make its target unknowable, while replacing a complete queue snapshot imports more than the observed status.

### 2. Delete reconciliation rather than leave it dormant or deprecated in code

Remove the reconciliation modules, tracker fields, queue projection and lineage used by Tracking, correlated reconciliation command payloads, observation effects, re-anchor model, and Tracking-specific tests. Do not add a feature flag or compatibility adapter: state is process-local, no stored format depends on it, and dormant code would preserve the complexity this change exists to remove.

The ordinary attached-session command helper remains and reports command failures through the existing user-visible error path. Sequence submission remains a direct `session_play_items` operation without tracker creation.

### 3. Remove Tracking UI through its existing ownership boundaries

Delete the Remote re-anchor Interactive Component, its modal registration and typed messages, the queue's Stop Tracking intent and hit target, Tracking labels/reasons in queue-title content, and the Tracking marker in the Sessions sidebar. Remove router/modal policy entries only where they name the deleted popup; do not introduce fallback input handling or a second routing site.

The queue panel continues to show its ordinary local/connected target presentation. No replacement status is added because Session playback status is already presented from `connected_session_state`.

### 4. Remove Tracking-specific queue-edit coordination without changing ordinary safeguards

Delete the first-edit warning and pending actions whose only purpose is to terminate Tracking. Preserve confirmation for removing a currently playing item, unsaved playlist replacement, owner command synchronization, undo, and all other ordinary queue rules.

Playlist save and Save on consume remain reachable only from authoritative queue lifecycle paths. Generic Session observations no longer call consume callbacks and therefore cannot mark a queue dirty or enqueue a playlist save.

### 5. Keep local queue latency outside this change

Local `PlayerEvent::Stopped`, `TrackCompleted`, `TrackChanged`, and `UnifiedQueueUpdated` behavior remains unchanged, including progress application, revision changes, owner synchronization, consume, and persistence. Mixing performance changes into this deletion would obscure whether regressions came from removing unsupported inference or changing authoritative playback behavior.

A later latency investigation can measure those paths independently after this observer-driven mutation path is gone.

### 6. Remove the capability and amend the domain model on archive

The delta removes every requirement in `remote-playback-reconciliation`; archive therefore removes that main capability rather than replacing it with a hollow specification. Update `CONTEXT.md` in implementation to delete Tracking-specific language and describe Session watch as read-only with respect to mbv's queue. Existing Session watch, Direct remote control, Queue scope, and Consume terms remain, with Consume narrowed to authoritative Player-owner lifecycle rather than another device's generic Session.

## Risks / Trade-offs

- **[Risk] Users relied on remote advancement moving the local queue cursor** -> Make the removal explicit in characterization tests: Session watch reports the remote item but leaves queue presentation state unchanged.
- **[Risk] Watched badges in queued snapshots remain stale after another device finishes an item** -> Accept staleness; refresh library content through ordinary library refresh paths rather than guessing a queue occurrence.
- **[Risk] Shared command helpers accidentally lose plain attached-session controls during correlation cleanup** -> Characterize sequence submission and each retained transport command without a tracker before deleting the correlation layer.
- **[Risk] Direct remote queue behavior is removed because both paths are colloquially called remote** -> Scope deletions to attached generic Emby Session state and retain ctrl/Player-owner integration tests.
- **[Trade-off] mbv no longer offers remote consume for generic clients** -> Prefer no mutation over a completion inference unsupported by the Session API.
- **[Trade-off] This does not fix local queue latency** -> Deliberately defer profiling and changes to authoritative local/owner paths.

## Migration Plan

1. Add retained-behavior characterization at the attached-session boundary: multi-item submission and transport still dispatch, polls still update Session presentation, and observations do not mutate queue state.
2. Remove observer-driven queue refresh, cursor matching, completion consume, and Tracking-specific queue-edit coordination.
3. Remove Tracking presentation, actions, popup registration, and component tests through the existing Interactive Component and queue-panel boundaries.
4. Simplify attached-session command and lifecycle events, then delete App tracker/projection fields and the `mbv-core` reconciliation model.
5. Remove obsolete tests and update domain documentation; verify local, Local daemon, Direct remote, playlist, queue, and Session watch coverage remains green.

Rollback is source-only: revert the change. There is no persisted Tracking state or data migration.

## Context

Single-item library enqueue currently flashes `Added: …` before attempting to synchronize the optimistic queue append. Feed enqueue flashes the same message after the append command is accepted for delivery. Neither path has a correlated acknowledgement proving that a remote Player owner applied the append. Folder and artist item-count enqueue toasts were removed by PR #482, leaving these two success messages as inconsistent survivors.

The visible queue is updated optimistically before synchronization. Existing failure paths report an error and restore the prior queue when the append cannot be sent. See `specs/enqueue-feedback/spec.md` for the required user-visible behavior.

## Goals / Non-Goals

**Goals:**

- Make successful enqueue behavior consistent across library, folder, artist, and Feed entry actions.
- Preserve current queue synchronization, persistence, tracking retirement, failure reporting, and rollback ordering.
- Keep the change local to presentation feedback.

**Non-Goals:**

- Add a ctrl-protocol acknowledgement for queue append application.
- Change optimistic queue mutation into an asynchronous pending state.
- Suppress or reclassify enqueue error toasts.
- Change queue routing, admission, persistence, or rollback semantics.

## Decisions

### Remove success feedback rather than delay it

Delete the remaining `Added: …` flashes from both single-item enqueue paths. The queue's visible mutation already confirms the user's action without generating a transient in-app message or desktop notification.

Moving the library toast into the existing boolean success branch was rejected because that boolean confirms only that the command is supported and accepted by the local delivery channel; it does not confirm remote application. A toast there would fix the immediate success-then-rollback contradiction but retain a stronger success claim than the architecture can establish.

### Leave enqueue transaction flow untouched

Do not reorder append, synchronization, persistence, tracking retirement, or rollback. Remove only the success-feedback side effect and update the single-item helper's documentation so it no longer promises a confirmation flash.

This keeps the change reversible and avoids coupling notification cleanup to queue correctness work.

### Preserve actionable errors

Keep all existing error flashes. Unlike success confirmation, failure feedback explains why the expected queue mutation did not remain visible and may identify unsupported remote behavior or an unplayable selection.

## Risks / Trade-offs

- Users may miss a successful enqueue when the queue panel is not their focus → The queue remains the canonical confirmation, matching folder and artist enqueue behavior already shipped.
- Removing the Feed success toast also removes its desktop notification → This is intentional; enqueue is an immediate UI action, not a background completion requiring interruption.
- A remote owner may still reject an append asynchronously → Existing remote event and queue reconciliation behavior remains responsible; this change avoids making an unconfirmed success claim.

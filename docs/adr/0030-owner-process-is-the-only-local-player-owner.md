---
status: accepted
---

# The Owner Process Is the Only Local Player Owner

## Decision

Every local launch attaches to one per-user **Owner process**, the sole host
of the local Player owner. The terminal UI is always a Client; it never owns a
local Player or editable queue. Stay-alive is only the Owner process lifetime
policy: enabled keeps it alive after Clients leave, disabled shuts it down
when its last Client leaves. Admission is exclusive when that policy is off,
and shutdown admission is closed before teardown begins. A Client retains its
home link for its lifetime, including while another route is active.

The Client queue is adopt-only (`QueueView`); queue operations are answered by
the owner before subsequent input, and owner-minted lineage and playback-run
identity guard delayed source updates and stale observations.

## Considered options

- **In-process local owner behind the same protocol (rejected).** It leaves two
  hosting paths whose behavior can diverge; the Owner process exists so
  playback can outlive the TUI.
- **Client-pushed lifetime/settings (rejected).** Multiple Clients could
  disagree; per-user configuration is the single source of truth.
- **Drop and reconnect the home link on route changes (rejected).** With
  stay-alive disabled the owner would exit; with it enabled the Local view
  would freeze.
- **Fire-and-forget queue edits or provisional local overlays (rejected).**
  The next input could use stale state, and an overlay would be a second queue
  answer.

## Consequences

There is one local Player-owner host regardless of Stay-alive policy. A
stay-alive-off Owner process ends with its last Client; the enabled policy
preserves playback across Client lifetimes. Clients display owner-accepted
queue snapshots only, while the home link remains available for local state
and shutdown.

This decision supersedes the prior Bare/in-process local-owner distinction
and retires the Composed queue stage for local launches. Related decisions:
[ADR 0006](0006-single-instance-flock-and-socket-detection.md),
[ADR 0011](0011-library-scoped-daemon-routing.md),
[ADR 0014](0014-multi-connection-model.md),
[ADR 0015](0015-local-daemon-for-stay-alive.md),
[ADR 0016](0016-local-daemon-target-tracking.md),
[ADR 0017](0017-composed-and-bound-queue-stages.md),
[ADR 0019](0019-player-owners-resolve-service-playback.md),
and [ADR 0029](0029-stay-alive-process-owns-and-persists-its-queue.md).

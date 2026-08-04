## Why

When mbv submits a multi-item sequence to a generic Emby client, Emby exposes the current media item but not the client's queue position or occurrence identity. mbv therefore cannot reliably follow duplicate items, distinguish completion from skips, or tell which queue item a completion belongs to, leaving the queue UI silently desynchronized and remote consume ineffective.

## What Changes

- Track each multi-item sequence submitted to an attached Emby session and reconcile later session observations against its ordered occurrences.
- Represent startup, valid tracking, ambiguity, invalidation, and temporary session loss explicitly in the queue UI.
- Use mbv-issued commands, observed item transitions, playback position, runtime, and occurrence identity as bounded reconciliation evidence.
- Infer occurrence completion for qualifying adjacent transitions at the existing 95 percent near-end boundary, while withholding consume across ambiguity, unexplained gaps, explicit skips, and invalid tracking.
- Allow users to re-anchor invalid tracking, stop tracking without disconnecting, and confirm that manual queue edits will terminate tracking.
- Consume a completed occurrence from the queue promptly, using the same consume behavior as local playback — including the separate, opt-in Save on consume — rather than treating consume as a playlist operation.
- Keep tracking state process-local; nothing is restored after mbv exits.

## Capabilities

### New Capabilities

- `remote-playback-reconciliation`: Tracks submitted multi-item sequences against observable Emby session playback, exposes uncertainty and recovery, and consumes completed occurrences from the queue.

### Modified Capabilities

None.

## Impact

- Affects attached Emby-session polling, remote command correlation, queue presentation and input behavior, and consume decisions.
- Introduces process-local reconciliation state for generic Emby sessions; direct mbv and local playback queues remain authoritative through their existing paths.
- Adds no dependency and makes no Emby protocol guarantee beyond the session observations and remote commands already available.

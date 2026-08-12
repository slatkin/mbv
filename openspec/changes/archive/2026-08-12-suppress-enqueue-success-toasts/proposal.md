## Why

The remaining `Added: …` enqueue toasts are redundant with the queue changing on screen and can claim success before a remote Player owner has applied the append. Folder and artist enqueue confirmations were already removed as notification noise; single-item library and Feed enqueue should follow the same truthful, quiet behavior.

## What Changes

- Stop showing success toasts after single-item library and Feed enqueue actions.
- Keep actionable enqueue failure toasts and rollback behavior unchanged.
- Treat the visible queue update as the confirmation for a successful enqueue rather than adding a transient or desktop notification.
- Do not add protocol acknowledgement or redefine the existing queue-append send result as remote confirmation.

## Capabilities

### New Capabilities

- `enqueue-feedback`: Defines silent successful enqueue behavior while preserving actionable failure feedback.

### Modified Capabilities

(none)

## Impact

- Affects the single-item library enqueue helper in `src/app/queue_actions_playlist_mutation.rs` and Feed enqueue in `src/app/feed_tab_actions.rs`.
- Existing enqueue error handling, optimistic queue mutation, rollback, persistence, and Player owner synchronization remain unchanged.
- No ctrl protocol, API, configuration, or dependency changes.
- Resolves the remaining form of GitHub issue #462 after PR #482 removed the originally cited folder-enqueue toast.

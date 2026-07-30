## Why

Direct playback through `mbvd` can take long enough to become audible that a user cannot tell whether a command was received. The current invisible double-key guards do not cover every input path or command and can themselves make playback controls feel unresponsive, while the unacknowledged control protocol allows repeated commands and stale work to execute.

## What Changes

- Add immediate, request-correlated playback-intent feedback for direct `mbvd` playback.
- Make equivalent commands idempotent while an earlier intent is unresolved or still starting.
- Make newer play intents supersede older unresolved play intents, and make Stop invalidate all pending playback work.
- Make Next and Previous single-flight until the requested track change is confirmed.
- Replace remote toggle-pause transmission with an explicit desired paused state.
- Preserve deliberate same-item restart once playback has settled.
- Remove the double-Space and double-Escape guards so the first keypress acts immediately.
- **BREAKING**: revise the strict daemon control protocol to carry playback request identity, generation, desired state, and lifecycle outcomes.

## Capabilities

### New Capabilities

- `daemon-playback-intents`: Request-correlated, visible, supersedable, and idempotent playback control between the TUI and `mbvd`.

### Modified Capabilities

None.

## Impact

- Affects the TUI input/action path, playback status rendering, remote player proxy, daemon control dispatch, player runtime coordination, and control protocol compatibility.
- Requires explicit handling for accepted, applied, coalesced, superseded, and rejected outcomes.
- Requires slow play-item resolution to stop blocking newer control intents.
- Does not change local playback or control of attached Emby sessions.
- Adds no external dependency.

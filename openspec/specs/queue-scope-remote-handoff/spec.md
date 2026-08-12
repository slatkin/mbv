# queue-scope-remote-handoff Specification

## Purpose
Keeps the queue panel's visible tab pointed at wherever a queue selection was just sent to play,
so a Direct remote control hand-off never leaves the user looking at a stale Local tab.
## Requirements
### Requirement: Playing the queue cursor switches to the destination queue scope

When the user plays the item under the queue cursor (`QueuePlayCursor`) and that action hands
playback off to a remote session — whether via tracked-occurrence reconciliation or a plain
attached-sequence hand-off — the client SHALL set the visible queue scope to
`playback_target_queue_scope()` before sending the hand-off, so the panel already shows the
destination queue once the item lands there.

Under Direct remote control, `playback_target_queue_scope()` resolves to Remote, so the panel
switches to the Remote tab. When there is no Direct remote control (a plain attached session with
no separate remote queue), the scope resolves to Local and switching is a no-op — no remote tab is
shown, since none exists.

#### Scenario: Playing the cursor item under Direct remote control switches to Remote

- **WHEN** Direct remote control is active, the queue panel is showing the Local tab, and the user
  plays the item under the queue cursor
- **THEN** the client SHALL send the item to the remote session
- **THEN** the queue panel SHALL switch to show the Remote tab

#### Scenario: Playing the cursor item on a plain attached session stays on Local

- **WHEN** the client is attached to a remote session without Direct remote control (no separate
  remote queue exists) and the user plays the item under the queue cursor
- **THEN** the client SHALL send the item to the remote session
- **THEN** the queue panel SHALL remain on the Local tab


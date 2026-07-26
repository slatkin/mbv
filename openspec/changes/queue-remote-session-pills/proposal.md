## Why

The mbv queue header currently uses a remote pill whose active styling is easy to confuse with the local state, and an attached mbv-to-emby session is not represented in the queue scope at all. Clear, consistent pills will distinguish an interactive mbv-to-mbv remote queue from a display-only non-mbv session without changing queue behavior.

## What Changes

- Restyle the active mbv-to-mbv remote queue pill to use yellow foreground on an aqua background.
- Add a right-side, non-interactive attached-session pill for mbv-to-emby connections, using the existing remote icon and host/device label resolution.
- Keep the attached-session pill display-only: clicking it and keyboard focus/action must be no-ops, while the existing direct mbv remote local/remote split remains interactive.
- Preserve current attached-session queue behavior and define width/layout handling for the additional pill.
- Add focused rendering, layout, and input tests for both connection types, exact colors, label fallback, icon reuse, and no-op interaction.

## Capabilities

### New Capabilities

- `queue-remote-session-pills`: Queue-header scope pills for direct mbv remotes and attached non-mbv sessions, including styling, labeling, layout, and interaction behavior.

### Modified Capabilities

- None.

## Impact

- Affected rendering and layout code in `src/app/render/queue.rs`, `src/app/render/chrome_status.rs` or shared remote-pill helpers, and `LayoutMain` queue-scope areas.
- Affected input dispatch and hit-testing in `input_mouse_dispatch.rs` and corresponding keyboard/action handling, with the attached-session pill intentionally non-interactive.
- Affected tests in queue rendering, queue scope, remote status, layout/input dispatch, and any snapshots or width calculations covering queue-header pills.
- No persistence, network protocol, or external API changes; the change is limited to in-process TUI presentation and interaction routing.

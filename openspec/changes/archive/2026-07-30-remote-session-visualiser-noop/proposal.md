## Why

The visualiser key currently toggles local or daemon visualiser state even while the UI is attached to a remote playback session. In that mode the visualiser is not a supported local action, and pressing `v` can produce misleading state changes or remote spectrum commands.

## What Changes

- Treat `v` as a no-op while a remote session is connected.
- Keep the key consumed by the visualiser handler so it does not fall through to another binding.
- Preserve existing visualiser behavior for local playback and supported direct daemon connections.
- Add focused coverage proving that a connected remote session's visualiser state and commands are unchanged.

## Capabilities

### New Capabilities

- `remote-session-visualiser-noop`: The visualiser key is inert while an attached remote session is active.

### Modified Capabilities

- `visualizer`: Restricts the toggle action to playback modes where the visualiser is supported.

## Impact

- Affected input handling in `src/app/input.rs`.
- Affected focused visualiser/input tests.
- No new dependencies, persistence changes, or remote protocol changes.

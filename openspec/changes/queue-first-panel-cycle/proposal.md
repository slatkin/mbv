## Why

The first single-panel view currently shows the library panel, even though the queue is the useful default when a user intentionally reduces the interface. The same library-first behavior appears when `x` is pressed from the wide two-panel layout, when the terminal narrows below the mini-view breakpoint, and when the application starts narrow.

## What Changes

- **BREAKING** Reverse the wide `x` cycle to `Both -> QueueOnly -> LibraryOnly -> Both`, making the queue the first single-panel destination.
- Make mini-view entry select the queue panel when the application starts below the breakpoint or crosses from wide to narrow.
- Keep narrow `x` behavior as a two-state toggle between queue-only and library-only.
- Preserve the existing rule that mini-view state is ephemeral and that widening restores the prior wide panel mode and focus.
- Update panel-mode tests, user-facing descriptions, and the panel-mode specification to describe queue-first behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `panel-mode`: Change the wide cycle order and make queue-only the default state whenever mini-view is first entered.

## Impact

- `src/app/action.rs`: wide panel-mode cycle and narrow toggle behavior.
- `src/app/construct.rs` and `src/app/render/mod.rs`: initial and width-transition mini-view selection.
- Panel-mode input/render tests and user-facing help text.
- `openspec/specs/panel-mode/spec.md` through the applied delta.
- No protocol, daemon, provider API, dependency, or playback changes.

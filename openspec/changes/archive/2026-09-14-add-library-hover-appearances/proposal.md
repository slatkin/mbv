## Why

Clickable tabs and Library selector pills currently give no visual response when the pointer is over them, making their mouse affordance less discoverable than their click behavior. The existing mouse subscription and retained-geometry architecture already admits pointer movement, so mbv can add this feedback without adding another input path.

## What Changes

- Give each visible tab label a transient hover appearance while the pointer is inside its latest painted hit region.
- Give each pill in the Library panel's main Selector row a transient hover appearance while the pointer is inside its latest painted hit region.
- Keep hover presentation-only: it does not select, focus, activate, scroll, persist, or emit shell work.
- Keep selected appearance dominant; hovering an already-selected target does not replace its selected styling.
- Clear hover when the pointer moves to a gap, another surface, or another target.
- Evaluate the initial styles in the running TUI at Narrow and Wide Panel modes and tune them before accepting the implementation.
- Exclude Workspace-selector pills, selection-modal pills, list controls, Queue/playback controls, and media rows.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mouse-input`: Add presentation-only hover feedback for tab labels and the Library panel's main Selector-row pills.

## Impact

The change affects the Tab panel and Library panel Interactive Components, their existing tab/pill Render Components and semantic theme roles, plus focused component, buffer, and live-tick integration coverage. It changes no Service, playback, queue, persistence, protocol, configuration, or public API behavior and adds no dependency.

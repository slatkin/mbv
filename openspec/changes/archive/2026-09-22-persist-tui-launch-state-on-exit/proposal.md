# Proposal

## Why

Stay-alive permits several complete TUI Clients to run concurrently, but each currently writes shared UI preferences and browse positions while it is still running. These live writes make independent sessions contend over disk state even though saved UI state only needs to seed a later launch.

## What Changes

- Replace live persistence of restorable TUI navigation state with one snapshot written when the TUI exits normally.
- Persist one coherent launch location: the selected tab, that tab's selected main Selector pill, that pill's selected library item, and whether Library or Queue held Panel focus.
- Restore saved tab, pill, and item identities when they still exist; otherwise fall back at each missing level to the first guaranteed valid choice in presentation order.
- Do not persist state for unselected tabs, the selected Queue item, nested Workspace pills, overlays, searches, multi-selection, scroll offsets, or other transient presentation state.
- Keep running TUI Clients independent in memory. When concurrent Clients exit, the last completed exit supplies the launch snapshot for the next TUI.
- Leave explicit configuration, playback lifecycle/progress, queue authority/persistence, caches, and auto-reconnect persistence outside this UI launch-state lifecycle.

## Capabilities

### New Capabilities
- `tui-launch-state`: Defines the exit-time TUI launch snapshot, restoration order, stable-identity fallback rules, and exclusions.

### Modified Capabilities
- `interactive-component-framework`: Changes persisted resting UI state from navigation-event writes to an exit snapshot without making the shell a live mirror of component-owned state.
- `local-daemon-thin-client`: Replaces per-library browse-position restoration with the same selected-tab launch snapshot used in Bare mode while preserving independent concurrent Clients and playback continuity.

## Impact

- Affects TUI startup and teardown, preference/library-position persistence, destination snapshot extraction and discrete restoration, and related persistence tests.
- Requires each selectable Library destination to expose its current main Selector pill and selected library-item identities at teardown without continuous shell mirroring.
- Changes the saved-state format and restoration behavior; older saved preferences and library-position data need a defined compatibility path.
- Does not change ctrl protocol, Local daemon behavior, Player ownership, canonical queue behavior, Service APIs, rendering, or dependencies.

# Proposal

## Why

PR #759 extracted shared row-flow mechanics but left the complete tree control inside the Grouped Music destination as `MusicTreeBrowser`. A second tree destination would still have to copy or rebuild the model adapter, tree state, filtering, expansion, navigation, painting, marquee, scrollbar, mark aggregation, and retained geometry, so the promised reusable tree control does not yet exist.

## What Changes

- Extract one destination-neutral embedded `TreeBrowser<Target>` that implements TuiRealm `Component` and is the complete nesting counterpart to the flat `WideMediaList<Target>` component.
- Make the shared Interactive Component own the complete tree behavior and presentation state: model reconciliation, expansion, filtering, cursor and viewport integration, keyboard operations, ordered marks and aggregate mark state, retained hit geometry, marquee, indentation, zebra stripes, selected-row treatment, metadata gutter, and scrollbar. Its `Component::view` delegates all Ratatui painting to one destination-neutral Render Component.
- Limit destinations to supplying plain `TreeNode<Target>` values and translating emitted stable-target intents. Destinations supply no trait implementation, callback, closure, model/query/state object, renderer, painter, navigation rule, filter matcher, aggregation algorithm, or action-order policy.
- Convert Grouped Music to consume the shared `TreeBrowser` while preserving its current behavior and rendered style.
- Remove `MusicTreeBrowser` and the destination-specific copies of tree-control behavior after Music adopts the shared component.
- Keep internal tree-library types private to the shared Interactive Component and its shared Render Component in production and tests.
- Do not migrate TV, change any Hero or Workspace, add a second consumer, or change user-visible Music behavior in this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `shared-list-components`: Strengthen the nesting-list contract from shared mechanics plus a destination-specific tree adapter to one reusable, complete `TreeBrowser` Interactive Component—the nesting counterpart to the complete flat-list component—which destinations populate only with typed data.

## Impact

Affected code is concentrated in `src/app/components/list/`, the current `src/app/components/music_tree*.rs` implementation, `src/app/components/music_content*.rs`, the Library panel's list adapter, and existing Music tree tests. The `tui-treelistview` dependency was removed (57f029f9) with no replacement; internal tree-library types stay private to those two shared modules in production and tests. Public behavior, Music presentation, TV, Hero content, Workspace content, shell effects, and playback are unchanged.

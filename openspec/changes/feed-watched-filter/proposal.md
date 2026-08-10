## Why

Feed playback state now roams through the shared feed-entry store, but the Feeds tab does not load or expose that state while browsing. Issue [#494](https://github.com/slatkin/mbv/issues/494) completes the RSS state milestone by making played state visible and filterable after the prerequisite [#493](https://github.com/slatkin/mbv/issues/493) landed.

## What Changes

- Hydrate fetched feed entries with their stored position and played state using one feed-scoped state scan per subscription.
- Add an unmodified `w` binding on the Feeds tab that cycles All, Watched, and Unwatched views without writing playback state.
- Apply the selected filter consistently to All and per-subscription groups, rendering, navigation, mouse selection, play, and enqueue actions.
- Show the active watched filter and a compact played-state indication in feed rows; resume-position display remains optional.
- Keep feed browsing and playback available when shared state is unsupported, disconnected, or fails, with unavailable state treated as stateless and unplayed.
- Verify position and played state roaming across two machines and the full stateless fallback with the shared-data daemon unavailable.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `feed-subscriptions`: Feed browsing hydrates roaming playback state and provides a filter-only watched-state view across every feed group.

## Impact

- Feeds-tab state, actions, key handling, rendering, mouse row mapping, and focused App tests under `src/app/`.
- Consumption of the existing capability-gated feed-entry prefix scan through `SharedClient`; no protocol version, capability string, storage schema, or dependency change.
- Feed domain documentation that still describes entries as permanently lacking persisted playback state.
- Manual two-machine acceptance verification spanning the shipped feed-entry store, event-driven playback wiring, watched filter, and stateless fallback.

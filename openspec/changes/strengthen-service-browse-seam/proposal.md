## Why

The left-panel tab type distinguishes Home, Emby, Audiobookshelf, and Feeds, but shared input and action paths repeatedly infer Emby from the absence of another tab kind. Recent Audiobookshelf keyboard and mouse panics were patched with guards, yet the underlying negative-routing pattern remains easy to miss and blocks safe provider-native playback activation.

## What Changes

- Establish one exhaustive app-level browse-dispatch boundary for Home, Emby libraries, Audiobookshelf libraries, and Feeds.
- Require Emby-only handlers to receive an explicitly selected Emby library rather than recovering one from mutable tab state or defaulting a missing index to library zero.
- Route keyboard, mouse, refresh, rendering, help, context-menu, and tab-activation behavior through the selected browse target without “all other tabs are Emby” fall-through.
- Preserve separate Emby, Audiobookshelf, and Feed browse models; do not introduce a provider-neutral browse item or library abstraction.
- Preserve `QueueItem` as the shared playback boundary while keeping Audiobookshelf episode activation read-only until the separate local-playback change is applied.
- Remove legacy tab-position mappings that cannot uniquely represent mixed Emby and Audiobookshelf tab strips.

## Capabilities

### New Capabilities

- `service-browse-dispatch`: Exhaustive Service-specific left-panel dispatch, target-safe action handling, and matching refresh/help behavior across Home, Emby, Audiobookshelf, and Feeds.

### Modified Capabilities

None.

## Impact

- App-local tab identity and tab-position mapping in `src/app`.
- Keyboard and mouse input dispatch, provider-specific browse actions, refresh behavior, context menus, and help presentation.
- Emby browse helpers whose current names or implicit `self.tab` lookup conceal an Emby-only precondition.
- Layout hit testing may be separated by provider, but provider-native catalog types and `mbv-core` APIs remain unchanged.
- No dependency, persistence format, ctrl protocol, queue representation, Service credential, or playback behavior change.

# Proposal

## Why

Latest content now appears both on Home and in the TV library, while other libraries require a trip to Home to see their own Latest items. Give each destination its own Latest pill and stop duplicating those sections on Home.

## What Changes

- Every visible Emby library, including TV, Movies, Music, Home Videos, and other library kinds, has a `Latest` pill alongside its existing selector choices. TV keeps its existing Latest behavior and size-dependent default; other destinations retain their existing opening selection.
- Each Audiobookshelf podcast library and the Feeds tab gains a `Latest` pill alongside its existing choices. Audiobookshelf book libraries are out of scope.
- Move the existing Home Latest content, provider dates, new-content markers, and playback actions to the corresponding destinations; Home retains Continue Watching and no Latest pills.
- **BREAKING (configuration):** Sunset `hidden_latest` and its Settings control. An existing `hidden_latest` value no longer suppresses Latest; do not remove or reinterpret `hidden_libraries`.

## Capabilities

### New Capabilities

- `destination-latest-modes`: Per-destination Latest selector, data, new-content acknowledgement, and activation rules across Emby, Audiobookshelf podcasts, and Feeds.

### Modified Capabilities

- `home-latest-sections`: Remove Home Latest sections and transfer their existing presentation/marker behavior to destination modes; retain Home Continue Watching.
- `tv-library-content-modes`: TV Latest becomes destination-only, with no Home counterpart; retain its existing TV mode choices and shared snapshot behavior where applicable.
- `audiobookshelf-podcast-library-ui`: Add Latest to the podcast selector without replacing the existing state/show choices.
- `feed-subscriptions`: Add Latest to the Feeds selector without replacing its current browsing and refresh contracts.

## Impact

Home content and launch-state selection, Emby/TV/Music/podcast/Feeds embedded content owners, Library Panel selector projections, destination fetch and refresh dispatch, settings/configuration parsing and serialization, new-content marker state, and focused component/shell integration tests. Reuse current Service data sources; no new remote endpoint or dependency is planned. Existing TV tree-browser work remains independent: Latest is a flat mode, not a tree branch.

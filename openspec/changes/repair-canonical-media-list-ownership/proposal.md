## Why

The canonical media-list controls share list mechanics, but several destinations still split cursor, scroll, painting, and row geometry between parent state and helper-like controls. Correcting that ownership now prevents multi-select and later list work from building on contradictory TuiRealm composition, while preserving the wheel policy from #674 as an input constraint.

## What Changes

- Make `WideMediaList<Target>` and `InlineMediaBrowser<Target>` persistent embedded TuiRealm `Component`s owned by their destination parents, without mounting, focusing, subscribing, or assigning them independent `ComponentId`s.
- Give the active embedded control sole authority over live cursor, scroll, viewport, list movement, painting entry point, and list-row geometry.
- Replace Home and Feeds lockstep Wide/Inline mutation with one active-control owner and a one-shot stable-target `ViewportAnchor` handoff at responsive transitions.
- Remove render-time canonical control construction and parent cursor/scroll mirrors from the existing canonical destinations: Home, generic Emby/Movies/homevideos/podcast browsing, TV, grouped Music, Audiobookshelf Podcast and Book, Feeds, and Queue.
- Preserve the accepted non-hero two-column Emby catalog carve-out and leave Inline Search, Search sidebar results, Playlists/open-playlist rows, Settings, and Sessions outside this change.
- Preserve parent and shell authority for provider workspaces, Service effects, images, playback, persistence, mouse gesture recognition, and typed intent translation.
- Correct stale canonical-list specifications, terminology, comments, and architecture documentation, and add structural ratchets plus focused component, rendering, breakpoint-handoff, and live-tick regression evidence.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `canonical-media-lists`: Require real persistent embedded TuiRealm components, active-control-only responsive ownership, and current mouse hit-resolution behavior across every existing canonical destination.
- `interactive-component-framework`: Tighten the embedded-control ownership contract so delegated list state, painting, geometry, and movement cannot remain split across the destination parent and control.

## Impact

- Affects canonical controls and their existing destination parents under `src/app/components/`, their render-layer entry points under `src/app/render/`, shell projection/persistence adapters, focused and live-tick tests, architecture scan rules, `CONTEXT.md`, and the interactive-surface documentation.
- No daemon, protocol, Service, playback, queue-authority, external API, dependency, or non-canonical list behavior changes.
- Implementation starts from the accepted #674 result and must preserve its painted-owner arbitration, one-row wheel movement, throttle, routing, and live-tick proofs.

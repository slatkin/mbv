## Why

Issue #640 reports that the Audiobookshelf podcast Wide surface bypasses the shared hero-on-left arrangement. It currently reserves a separate `right_panel`/hero panel and uses provider-local column sizing, unlike the shared presentation used by other hero-bearing browse surfaces.

## What Changes

- Route Audiobookshelf podcast Wide rendering through the shared Hero-on-left placement: hero workspace on the left and one fixed-row show/episode rail on the right.
- Restore Wide pills and right-rail framing while preserving the provider-specific episode workspace, images, and all Normal/Narrow behavior.
- Characterize current width-boundary and re-anchor behavior at 81 and 82 columns with Ratatui `TestBackend` before changing geometry.
- Split `src/app/render/components/audiobookshelf_podcast.rs` if implementation would exceed the 800-line source limit.

## Capabilities

### Modified Capabilities

- `right-panel-arrangements`: Audiobookshelf podcast Wide uses the shared hero-on-left arrangement and a single-column right rail with pills.
- `ui-design-system`: The podcast rail uses shared semantic framing and geometry; Narrow remains the inline presentation.

## Impact

Render arrangement/component, interactive component geometry projection, and Audiobookshelf podcast shell layout predicate only. No provider, playback, queue, protocol, canonical-list, keyboard, or mouse changes. This is an independent change stacked on the PR #606 feature branch; it does not alter PR #606's merge rule.

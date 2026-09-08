# Repair Canonical Media-List Ownership

## Why

The list controls still expose a helper-style paint API that splits a frame's geometry between callers and painters. Repair that seam first, before applying it across destinations, so the shared contract is small enough to verify and later migrations do not inherit duplicate geometry ownership.

## What Changes

- Make `WideMediaList<Target>` and `InlineMediaBrowser<Target>` persistent embedded plain TuiRealm `Component`s with a semantic-only per-frame paint policy and retained read-only paint result.
- Preserve parent-owned panel framing and row-flow placement; make the child `Component::view` paint the supplied row flow once and retain current-frame claim/content geometry, selected-row facts, and Inline admitted-detail rectangle for parent reads and point resolution.
- Prove the seam in Queue's fixed-row Wide presentation and Grouped Music's existing presentations, while preserving their mounted-parent gesture ownership, typed requests, existing visuals, and PR #683 wheel behavior.
- Remove only the Queue/Grouped Music row-map reconstruction and compatibility geometry made redundant by the retained child result.
- Defer responsive active-control handoff, Browser identity/grid work, Home, TV, Feeds, Audiobookshelf, universal ratchets, documentation sweeps, and multi-select to follow-on changes under issue #681.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `canonical-media-lists`: Define the embedded component view/result seam and its Queue/Grouped Music proof behavior.
- `mouse-input`: Define retained current-frame point resolution for the bounded migrated proof while preserving compatibility behavior for untouched destinations.

## Impact

- Affects `src/app/components/media_list/`, Queue and Grouped Music components, their existing render entry points, and focused component/render/live-tick tests.
- No Service, playback, queue-authority, persistence, layout-breakpoint, responsive-handoff, external API, dependency, or non-proof-destination behavior changes.
- Starts from landed PR #683 (`6b58a608`) and preserves its painted-owner arbitration, one-row wheel movement, throttle, routing, and real-`Application::tick()` evidence.

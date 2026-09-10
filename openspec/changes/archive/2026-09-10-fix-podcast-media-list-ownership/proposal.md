## Why

The Audiobookshelf podcast destination still constructs its Wide show list per frame and duplicates show position and hit geometry in parent/content state, violating the canonical media-list ownership contract and blocking umbrella row 7.1. Its presentation also suppresses episode content in the inline hero and diverges from the Emby reference, including where the current library-UI specification entrenches that deviation.

## What Changes

- Make persistent Wide and Inline show controls the sole live owners of podcast-show cursor, scroll, retained hit geometry, and responsive `ViewportAnchor` handoff; keep the content-struct cursor only as effect-path state keyed by component-resolved values.
- Delete the podcast parent’s bespoke show-list geometry, movement, scroll, and paint-writeback path; resolve click and wheel interaction through the active control’s retained current-frame geometry, with wheel claims only over that control.
- Preserve the existing shell-owned detail-fetch and library-position effects, event-scoped projection, episode workspace state, and intentional `AudiobookshelfPodcastEpisodeTransition` no-op.
- Converge visible podcast presentation on the Emby/TV reference: show episode rows in inline detail with filter pills, project canonical episode duration and played/active states, show the title in Wide hero, and apply the accepted pill truncation/overflow and chrome policy.
- Correct alphabetical bucket-pill requirements to describe the deliberate shared Books behavior: only non-empty surname ranges, without `All` or `#` pills.
- Add focused component/media-list, render-buffer, and Wide plus Normal/Narrow shell-tick coverage for active-control movement, retained hits and claims, one-painter/no-underpaint ownership, presentation convergence, and target-plus-offset breakpoint handoff.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audiobookshelf-podcast-library-ui`: Make inline selected-show detail include downloaded episode rows and episode filter pills in parity with the TV reference, and define alphabetical bucket pills as non-empty surname ranges without `All` or `#`.

## Impact

- Affected area: `AudiobookshelfPodcastComponent`, its podcast render component and shell projection, Audiobookshelf browse cursor effect path, app test registration, and focused component/render/shell-tick tests.
- No API, dependency, persistence-format, Service lifecycle, playback/enqueue, episode modal, image, download/progress, provider bucket-data, router, mounted identity, or shared-helper change.

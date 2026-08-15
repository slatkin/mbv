## Why

The Audiobookshelf podcast tab currently renders a flat mixed list that does not match mbv's established TV-library browsing experience. Rework it into the same show-detail interaction so podcast shows, played-state filtering, and episodes have a clear hierarchy and consistent visual language.

## What Changes

- Replace the flat Audiobookshelf paragraph list with the TV Shows tab's top-pinned selected-show hero and show list below it.
- Map Series to podcast shows, Series Primary images to Audiobookshelf covers, season selectors to `All`/`Played`/`Unplayed`, and TV episodes to downloaded podcast episodes.
- Preserve the TV tab's geometry, image placement, focus styling, scrolling, responsive behavior, and selection-mode interaction outside those domain substitutions.
- Preserve stable show and episode selection, read-only progress presentation, and inert episode activation.
- Remove personalized shelves from the supported Audiobookshelf podcast UI and do not expose shelf rows in the main list.
- Keep playback, queue mutation, progress writes, and live updates out of scope.

## Capabilities

### New Capabilities

- `audiobookshelf-podcast-library-ui`: TV-identical presentation and navigation for Audiobookshelf podcast shows, played-state filters, and downloaded episodes.

### Modified Capabilities

None.

## Impact

- Audiobookshelf browse state, rendering, keyboard/mouse navigation, and progress filtering in `src/app`.
- Audiobookshelf shelf loading/state/API usage introduced by the prior browse change will be removed from the visible and navigable model.
- No new dependencies, playback protocol, queue model, daemon behavior, or persistence changes.

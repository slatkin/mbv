## Why

The Audiobookshelf podcast tab currently renders a flat mixed list that does not match mbv's established TV-library browsing experience. Rework it into the same show-detail interaction so podcast shows, played-state filtering, and episodes have a clear hierarchy and consistent visual language.

## What Changes

- Replace the flat Audiobookshelf paragraph list with a TV-style show list and selected-show detail view.
- Render the selected podcast's `All`, `Played`, and `Unplayed` filters using the existing pill-selector treatment.
- Render downloaded podcast episodes in a structured episode table beneath the selected show.
- Preserve stable show and episode selection, read-only progress presentation, and inert episode activation.
- Remove personalized shelves from the supported Audiobookshelf podcast UI and do not expose shelf rows in the main list.
- Keep playback, queue mutation, progress writes, and live updates out of scope.

## Capabilities

### New Capabilities

- `audiobookshelf-podcast-library-ui`: TV-style presentation and navigation for Audiobookshelf podcast shows, played-state filters, and downloaded episodes.

### Modified Capabilities

None.

## Impact

- Audiobookshelf browse state, rendering, keyboard/mouse navigation, and progress filtering in `src/app`.
- Audiobookshelf shelf loading/state/API usage introduced by the prior browse change will be removed or left unused according to the implementation boundary.
- No new dependencies, playback protocol, queue model, daemon behavior, or persistence changes.

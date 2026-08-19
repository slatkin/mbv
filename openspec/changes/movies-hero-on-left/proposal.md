## Why

The wide Movies tab currently uses the hero-on-top two-column arrangement, while Home already
provides the desired read-only selected-movie card in its hero-on-left wide view. Movies should use
that exact card so the same media presentation is not reimplemented or allowed to drift.

## What Changes

- **BREAKING (visual):** Assign the Movies library tab to the hero-on-left wide arrangement.
- Keep the existing hero-on-top arrangement as the narrow fallback below the shared breakpoint.
- Put the Movies tab's letter-range pills in the hero-on-left right rail above the list.
- Render the Movies list as one column in the right rail; its cursor remains the source of the
  selected movie.
- Make the left hero a read-only projection of the selected movie. It never receives focus or owns
  activation; keyboard navigation and activation remain with the right-hand list.
- Reuse the exact Home Movies Latest selected-media hero card, including its layout, artwork
  selection, watch-state indicator, metadata, overview, and image cache behavior.
- Leave TV shows, podcasts, feeds, home videos, and their existing arrangements unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `right-panel-arrangements`: Movies move from the hero-on-top wide assignment to the hero-on-left
  assignment, with a non-focusable hero pane and a single-column right-hand list.
- `library-list-hero`: The Movies hero uses the same Home wide selected-media card and follows the
  right-rail list cursor without becoming an activation surface.

## Impact

- Affected rendering: the Movies library dispatch, the shared hero-on-left composition, the Home
  selected-Emby hero-card model/painter, and the right-rail letter-pill/list placement.
- Affected input/layout bookkeeping: the Movies list must publish right-rail geometry while keeping
  cursor movement and activation on the list path; no new hero focus state is introduced.
- No Service, playback protocol, persistence, or external API changes.

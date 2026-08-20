## Why

Hero placement is still Service- and surface-dependent: Audiobookshelf and Feeds retain pinned hero-on-top paths, Home retains one in its narrow presentation, and some Emby surfaces do not adopt hero-on-left when wide. This contradicts the live inline-hero requirements and leaves an obsolete arrangement, fallback, border variant, and vocabulary for later UI work to preserve.

## What Changes

- Make hero placement invariant across every hero-bearing browse surface: hero-on-left when wide enough, otherwise an inline hero at the selected row.
- Preserve the existing minimum-height guard for hero-on-left; when it cannot fit, use the inline presentation rather than hero-on-top.
- Move Audiobookshelf podcasts and Feeds to hero-on-left when wide and inline heroes when narrow.
- Keep Audiobookshelf books, grouped Music, Movies, TV shows, and Home hero-on-left when wide while replacing every remaining narrow pinned hero with an inline hero.
- Give Emby podcast and home-video browsing an explicit wide hero-on-left presentation instead of a wide inline/two-column fallthrough.
- Preserve each surface's hero content, artwork, selectors, detail rows, focusability, and playback behavior while changing placement.
- Remove the hero-on-top arrangement, geometry helper, border variant, fallback behavior, tests, comments, and domain vocabulary rather than retaining a dormant compatibility path.
- Add an ADR recording hero-on-left and inline hero as the only supported hero placements, and update `CONTEXT.md` to remove the conflicting Hero-on-top term.
- Reconcile the contradictory live arrangement and surface specifications before the `enforce-mbv-ui-design-system` change begins.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `right-panel-arrangements`: Replace two wide hero arrangements with hero-on-left wide and inline-hero fallback as the only responsive placement rule for hero-bearing browse surfaces.
- `library-list-hero`: Remove pinned/top fallback requirements and standardize inline flow, suppression, and inert hit behavior below the wide geometry threshold.
- `library-list-columns`: Remove hero-on-top-specific column rules; hero-on-left browsers are one column and inline presentations use one column.
- `music-library-hero`: Replace grouped Music's narrow hero-on-top fallback with its inline selected-album detail.
- `audiobookshelf-podcast-library-ui`: Replace the podcast tab's top hero with a wide left podcast workspace and narrow inline selected-show detail.
- `audiobookshelf-book-browsing`: Replace the book tab's narrow top fallback with inline selected-book detail while retaining its wide left workspace.

## Impact

- Rendering and geometry under `src/app/render/`, especially Home, generic Emby lists, Audiobookshelf podcast/book browsing, Feeds, hero framing, hero-left composition, row maps, scrolling, and hit targets.
- `LayoutMain` hero geometry and mouse dispatch must continue distinguishing inert inline hero rows from interactive detail rows without preserving top-hero activation behavior.
- Focused Ratatui tests and temporary visual captures for all hero-bearing surfaces at wide, narrow, and short-height dimensions.
- Live OpenSpec capabilities listed above, `CONTEXT.md`, and a new presentation ADR.
- No Service API, playback, queue, persistence, protocol, configuration, or dependency changes.
- Unblocks issue #563 and the paused `enforce-mbv-ui-design-system` change after this baseline lands.

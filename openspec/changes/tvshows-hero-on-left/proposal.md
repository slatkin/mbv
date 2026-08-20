## Why

Wide TV libraries currently use the hero-on-top arrangement, even though a selected Series has a
natural interactive detail surface: its seasons and episodes. TV should adopt the Music-style
hero-on-left workspace so episode browsing stays visible beside the Series library while preserving
the existing season filtering and playback behavior.

## What Changes

- Assign the TV shows library to the hero-on-left arrangement at or above the shared breakpoint.
- Keep the existing hero-on-top, one-column TV presentation below the breakpoint.
- Render the selected Series' artwork, metadata, and overview in the left pane.
- Keep the selected Series' episode list persistently visible in the left pane, analogous to Music's
  persistent track preview.
- Add the existing TV season pill bar to the left pane above the episode list; it filters episodes
  for the selected Series without replacing the right-rail library pills.
- Make Enter activate episode selection in the visible left-pane episode list, retaining existing
  episode navigation, season switching, playback, and Escape/Backspace behavior.
- Put TV letter-range pills or active search, followed by a one-column Series list, in the right rail.
- Keep the right-rail Series cursor as the source of the selected Series shown on the left, including
  when inline search is active.
- Preserve existing narrow TV hero-on-top behavior and existing TV alphabet filtering semantics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `right-panel-arrangements`: TV shows move from the hero-on-top wide assignment to the interactive
  hero-on-left assignment, with the Series browser on the right and episode detail on the left.
- `library-list-hero`: Wide TV shows persistently render the selected Series detail, season pills,
  and episode list beside the one-column Series browser; the left pane becomes focusable for episode
  selection while remaining a projection during Series browsing.

## Impact

- Affected TUI rendering and layout bookkeeping for TV library dispatch, hero-on-left panes, Series
  detail, season pills, episode rows, and right-rail list geometry.
- Affected keyboard and mouse routing for switching between Series browsing and episode selection,
  season changes, episode activation, and right-rail selection.
- Existing Series detail fetching, TV letter filtering, playback, persistence, Service APIs, ctrl
  protocols, and stored media data remain unchanged.

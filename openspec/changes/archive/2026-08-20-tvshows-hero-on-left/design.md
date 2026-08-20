## Context

See `proposal.md` for motivation and user-visible scope. The current TV library uses the shared
hero-on-top list renderer. A selected Series is represented by an inline detail block that fetches
`SeriesDetail`, renders season pills and episodes, and uses `series_selection` plus
`series_season_cursor` for episode interaction. Wide Music already provides the target composition:
persistent detail content on the left, a one-column browser on the right, and pane focus derived from
existing library state.

The existing right-panel arrangement owns the breakpoint and pane geometry. TV must consume those
helpers rather than introduce another width-specific layout. Narrow TV must continue through the
existing hero-on-top path.

## Goals / Non-Goals

**Goals:**

- Compose wide TV from the existing hero-on-left geometry and shared right-rail chrome.
- Keep Series browsing on the right and show the selected Series' persistent episode workspace on the
  left.
- Reuse the existing Series detail data, season cursor, episode cursor, fetching, and playback paths.
- Make the season pill row local to the selected Series workspace, separate from TV library letter pills.
- Preserve active-search selection, one-column cursor movement, mouse targets, and narrow rendering.
- Keep the left episode workspace's focus treatment consistent with wide Music.

**Non-Goals:**

- Changing the Emby Series detail or episode APIs.
- Changing TV letter-range filtering or saved library-position semantics.
- Redesigning season labels, episode row content, artwork selection, or hero metadata.
- Adding a third responsive arrangement or a new global panel-focus concept.
- Changing playback behavior beyond routing the existing episode activation through the focused pane.

## Decisions

### 1. Use a TV-specific wide composition built from shared arrangement primitives

Add a focused TV wide renderer, dispatched only for the top-level `tvshows` library at the shared
wide breakpoint. It uses the shared pane split, right-rail pill slot, list panel border, and one-column
row renderer. The narrow fallback remains the current list path.

**Alternative rejected:** Adding TV conditions throughout the generic hero-on-top renderer. That would
mix two arrangements and make the season/episode interaction depend on incidental top-hero layout.

### 2. Make the left workspace persistent, with focus represented by existing selection state

The left pane always renders the selected Series detail and the current season's episode preview.
`series_selection == None` means the right Series browser owns focus and the episode rows are a
non-cursor preview. `series_selection == Some(index)` means the left episode list owns focus, matching
Music's `album_track_focus` distinction without introducing a second global focus enum.

Entering episode selection starts at the existing initial episode. Escape or Backspace clears the
selection state and returns visual focus to the Series browser. Up/Down, Enter, and season switching
continue using the existing TV actions while selection is active.

**Alternative rejected:** Keeping episodes hidden until Enter. That preserves the old visual behavior
but fails the persistent-track-preview model and makes the wide left pane look empty during normal
Series browsing.

### 3. Keep the two pill bars in their owning panes

The right rail retains the existing TV letter-range pills, or the active search box in their place.
The left workspace renders the selected Series' season pills directly above its episode list. Season
selection changes only `series_season_cursor` and the episode source; it never changes the right-rail
Series filter.

**Alternative rejected:** Reusing the right-rail pill slot for seasons. A season belongs to the
selected Series, while the right rail browses the whole library; combining them would make the active
filter ambiguous and hide the existing TV library navigation.

### 4. Resolve the selected Series from the active right-rail source

When search is active, the left workspace follows the search result cursor. Otherwise it follows the
current top-level Series cursor. A Series change clears or safely reinitializes episode-selection
state and resets the season/episode view as needed so detail from the previous Series cannot appear
under the new title.

### 5. Use arrangement-produced hit targets for both panes

The wide TV renderer publishes the right-rail list geometry and one-column row map as the library
browse surface. It additionally publishes episode-row and season-pill hit targets in the left pane.
Episode clicks enter episode selection; season-pill clicks change the active season without playback;
artwork and blank hero space remain inert. Existing narrow hero hit handling is unchanged.

## Risks / Trade-offs

- **Long seasons exceed the left pane** -> Render a bounded episode viewport, keep the selected episode
  visible during episode selection, and retain the existing full episode source for playback.
- **Series changes leave stale episode state** -> Reconcile the active Series identity before using
  cached detail, reset episode selection when the right cursor changes, and test rapid cursor changes.
- **Series detail or season episodes are still loading** -> Keep the left pane's existing loading and
  empty states; never display episodes from the previous Series while the new detail is unresolved.
- **Mouse targets overlap the two pane systems** -> Generate targets from the final painted rectangles
  and test episode rows, season pills, Series rows, artwork, and blank space separately.
- **Existing TV keyboard behavior conflicts with pane focus** -> Keep the current `series_selection`
  state as the mode boundary and test both normal Series browsing and active episode selection at
  wide and narrow widths.

## Migration Plan

1. Add focused render and interaction coverage for the persistent Series workspace and two pill bars.
2. Add the wide TV composition and selected-Series source resolution while leaving the narrow path
   unchanged.
3. Route wide TV episode and season interactions through the existing selection and playback actions.
4. Verify wide/narrow rendering, search, scrolling, mouse targets, keyboard navigation, formatting,
   file-size limits, and existing TV tests.

Rollback is a change-level revert of the wide TV dispatch and its interaction/layout bookkeeping;
the existing hero-on-top renderer and TV state remain the fallback.

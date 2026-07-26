## Context

The grouped album renderer currently emits artist headers but constructs selected blocks independently for artist-header focus, collapsed album focus, expanded album focus, and loading album focus. Those paths duplicate block-bound calculations and insert `GroupedAlbumDisplayRow::AlbumArtist` inside selected albums even though an `ArtistHeader` already establishes the group. The renderer also measures a selected block with one constant `(full_width, artwork_width)` pair, which cannot support rows returning to full width below the artwork.

Music-group navigation already traverses a flat sequence of artist headers and albums, and track focus already demonstrates cursor-follow behavior inside an oversized selected block. The change should reuse those concepts while replacing the selected-row producers with one artist-group plan. Plain/search album lists remain per-album selections but lose their duplicated artist row.

## Goals / Non-Goals

**Goals:**

- Make one artist group the stable framed context in music-group view while preserving artist and album action targets.
- Bound album entries to eight terminal rows with deterministic, marker-derived overflow behavior and no new persisted UI state.
- Keep track details, target-sensitive art, wrapping, hit targets, and outer viewport calculations represented by one coherent display plan.
- Remove the redundant album-artist row from every album rendering path.

**Non-Goals:**

- Changing plain/search views to artist-scoped frames.
- Adding a nested scrollbar, sticky artist header, pagination, `+N more` row, or configurable album-region height.
- Persisting inner album-window position across focus changes or application restarts.
- Changing play, enqueue, shuffle, track navigation, or resolved-artist semantics.

## Decisions

### Build one selected artist-group block

The display planner will identify artist boundaries before emitting selected detail rows. In music-group view, the artist containing the current header or album target will produce one framed sequence: top padding, artist target row, pinned hint row, bounded album rows, optional expanded-track rows, artwork filler as needed, and bottom padding. `artist_header_focus` will select the artist row within this path rather than invoking a separate block producer. `selected_block_bounds` will therefore have one producer for music groups.

Alternative considered: retain separate artist and album selected-block branches and synchronize their layouts. This preserves the duplication that caused the current inconsistency and makes nested scrolling and shared block bounds harder to reason about.

### Derive a trailing album window from the marker

The planner will measure each album entry in rendered terminal rows and choose a canonical window of at most eight rows. Before overflow, the window starts at the first album. Once the focused album would cross the lower edge, the start advances by the minimum number of rendered rows needed to keep the complete focused entry visible at the lower edge where possible; a wrapped entry can therefore displace multiple rows on one album-navigation step. The result is recomputed from the current marker each frame; no `LibraryTab` field is added. The hint row receives the first and last represented album ordinals and total count for `first-last/total` feedback.

For an individual album entry taller than the entire region, the renderer will dedicate the region to that entry, retain its marker-bearing first line, render its first eight wrapped lines, and clip the remainder. This avoids introducing a new title-abbreviation policy while keeping the action target identifiable.

Alternative considered: persist an inner offset beside `album_track_focus`. Persistent state supports direction-sensitive window history but adds synchronization and restore/reset cases for a window that is fully determined by the current action target. Page-sized movement and a second scrollbar were rejected because they make row navigation less direct and compete with the outer scrollbar.

### Separate selectable order from visible display rows

Keyboard navigation will continue to use the full flat artist-header-plus-album sequence, including albums outside the current eight-row window. The rendered plan will expose only the selected group's current album window while preserving row targets for visible entries and the marker. Moving across an artist boundary changes the selected group; moving within an artist changes only the marker and derived window.

When track-table focus is active, the artist-block marker remains on the expanded album and its cover remains selected. The track table's existing cursor independently identifies the track action target; no second block marker is introduced.

Alternative considered: remove hidden albums from the navigation sequence. That would make albums unreachable without a separate nested-scroll command and would couple keyboard semantics to viewport capacity.

### Measure layout top-down against a fixed artwork zone

Artwork remains anchored at the selected block top and occupies 12 terminal rows. The planner will replace the block-wide `wrap_widths: Option<(u16, u16)>` assumption with layout inputs that can answer the available width for each absolute row. Artist and hint rows establish the artwork origin; album entries are then measured top-down, using narrowed width only for lines overlapping the art zone and full width afterward. Track rows follow the same overlap rule. Existing filler retains a minimum block extent sufficient to render the complete art box.

Alternative considered: reserve artwork width for the whole selected block. This is simpler but wastes scarce terminal width below the image and contradicts the intended wrap-around layout.

### Keep outer scrolling block-stable with cursor fallback

The outer offset calculation will preserve its prior offset while the action target remains visible inside the same selected artist block. When the selected block exceeds the viewport or track focus moves beyond it, the existing display-cursor approach will clamp the offset just enough to reveal the active target. The artist row will scroll normally rather than being copied into a sticky header.

Alternative considered: always align the display cursor after every marker move. This would make the entire artist frame drift while its internal window is already handling album overflow.

### Delete the duplicated structural artist row globally

`GroupedAlbumDisplayRow::AlbumArtist` and its renderer will be removed from grouped and non-grouped album paths. Music groups use their existing artist header; plain/search per-album frames retain their shape and actions without adding a replacement artist line. Artwork top offsets consequently no longer include selected artist text height.

Alternative considered: remove the row only in music-group view. The same duplicated label is present in plain/search selection frames, so retaining it there would leave the issue partially fixed and preserve unnecessary measurement branches.

## Risks / Trade-offs

- [Variable-width wrapping can make window calculation circular] → Anchor artwork before album layout and measure entries top-down against known absolute row bands.
- [A wrapped album taller than eight rows cannot be shown completely] → Give it the full region, retain the marker-bearing first line, clip after the eighth wrapped line, and cover this boundary with a planner test.
- [Hidden albums could disappear from keyboard navigation] → Keep the full selectable sequence independent of emitted visible rows and test movement through both window and artist boundaries.
- [Outer and inner scrolling can fight each other] → Freeze the outer offset whenever the active target is visible and apply cursor-follow only as a fallback.
- [Removing a display-row variant can shift artwork and mouse hit-testing] → Update offset calculations and assert row targets, block bounds, and art origins in focused tests.

## Migration Plan

This is an in-memory rendering change with no stored-data or configuration migration. Implement the new plan behind the existing `is_music_group_view` branch, remove the obsolete row variant after all producers are gone, and retain the existing non-grouped frame path. Rollback consists of reverting the renderer and planner changes; no user data requires conversion.

## Open Questions

None. The overflow indicator, whole-entry cursor behavior, oversized-title clipping, track-focus marker, outer-offset policy, derived state, and physical-row interpretation were resolved during specification.

## Context

The grouped album renderer currently emits artist headers but constructs selected blocks independently for artist-header focus, collapsed album focus, expanded album focus, and loading album focus. Those paths duplicate block-bound calculations and insert `GroupedAlbumDisplayRow::AlbumArtist` inside selected albums even though an `ArtistHeader` already establishes the group.

Music-group navigation already traverses a flat sequence of artist headers and albums, and track focus already demonstrates cursor-follow behavior inside an oversized selected block. The change should reuse those concepts while replacing the selected-row producers with one artist-group plan. Plain/search album lists remain per-album selections but lose their duplicated artist row.

## Goals / Non-Goals

**Goals:**

- Make one artist group the stable framed context in music-group view while preserving artist and album action targets.
- Render the focused artist's discography inside that block, using a derived 12-album inline window for larger groups.
- Keep track details, target-sensitive art, hit targets, and outer viewport calculations represented by one coherent display plan.
- Remove the redundant album-artist row from every album rendering path.

**Non-Goals:**

- Changing plain/search views to artist-scoped frames.
- Adding a nested scrollbar, sticky artist header, or pagination.
- Persisting inner album-window position across focus changes or application restarts.
- Changing play, enqueue, shuffle, track navigation, or resolved-artist semantics.

## Decisions

### Build one selected artist-group block

The display planner will identify artist boundaries before emitting selected detail rows. In music-group view, the artist containing the current header or album target will produce one framed sequence: top padding, artist target row, pinned hint row, the current 12-album window (or the complete group when it has 12 or fewer albums), optional expanded-track rows, artwork filler as needed, and bottom padding. `artist_header_focus` will select the artist row within this path rather than invoking a separate block producer. `selected_block_bounds` will therefore have one producer for music groups.

Alternative considered: retain separate artist and album selected-block branches and synchronize their layouts. This preserves the duplication that caused the current inconsistency and makes nested scrolling and shared block bounds harder to reason about.

### Deferred: row-aware artwork wrapping

Two decisions originally scoped into this change are deferred to a possible follow-up change, per critic review of the implementation plan:

- **Row-aware top-down artwork wrapping.** This would keep album title rows below the 12-row art band at full width. The current implementation keeps one constant narrowed width for the whole selected block, matching existing behavior (rows below the art band stay narrowed rather than reclaiming full width). This can be revisited independently of the inline window.

### Track focus keeps the album marker

When track-table focus is active, the artist-block marker remains on the expanded album and its cover remains selected. The track table's existing cursor independently identifies the track action target; no second block marker is introduced.

### Keep outer scrolling block-stable with cursor fallback

The outer offset calculation preserves the selected block's anchor while focus moves within it. Larger groups shift a derived 12-album window inside the block; the outer viewport does not scroll through album continuation rows. Track tables retain their existing internal cursor scrolling. The artist row scrolls normally rather than being copied into a sticky header.

Alternative considered: always align the display cursor after every marker move. This would make the entire artist frame drift on every navigation step instead of only when the target actually leaves the viewport.

### Delete the duplicated structural artist row globally

`GroupedAlbumDisplayRow::AlbumArtist` and its renderer will be removed from grouped and non-grouped album paths. Music groups use their existing artist header; plain/search per-album frames retain their shape and actions without adding a replacement artist line. Artwork top offsets consequently no longer include selected artist text height.

Alternative considered: remove the row only in music-group view. The same duplicated label is present in plain/search selection frames, so retaining it there would leave the issue partially fixed and preserve unnecessary measurement branches.

## Risks / Trade-offs

- [A large discography can exceed the viewport] → Keep the outer offset block-stable and shift a derived 12-album window inside the selected artist block (tasks.md Pass 1 tasks 2.4 and 2.6).
- [Outer and inner scrolling can fight each other] → Freeze the outer offset whenever the active target is visible and apply cursor-follow only as a fallback.
- [Removing a display-row variant can shift artwork and mouse hit-testing] → Update offset calculations and assert row targets, block bounds, and art origins in focused tests.

## Migration Plan

This is an in-memory rendering change with no stored-data or configuration migration. Implement the new plan behind the existing `is_music_group_view` branch, remove the obsolete row variant after all producers are gone, and retain the existing non-grouped frame path. Rollback consists of reverting the renderer and planner changes; no user data requires conversion.

## Open Questions

None. Track-focus marker, outer-offset policy, and derived state were resolved during specification.

# Implementation Plan: Artist Discography Selection Block

This is an implementation plan only. Do not change the OpenSpec proposal, design,
specification, or task list while implementing it. Do not add a dependency, a
persistent inner-scroll field, or a new action.

## Implementation Order

The order below is a dependency order. Each numbered step should be complete and
compiling before the next step starts.

1. Add focused failing tests and fixtures.
2. Replace grouped display planning in `src/app/render/album_plan.rs`.
3. Update grouped rendering, row hit maps, artwork, and outer scrolling in
   `src/app/render/album.rs`.
4. Update cursor/navigation consumers in `src/app/render/album_cursor.rs`.
5. Preserve action and mouse semantics while adapting only their row-plan
   consumers.
6. Remove `GroupedAlbumDisplayRow::AlbumArtist` and its dead offset/measurement
   code, then run the full verification checklist.

The planner is the source of truth for absolute display rows. The renderer must
not independently guess the selected block's artist height, album window, or
artwork origin.

## 1. Tests First

Use the existing fixtures rather than introducing a test-only model:

- `src/app/render/tests.rs::make_power_music_group_app` is the renderer fixture.
- `src/app/input_power_music_track_focus_tests.rs::make_power_music_album_app`
  and `make_power_music_album_list_app` cover navigation, tracks, loading, and
  multi-album lists.
- `make_item`, `BrowseLevel`, `LibraryTab`, `render_full_app`, and the existing
  `buffer_to_string` helpers are the established conventions.

Add tests before changing production code. Give the tests names that expose the
behavior, not implementation details:

- `artist_scoped_plan_is_shared_for_header_and_album_focus`: construct two
  artists and several albums, build the plan once with
  `selected_artist_header = Some(...)` and once with an album cursor in the same
  group. Assert one selected block in each plan, exactly one artist row, the
  pinned hint immediately after it, the selected group's visible sibling album
  rows, and identical block top/bottom bounds for the same derived window.
- `grouped_album_window_counts_physical_wrapped_rows`: use one artist with at
  least ten albums and one title long enough to wrap. Assert that the visible
  album region has at most eight physical rows, a wrapped continuation consumes
  one row, the focused album is complete when its height is at most eight, and
  the range metadata is one-based and has the expected `first-last/total`
  values.
- `grouped_album_window_moves_by_whole_entries`: focus an album just beyond the
  first eight-row window, then focus the next album. Assert that the window
  drops complete preceding entries, advances by their physical row counts, and
  never starts in the middle of a wrapped album.
- `oversized_focused_album_is_clipped_to_first_eight_lines`: make the focused
  album title wrap to more than eight lines at the test width. Assert that the
  region contains only that album, exactly eight album-region rows, and the
  first row carries the album marker; assert no later title line is emitted.
- `hidden_grouped_albums_remain_keyboard_selectable`: use
  `make_power_music_album_list_app(20, 0)`, move down repeatedly with the normal
  key path, and assert that raw album cursors outside the currently emitted
  eight-row window are reached and that crossing the last album changes to the
  next artist header rather than stopping at the visible window.
- `grouped_header_and_album_targets_keep_action_resolution`: focus a header,
  assert header play/enqueue/shuffle resolution still uses all albums in that
  artist group; focus an album, assert `selected_album_item` and the existing
  folder actions still use the raw cursor item. Do not change action assertions.
- `grouped_track_focus_keeps_album_marker_and_cover`: load tracks for the
  selected album, enter track focus, render, and assert the album row remains
  the marked/bold target, the track table has its own focused-track marker, and
  the inline image remains the selected album cover rather than switching to a
  collage.
- `grouped_artwork_wrap_uses_full_width_after_twelve_rows`: render a long
  album/title list with images enabled and assert a title line below the
  twelve-row art band uses the full content width while an overlapping line is
  narrowed. Also assert the art origin is the block's first content row and the
  filler keeps the complete art box inside the block.
- `grouped_outer_scroll_preserves_offset_until_target_leaves_view`: render the
  same artist at a short height, move among visible targets, and assert the
  stored outer offset does not change. Then move to a target or focused track
  outside the viewport and assert the offset follows just far enough to reveal
  it, without rendering a duplicate/sticky header.
- `mouse_targets_follow_shifted_album_window`: render an overflowing group,
  locate a visible album through `left_row_targets`, click its rendered row,
  and assert the raw album cursor becomes that album. Assert hidden rows have no
  click target.
- `plain_and_search_album_frames_have_no_album_artist_row`: exercise the
  non-music/plain and search paths through the existing per-album renderer.
  Assert the frame remains per album, album actions remain unchanged, and the
  resolved artist label is not emitted as a separate structural row.

Update existing display-row-count assertions in
`src/app/input_power_music_track_focus_tests.rs` (especially the PageUp/PageDown
tests around `page_down_in_album_list_mode_pages_by_rendered_rows_with_inline_detail`,
`page_up_in_album_list_mode_pages_by_rendered_rows_with_inline_detail`, and
`paging_from_non_selectable_hint_and_header_rows_chooses_nearest_album_by_direction`).
Their comments currently count `AlbumArtist`; those comments and expected rows
must describe the new artist-scoped block, not preserve obsolete counts.

Run the focused tests while they are being added with:

```text
cargo test -p mbv --bin mbv artist_scoped_plan
cargo test -p mbv --bin mbv grouped_album_window
cargo test -p mbv --bin mbv hidden_grouped_albums
cargo test -p mbv --bin mbv grouped_track_focus
cargo test -p mbv --bin mbv grouped_artwork
cargo test -p mbv --bin mbv grouped_outer_scroll
cargo test -p mbv --bin mbv mouse_targets_follow_shifted
cargo test -p mbv --bin mbv plain_and_search_album_frames
```

## 2. Rebuild the Grouped Display Plan

Edit `src/app/render/album_plan.rs`, primarily
`GroupedAlbumDisplayRow`, `GroupedAlbumDisplayPlan`, and
`App::build_grouped_album_display_plan`.

### 2.1 Discover artist groups before emitting rows

Keep the current metadata semantics:

- Resolve each album artist with `resolve_group_album_artist`.
- Keep the current year/name derivation using `production_year`,
  `parse_album_folder_name`, and `display_name()`.
- Keep `order` sorted by `natural_sort_key(strip_article(&artist))`.
- Keep raw album indices in `order`; `BrowseLevel::cursor` is still a raw item
  index and must never be interpreted as a display-order index.

While iterating `order`, build contiguous group ranges. For every range record
its resolved artist, first raw album index/id, ordered raw indices, and total
album count. The header identity remains the existing
`ArtistHeaderSelection { first_album_id, artist_label }`.

Use this group discovery for all of the following:

1. Find the group containing raw `cursor` when an album is focused.
2. Find the group matching `selected_artist_header` when the header is focused.
3. Validate a stale header selection.
4. Produce all members for `artist_header_album_items_for_selection`, including
   albums hidden by the eight-row display window.
5. Produce the full selectable header-plus-album navigation sequence.

Do not derive group membership by scanning only emitted `rows`. That is the
current behavior in `App::artist_header_album_items_for_selection` in
`album_cursor.rs`, and it would silently omit hidden albums after this change.

### 2.2 Keep selectable order separate from emitted rows

Extend the plan with the minimum metadata needed by consumers, such as the full
selectable target sequence and visible one-based album range. Keep this data
derived per frame; do not add a field to `LibraryTab` or `BrowseLevel`.

The full selectable sequence for a music-group plan is:

```text
for each discovered artist group in display order:
    emit ArtistHeader(selection) when headers are selectable
    emit Album(raw_index) for every album in that group
```

The emitted display rows contain all ordinary groups, but for the selected group
they contain only the selected artist block and its current visible album
window. Hidden albums remain in the selectable sequence and in `order`, but have
no `left_row_map` or mouse target because they are not on screen.

`display_cursor` must remain the absolute row index of the current target's
marker-bearing row for normal header/album focus. For track focus, retain the
album marker row as the target cursor and also expose or calculate the absolute
focused-track row for outer cursor-follow; do not move the album marker to a
track row.

### 2.3 Emit exactly one selected artist block

For the selected artist group, regardless of whether the target is a header, a
collapsed album, a loading album, or an expanded album, emit this order:

```text
outer top border rule
colored top padding
ArtistHeader(selection)                         # one artist row
ArtistActionHint or AlbumActionHint             # pinned second row
visible Album(raw_index) + continuation rows    # at most 8 physical rows
optional AlbumDetailStart(raw_index) + detail continuation rows
colored bottom padding
outer bottom border rule
```

The hint text is target-sensitive:

- Header target: `^P: Play | ^A: Enqueue | ^S: Shuffle`.
- Album target: `^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks`.

When the artist has hidden albums, append the derived range to that same pinned
hint row as ` • first-last/total`, for example
`^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks • 3-8/20`.
Do not emit the range when all albums are visible.

For a header target, use the first album in that group as the canonical window
anchor. The raw album cursor remains unchanged, and no album is marked selected
while `artist_header_focus` is set.

For a collapsed, loading, or expanded album target, the marker is on the raw
cursor album. If the track table is focused, the marker and cover still belong
to that album and the track table's existing cursor is independent.

Non-selected groups retain ordinary `ArtistHeader` and `Album` rows. Outside the
music-group view, retain the existing selected per-album frame, but omit its
duplicated artist row.

### 2.4 Derive the eight physical-row album window

Measure each album entry in physical terminal rows using the exact content width
that the renderer will use for each absolute row. A title's first line is an
`Album(raw_index)` row; each additional title line is an
`AlbumWrappedContinuation(raw_index)` row or equivalent source-indexed
continuation. The marker gutter is reserved on every line so continuation text
does not shift horizontally. The year suffix uses the existing ` • YEAR` rule
and must be included in the measurement.

Use this stateless algorithm, where `height[i]` is the complete un-clipped
physical height of group entry `i`:

```text
REGION_ROWS = 8

if height[focused] > REGION_ROWS:
    start = focused
    end = focused + 1
    emit only the focused entry's first REGION_ROWS lines
else if sum(height[0..group_len]) <= REGION_ROWS:
    start = 0
    end = group_len
else:
    start = 0
    while start <= focused and sum(height[start..=focused]) > REGION_ROWS:
        start += 1                         # drop a whole preceding entry
    end = focused + 1
    while end < group_len and sum(height[start..=end]) <= REGION_ROWS:
        end += 1                           # add only complete trailing entries
```

The first branch is the oversized-title rule: dedicate the region to that
album, preserve its marker-bearing first line, emit the first eight wrapped
lines, and clip all later lines. The normal branch advances by complete entry
heights, so a wrapped entry displaces multiple physical rows in one navigation
step. Never emit a partial neighboring album merely to fill the budget.

Store visible range ordinals as `start + 1` and `end` for the selected artist;
the total is the group length. Store `None` when `start == 0 && end == len`.
The range is display-group order, not raw item index order.

### 2.5 Measure physical rows top-down with artwork

Replace the current `wrap_widths: Option<(u16, u16)>` constant pair with layout
inputs sufficient to answer width by absolute row. Do not reserve art width for
the entire block.

Use these absolute-row invariants:

- `block_top` is the colored top-padding row; `block_bottom` is the colored
  bottom-padding row. Preserve the existing `selected_block_bounds` tuple
  semantics as `(top_padding_abs, bottom_padding_abs)`.
- The artist content row is `top_padding_abs + 1`.
- If images are enabled and the terminal is wide enough for
  `INLINE_ALBUM_ART_RESERVED`, artwork starts at the artist content row and
  occupies exactly `INLINE_ALBUM_ART_ROWS` rows. Otherwise there is no art band.
- `content_width(abs_row) = full_width - INLINE_ALBUM_ART_RESERVED` only when
  `art_top <= abs_row < art_top + INLINE_ALBUM_ART_ROWS`; otherwise it is
  `full_width`.
- A target row always reserves two columns: active `▌` plus one space, or two
  spaces when inactive. Album title text starts at the same column regardless
  of focus.

Use a top-down wrapping loop so width changes when a title crosses the art
boundary:

```text
wrap_entry(start_abs_row, title, year, full_width, art_band):
    remaining = title
    line = 0
    while remaining is not empty:
        row = start_abs_row + line
        width = content_width(row, art_band)
        title_width = max(1, width - TARGET_GUTTER - year_suffix_width)
        first_wrapped_piece = wrap(remaining, title_width).first
        emit first_wrapped_piece for this physical row
        consume that piece and its following whitespace
        line += 1
    append the year suffix to the final emitted line using the existing style
```

Apply the same row-width rule to artist text, the pinned hint, loading text,
and track-detail rows. The planner and renderer must use the same start row and
band; otherwise a measured eight-row window will not match what is painted.
If a detail table extends below the art band, its later rows use full width.

Ensure filler rows make `block_bottom` at least `art_top +
INLINE_ALBUM_ART_ROWS` before bottom padding when images are enabled. This is
the continuation-space rule that prevents short discographies from cropping
the art box.

## 3. Render the New Plan

Edit `src/app/render/album.rs::App::render_power_grouped_album_rows`.

1. Keep album metadata derivation and `layout.inline_image_rect = None` at the
   start, but consume the planner's group metadata, visible range, block bounds,
   artwork band, and absolute cursor rows rather than reconstructing them.
2. Keep the current `selected` header lookup and `expand_selected` state rule:
   grouped views expand only after `album_track_focus` is entered; non-grouped
   album-folder behavior remains as it is.
3. Paint the selected background from the one shared
   `selected_block_bounds`. A header target must now have the same block bounds
   shape as an album target.
4. Render `ArtistHeader` with the fixed two-column gutter. Apply the AQUA marker
   and bold white title only when it matches `artist_header_focus` and the
   library pane is focused. Keep the resolved artist color and existing action
   semantics.
5. Render the pinned hint immediately below the artist row, append the range
   metadata only when present, and wrap it using the row-aware width.
6. Render each visible album's first line with the fixed two-column gutter and
   render its continuation rows at the planner's precomputed line positions.
   The active album gets `selection_marker(true)` and bold white text only when
   it is the current album target and no header target is active. Do not render
   an artist label before the album title.
7. Keep `AlbumDetailStart`, `AlbumDetailContinuation`, and `AlbumLoading` below
   the bounded album region. Preserve `render_power_album_detail`'s track cursor
   and `selected_region_gutter` behavior. When `album_track_focus.is_some()`, do
   not overwrite `layout.cursor_screen_y` with the album marker after rendering
   the track table; the detail renderer's cursor remains authoritative for the
   active track.
8. Reserve artwork per absolute row, not with one selected-block-wide width.
   Render the artist collage when the header is the active target and the
   selected album cover otherwise. Track focus is the latter case, so it keeps
   the album cover. The collage's album inputs must come from the full selected
   group in display order, not only the visible eight-row slice.
9. Keep `layout.left_sorted_indices = plan.order.clone()`.
   `left_row_map` and `left_row_targets` must be produced only for visible
   `Album` and selectable `ArtistHeader` rows. Continuations, borders, hints,
   loading rows, and detail rows map to `None`. A shifted window therefore maps
   a click to the correct raw index without inventing hidden hit targets.
10. Keep the right scrollbar for the actual emitted display-row sequence. Do
    not add a second scrollbar for the nested album region.

### Block bounds and outer offset algorithm

Replace the current artist-line-specific art math and make the outer offset
calculation target-aware:

```text
block_start = colored_top_padding_abs - 1       # include outer border
block_end   = colored_bottom_padding_abs + 1     # include outer border
active_row  = focused_track_abs_row if track focus else plan.display_cursor

if stored_scroll <= active_row < stored_scroll + visible_rows:
    offset = stored_scroll                       # preserve stable block position
else if active_row < stored_scroll:
    offset = active_row                           # reveal target above
else:
    offset = active_row + 1 - visible_rows        # reveal target below

offset = offset.clamp(global_min_offset, global_max_offset)
```

Use block bounds to compute the same minimum/maximum reveal limits as the
existing selected-block cursor-follow behavior when the block cannot fit. The
important invariant is that a target-visible move does not change the outer
offset, while an off-screen album or track moves it only enough to reveal the
active row. The artist header scrolls normally; never render a copied sticky
header. Persist only this outer offset through the existing
`BrowseLevel::scroll` path.

## 4. Navigation and Mouse Consumers

### `src/app/render/album_cursor.rs`

Update these exact methods:

- `move_power_music_group_display_cursor`: navigate through the plan's full
  header-plus-all-albums target sequence, not `plan.rows` (which intentionally
  omits hidden album entries). Resolve the new target exactly as today: header
  sets `artist_header_focus` and clears track focus; album clears header focus,
  updates the raw cursor, and clears track focus only when the album changes.
- `jump_power_music_group_display_cursor`: use the first/last item of the same
  full target sequence. Do not jump to the first/last visible row.
- `artist_header_album_items_for_selection`: use discovered artist group
  membership and display order directly, not emitted rows. This preserves
  header bulk actions for hidden albums.
- `page_power_grouped_album_cursor`: preserve its existing page intent, but do
  not make the eight-row inner window the navigation boundary. If a page target
  is not emitted, resolve to the nearest full-sequence album in the direction
  of travel. Keep raw cursor semantics and the existing fetch/page behavior.

### `src/app/actions.rs`, `src/app/action.rs`, and related action code

Do not change action semantics. Verify, but do not redesign:

- `App::selected_album_item` continues to read `nav_stack.last().items[cursor]`.
- `App::current_lib_item` continues to resolve a focused track only while
  `album_track_focus` is set, otherwise the album folder.
- `App::activate_album_folder_row` still treats a focused artist header as a
  consumed no-op, enters track mode only for an album, and plays a focused track
  when already in track mode.
- Header play/enqueue/shuffle continues through
  `play_selected_artist_header` and its all-member group lookup.

Only update callers if a changed plan field or target representation requires a
mechanical adaptation. Do not add commands, alter key bindings, or change play,
enqueue, shuffle, track navigation, or resolved-artist behavior.

### `src/app/input_mouse.rs` and `src/app/layout.rs`

Keep the existing typed `LibraryRowTarget` variants and click branches. The
renderer must make `left_row_targets` accurate for the shifted visible window;
the mouse handler should continue to:

- set header focus and clear track focus on a header click;
- set the raw album cursor and clear header/track focus when a different album
  is clicked;
- avoid opening track mode from a mouse click on an album;
- use the existing double-click and activation paths unchanged.

Do not add a nested viewport coordinate, a second scroll field, or a new mouse
action. `LayoutMain::left_area`, `left_row_map`, `left_row_targets`,
`cursor_screen_y`, and `inline_image_rect` remain the geometry seam.

## 5. Remove `AlbumArtist` and Simplify Art Math

Remove `GroupedAlbumDisplayRow::AlbumArtist` only after all new producers and
tests are in place.

Current producers, renderers, and consumers to remove or update are:

- `src/app/render/album_plan.rs::GroupedAlbumDisplayRow`: delete the enum
  variant.
- `src/app/render/album_plan.rs::App::build_grouped_album_display_plan`: delete
  the three producers currently at the collapsed, loaded-track, and loading
  selected-album branches (current lines 258, 292, and 322). Delete the
  `album_artist_labels` collection, `selected_artist_lines` closure, and any
  measurement that exists only to emit that row.
- `src/app/render/album.rs::App::render_power_grouped_album_rows`: delete the
  `AlbumArtist(idx)` match arm (current lines 195-222), including its wrapped
  artist paragraph. Update the `left_row_map` match (current lines 456-467) so
  only the new continuation variant and other structural rows map to `None`.
- `src/app/render/album_cursor.rs`: update the group-member scan that currently
  walks `plan.rows`; it must use discovered group ranges, not replace the
  deleted row with another emitted-row scan.
- All wildcard matches over `GroupedAlbumDisplayRow` must be rechecked after
  the variant deletion; use compiler errors and `rg` to ensure no usage remains.

The current artist-line-dependent artwork calculations to remove/simplify are:

- `album.rs` `selected_art_abs_rows`'s `title_offset` branch that adds
  `album_artist_label` wrapping for album focus (current lines 101-109). The
  new art origin is the artist content row for every selected artist block.
- `album.rs` `Album` rendering's `has_block` calculation that recomputes
  artist-line height (current lines 237-245). Block membership and first-line
  position come from the plan.
- `album_plan.rs` `selected_title_lines`, `selected_hint_lines`, and
  `selected_detail_rows` use of one constant `(full_width, artwork_width)`
  pair. Replace them with the row-aware top-down measurement described above.
- `album.rs` any `area.width - selected_art_reserved_w` applied to rows below
  the art band. Apply the reservation only when the current absolute row
  overlaps the art band.
- `album_plan.rs::App::album_artist_label` and its remaining calls, if no
  non-structural consumer remains after the rewrite. Keep
  `resolve_group_album_artist`; it is still required for group discovery.

Run this search after the cleanup and require zero Rust matches for the removed
variant:

```text
rg 'GroupedAlbumDisplayRow::AlbumArtist|AlbumArtist\(' src
```

Do not remove `AlbumArtistFetched`, `album_artist_cache`, or
`resolve_group_album_artist`; those support resolved-artist grouping and must
remain unless compilation and the existing behavior prove a particular helper
is genuinely unused.

## Do Not Overengineer

- No new dependencies.
- No persistent inner-scroll field beside `album_track_focus`.
- No nested scrollbar.
- No sticky artist header.
- No pagination, `+N more`, or configurable album-region height.
- No broad render/layout refactor.
- No changes to action semantics, key bindings, or mouse activation semantics.
- No speculative generic layout framework or new abstraction hierarchy. Keep
  the derived group/window/row logic local to the existing planner and reuse
  `LayoutMain`, `LibraryRowTarget`, `selection_marker`, and the existing album
  detail/art helpers.
- Do not make hidden albums disappear from keyboard navigation merely because
  they are absent from the emitted rows.
- Do not use a constant narrowed width for rows below the twelve-row art zone.

## Risks Requiring Confirmation

1. `artist_header_album_items_for_selection` currently derives members by
   scanning `plan.rows`. The new spec requires hidden albums to remain
   selectable, while header bulk actions currently use the same member lookup.
   This plan assumes the intended behavior is that header actions cover all
   albums in the resolved artist group, not only the eight visible rows. If that
   assumption is not intended, the spec and existing header-action behavior
   conflict and must be clarified before implementation.
2. The source currently uses one `Album` row plus placeholder continuation rows
   while the renderer paints a multi-line paragraph. The new row-aware width
   rule requires continuation rows to identify their source album or equivalent
   planner-owned line data. This plan assumes changing the continuation row's
   internal representation is acceptable because it is private to
   `render/album_plan.rs`; the public row targets and action semantics do not
   change.

## Verification Checklist

Run these after implementation, from the repository root:

1. Focused planner/render/navigation/action tests:

   ```text
   cargo test -p mbv --bin mbv artist_scoped_plan
   cargo test -p mbv --bin mbv grouped_album_window
   cargo test -p mbv --bin mbv hidden_grouped_albums
   cargo test -p mbv --bin mbv grouped_track_focus
   cargo test -p mbv --bin mbv grouped_artwork
   cargo test -p mbv --bin mbv grouped_outer_scroll
   cargo test -p mbv --bin mbv mouse_targets_follow_shifted
   cargo test -p mbv --bin mbv plain_and_search_album_frames
   cargo test -p mbv --bin mbv selectable_artist_header
   cargo test -p mbv --bin mbv current_lib_item
   ```

2. Formatting:

   ```text
   cargo fmt --all -- --check
   ```

3. Clippy, with existing warnings treated as errors:

   ```text
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```

4. Relevant workspace tests after the focused tests pass:

   ```text
   cargo test --workspace
   ```

5. OpenSpec validation:

   ```text
   openspec validate artist-discography-selection-block --strict --no-interactive
   ```

6. Final source checks:

- `rg 'GroupedAlbumDisplayRow::AlbumArtist|AlbumArtist\(' src` returns no
  removed display-row usage.
- Header focus, album focus, and track focus each have exactly one intended
  target marker.
- Track focus retains the expanded album marker and album cover.
- Hidden albums remain reachable by keyboard and retain raw-index action
  resolution.
- Plain/search frames remain per-album and have no duplicated artist row.
- No `LibraryTab` or `BrowseLevel` field was added for inner album scrolling.
- No sticky header or second scrollbar appears in the render path.

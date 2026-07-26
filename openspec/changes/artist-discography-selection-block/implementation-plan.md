# Implementation Plan: Artist Discography Selection Block

`tasks.md` is the authoritative task list (2 passes: remove the duplicated
artist row, then build the artist-scoped frame with a derived 12-album window
for large groups). This document is background/rationale only. Row-aware
top-down artwork wrapping remains deferred; see design.md's "Deferred" note.
Do not add a dependency, a persistent inner-scroll field, or a new action.

## Implementation order

1. Pass 0: remove `GroupedAlbumDisplayRow::AlbumArtist` and simplify the art
   offsets that depended on it (tasks.md group 1).
2. Pass 1: replace grouped display planning in `src/app/render/album_plan.rs`
   and grouped rendering in `src/app/render/album.rs` with the unified
   artist-scoped block (tasks.md group 2).

Do not touch `src/app/input_mouse.rs` or `src/app/layout.rs` — see tasks.md
task 2.6. Grouped cursor/page planning must keep albums outside the visible
12-album window reachable, while header actions continue to resolve the full
artist group.

The planner is the source of truth for absolute display rows. The renderer
must not independently guess the selected block's artist height or artwork
origin.

## Pass 0 background

Current producers, renderer, and consumers to remove:

- `src/app/render/album_plan.rs::GroupedAlbumDisplayRow`: delete the
  `AlbumArtist(usize)` variant.
- `src/app/render/album_plan.rs::App::build_grouped_album_display_plan`:
  delete the three producer sites (currently `album_plan.rs:258`, `:292`,
  `:322`) — each is a `rows.push(AlbumArtist(idx))` paired with a
  `rows.extend(...AlbumWrappedContinuation...)` call; delete both statements
  at each site (see tasks.md 1.1 for why the continuation half matters).
  Delete the now-uncalled `album_artist_labels` collection and
  `selected_artist_lines`.
- `src/app/render/album.rs::App::render_power_grouped_album_rows`: delete the
  `AlbumArtist(idx)` match arm (current lines 195-222) and its `left_row_map`
  arm (current line 459).
- `album.rs` `selected_art_abs_rows`'s `title_offset` branch that adds
  `album_artist_label` wrapping for album focus (current lines 101-109). The
  art origin becomes the artist content row unconditionally.
- `album.rs` `Album` rendering's `has_block` calculation that recomputes
  artist-line height (current lines 237-245). Block membership and
  first-line position come from the plan.
- `album_plan.rs::App::album_artist_label` and its remaining calls, once
  nothing else calls it.

Keep `resolve_group_album_artist`, `album_artist_cache`, and
`AlbumArtistFetched` — grouping still depends on them.

Verify with `rg 'GroupedAlbumDisplayRow::AlbumArtist|AlbumArtist\(' src`
(zero matches expected).

## Pass 1 background

Edit `src/app/render/album_plan.rs`, primarily `GroupedAlbumDisplayRow`,
`GroupedAlbumDisplayPlan`, and `App::build_grouped_album_display_plan`.

### Discover artist groups before emitting rows

Keep the current metadata semantics:

- Resolve each album artist with `resolve_group_album_artist`.
- Keep the current year/name derivation using `production_year`,
  `parse_album_folder_name`, and `display_name()`.
- Keep `order` sorted by `natural_sort_key(strip_article(&artist))` — this
  sorts groups, and is stable within a group, so album order within a group
  is unaffected (see tasks.md 2.1).
- Keep raw album indices in `order`; `BrowseLevel::cursor` is still a raw
  item index and must never be interpreted as a display-order index.

While iterating `order`, build contiguous group ranges. For every range
record its resolved artist, first raw album index/id, ordered raw indices,
and total album count. The header identity remains the existing
`ArtistHeaderSelection { first_album_id, artist_label }`.

### Emit exactly one selected artist block

For the selected artist group, regardless of whether the target is a header,
a collapsed album, a loading album, or an expanded album, emit this order:

```text
outer top border rule
colored top padding
ArtistHeader(selection)                         # one artist row
ArtistActionHint or AlbumActionHint             # pinned second row
the current 12-album window (or every album for groups of 12 or fewer)
  + continuation rows
optional AlbumDetailStart(raw_index) + detail continuation rows
colored bottom padding
outer bottom border rule
```

The hint text is target-sensitive:

- Header target: `^P: Play | ^A: Enqueue | ^S: Shuffle`.
- Album target: `^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks`.

For a header target, the raw album cursor remains unchanged, and no album is
marked selected while `artist_header_focus` is set.

For a collapsed, loading, or expanded album target, the marker is on the raw
cursor album. If the track table is focused, the marker and cover still
belong to that album and the track table's existing cursor is independent.

Non-selected groups retain ordinary `ArtistHeader` and `Album` rows. Outside
the music-group view, retain the existing selected per-album frame, but omit
its duplicated artist row.

## Render the new plan

Edit `src/app/render/album.rs::App::render_power_grouped_album_rows`.

1. Keep album metadata derivation and `layout.inline_image_rect = None` at
   the start, but consume the planner's group metadata, block bounds,
   artwork band, and absolute cursor rows rather than reconstructing them.
2. Keep the current `selected` header lookup and `expand_selected` state
   rule: grouped views expand only after `album_track_focus` is entered;
   non-grouped album-folder behavior remains as it is.
3. Paint the selected background from the one shared `selected_block_bounds`.
   A header target must now have the same block bounds shape as an album
   target.
4. Render `ArtistHeader` with the fixed two-column gutter. Apply the AQUA
    marker and bold blue title only when it matches `artist_header_focus`
   and the library pane is focused. Keep the resolved artist color and
   existing action semantics.
5. Render the pinned hint immediately below the artist row.
6. Render each album's first line with the fixed two-column gutter and
   render its continuation rows at the planner's precomputed line positions.
   The active album gets `selection_marker(true)` and bold white text only
   when it is the current album target and no header target is active. Do
   not render an artist label before the album title.
7. Keep `AlbumDetailStart`, `AlbumDetailContinuation`, and `AlbumLoading`
   below the album region. Preserve `render_power_album_detail`'s track
   cursor and `selected_region_gutter` behavior. When
   `album_track_focus.is_some()`, do not overwrite `layout.cursor_screen_y`
   with the album marker after rendering the track table; the detail
   renderer's cursor remains authoritative for the active track.
8. Render the artist collage when the header is the active target and the
   selected album cover otherwise (track focus keeps the album cover). The
   collage's album inputs must come from the full selected group in display
   order. Use one constant narrowed width for the whole block — do not
   introduce row-aware per-row width measurement (deferred, see design.md).
9. Keep `layout.left_sorted_indices = plan.order.clone()`. `left_row_map` and
    `left_row_targets` must be produced for every emitted `Album` in the
    current window and selectable `ArtistHeader` row. Hidden albums remain
    reachable through grouped cursor/page planning.
   Continuations, borders, hints, loading rows, and detail rows map to
   `None`.
10. Keep the right scrollbar for the actual emitted display-row sequence.

### Outer-offset policy (required — see tasks.md task 2.4)

Keep the outer offset block-stable using the same lower/upper bounds as the
other selected-block renderers. Large groups shift the derived inline window;
the outer viewport must not scroll through album continuation rows. Track
tables retain their existing internal cursor scrolling.

## Do not overengineer

- No new dependencies.
- No persistent inner-scroll field beside `album_track_focus`.
- No nested scrollbar.
- No sticky artist header.
- No pagination or `+N more` row; the derived window has no persisted position.
- No broad render/layout refactor.
- No changes to action semantics, key bindings, or mouse activation
  semantics.
- No speculative generic layout framework or new abstraction hierarchy. Keep
  the derived group/row logic local to the existing planner and reuse
  `LayoutMain`, `LibraryRowTarget`, `selection_marker`, and the existing
  album detail/art helpers.

## Verification checklist

Run from the repository root, after each pass:

```text
cargo test -p mbv --bin mbv
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
openspec validate artist-discography-selection-block --strict --no-interactive
rg 'GroupedAlbumDisplayRow::AlbumArtist|AlbumArtist\(' src   # expect no matches, after Pass 0
```

Final source checks:

- Header focus, album focus, and track focus each have exactly one intended
  target marker.
- Track focus retains the expanded album marker and album cover.
- Plain/search frames remain per-album and have no duplicated artist row.
- No `LibraryTab` or `BrowseLevel` field was added.
- No sticky header or second scrollbar appears in the render path.
- A discography larger than 12 albums shifts its inline window while the
  outer artist block remains anchored.

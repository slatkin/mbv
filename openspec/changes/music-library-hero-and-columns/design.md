## Context

Grouped Music currently renders through the shared library-list path with a selected-album hero above grouped album rows. Below the shared 82-column threshold those rows are one column; at and above it they pack into two columns. Music-group pills are carved from the full content width before library rendering. The album track table appears only when `album_track_focus` is active.

The Home tab already provides the intended wide visual precedent: a roughly 40/60 horizontal split, a two-column gap, pills over the right list, focused `BG_GREEN` and unfocused `PLAYBACK_PANEL_BG` surfaces, and one logical list item per terminal row. See `proposal.md` for motivation and the delta specs for behavior.

The separate `remove-selectable-music-artist-headers` change removes artist-header focus and actions. This design assumes that change lands first, so grouped Music has only album browsing and track selection as internal focus states.

## Goals / Non-Goals

**Goals:**

- Preserve the current grouped Music renderer and interaction below the existing breakpoint.
- Give wide grouped Music a stable left album/track workspace and right album browser.
- Keep responsive composition as render-time state derived from the content width.
- Reuse existing album selection, track cache, track cursor, grouping snapshot, and Home palette behavior.

**Non-Goals:**

- Generalizing the split into a shared layout for other library types.
- Adding a persisted pane-focus or split-size setting.
- Adding artist summaries or replacing removed artist bulk actions.
- Changing Music API requests, grouping configuration, or queue construction.

## Decisions

### 1. Branch grouped Music at the existing shared breakpoint

Grouped Music uses the same content-width threshold as Home wide mode and library column packing. Below it, rendering and input continue through today's hero-above-list path unchanged. At or above it, a Music-specific coordinator renders the horizontal layout.

The branch is based on the padded library content area, not terminal width, so queue width and panel mode are already accounted for. No new responsive state is stored; resizing derives the composition each frame.

Rejected: stacking the new wide panes at narrow widths. The user explicitly chose to retain the current narrow composition rather than introduce another fallback.

### 2. Use Home's 40/60 split and put pills in the right rail

Wide Music divides the full content area into a left pane of approximately two-fifths, a two-column gap, and a right pane using the remainder, with the same minimum-side clamps as Home where practical.

The wide coordinator owns music-group pill placement: pills occupy the right rail's first row and the album list begins below the same spacing and rule treatment used by Home's wide list. The existing full-width pill carve remains the narrow path only.

This requires deciding wide versus narrow before the global Music pill carve. Keeping pills full-width was rejected because it weakens the right rail's identity as the complete album browser and shortens the left hero.

### 3. Divide the left pane vertically between a large hero and persistent tracks

The left pane is one visual workspace with two vertical regions:

- A Home-style album hero above, using yellow title treatment, metadata, and centered large artwork.
- A track region below, using the existing wrapped track-table content and duration rules.

The initial allocation gives the hero roughly three-fifths of available height and tracks roughly two-fifths. Track sizing is content-aware: a short album may use fewer track rows and return space to artwork, while a long album scrolls within the track region. When height is constrained, reserve the track label plus at least one visible track row when tracks exist, then shrink or omit artwork before removing the track region. Loading and empty states use the same reserved track region so the layout does not jump when data arrives.

The existing album-detail renderer may be decomposed or parameterized, but wide layout must not derive pane height from total track count; that would recreate the current hero growth problem.

### 4. Separate track visibility from track focus

`album_track_focus: Option<usize>` remains the internal focus discriminator:

- `None`: right album browser is active; left tracks render as a preview without an active cursor.
- `Some(i)`: left track region is active and keeps track `i` visible.

Wide rendering requests the selected album's cached tracks regardless of this option. A cache miss shows Loading and starts the existing album-id-keyed fetch. Selection changes must key every title, art, loading, and track lookup to the new album immediately so stale tracks cannot appear under the new hero.

Preview mode starts at the top of the track list. Focused mode uses render-local table scrolling to keep the selected index visible. No second persisted track-scroll field is introduced unless implementation proves the existing table state cannot preserve cursor visibility.

### 5. Keep the right album browser strictly one column

The wide right rail renders the settled grouped display with one album per physical album row and full-width artist labels. The grouped two-column packing and left/right album-cell navigation introduced by the earlier version of this change have no remaining responsive use: narrow is below the threshold and wide deliberately uses one column.

Column-specific grouped code should be deleted when no other caller requires it, while retaining shared grouping plans, wrapped album labels, album cursor identity, paging, and scroll clamping. Up/Down remain album movement; Left/Right do not become cross-pane focus controls. Enter and Escape are the explicit keyboard focus transitions.

### 6. Derive reciprocal pane styling from existing focus state

No new focus enum is needed. Styling derives from outer `PanelFocus` plus `album_track_focus`:

| Outer focus | Track focus | Left workspace | Right browser |
| --- | --- | --- | --- |
| Library | None | `PLAYBACK_PANEL_BG` preview | `BG_GREEN` focused |
| Library | Some | `BG_GREEN` focused | `PLAYBACK_PANEL_BG` context |
| Queue | either | normal dimmed library treatment | normal dimmed library treatment |

The focused right row follows Home's aqua cursor bar and contrasting selected-row treatment. During track focus the selected album marker remains visible but its text and surface dim with the rail. The focused track uses the existing yellow active-track treatment.

### 7. Preserve keyboard semantics and geometry during focus changes

Enter on an album sets `album_track_focus` to the existing initial index. Up/Down, Enter on a track, current-item scope, and Escape/Backspace keep their current behavior. Entering or leaving track selection changes only styling and active cursor; the split and vertical allocations do not move.

Resizing across the breakpoint preserves selected album identity and `album_track_focus`. A focused track therefore remains focused when switching into narrow mode, where today's hero shows tracks; switching to wide mode exposes the same focused track in the persistent region. With no track focus, narrow mode continues to collapse tracks while wide mode previews them.

### 8. Add track-row hitmaps only for wide mode

Wide rendering records one logical hit target per visible track, covering all wrapped physical rows belonging to it. A single click sets `album_track_focus` to that track; a double-click first sets the same focus and then invokes existing focused-track playback.

Clicking an album or group pill clears track focus through existing selection paths and returns visual focus right. Artwork and blank hero space may focus the outer Library panel but do not enter track selection or invoke playback. The generic whole-hero double-click activation behavior does not apply to the wide Music left pane because its track rows provide the explicit interactive targets.

### 9. Keep responsive grouping continuity selection-based

Both compositions consume the same settled grouped snapshot and album cursor. Width changes rebuild display geometry and clamp scroll around the selected album; they do not restart grouping or create separate narrow/wide snapshots. Render-derived row maps and hitmaps are replaced every frame so stale geometry cannot cross the breakpoint.

## Risks / Trade-offs

- **[Artwork dominates short terminals]** Large Home-style artwork could starve tracks. -> Reserve the track region first and let artwork shrink or disappear before tracks.
- **[Track fetch churn]** Moving quickly through albums can request tracks for several selections. -> Reuse the album-id cache/loading set and issue at most one request per album; do not add speculative prefetch in this change.
- **[Stale mouse geometry]** Wrapped tracks and responsive resizing can invalidate hit rows. -> Rebuild and clear wide track hitmaps every frame and key them by logical track index.
- **[Breakpoint discontinuity]** Pills and tracks move when crossing 82 columns. -> Preserve album/track identity and scroll visibility while treating the composition switch as intentional, matching Home.
- **[Partially implemented obsolete columns]** Existing grouped two-column code may obscure the one-column invariant. -> Delete unused column-specific branches rather than retaining compatibility for a layout no longer specified.
- **[Visual verification burden]** Color and art/track proportions are difficult to validate with stable text assertions. -> Prefer geometry/interaction tests and perform real-terminal checks at narrow, threshold, wide, and short-height sizes.

## Migration Plan

No persisted data or protocol migration is required. Apply `remove-selectable-music-artist-headers` first, then implement this responsive layout against the simplified album/track focus model. Rollback restores the current hero-above-list and grouped column behavior without data changes.

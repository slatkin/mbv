## Context

See `proposal.md` for the motivation and user-visible scope. The repository already has the
Hero-onLeft geometry and right-rail chrome in `render/hero_left.rs`, and Home's wide branch already
builds and paints the selected Emby media card required here.

The current Movies path is different: `render_list` measures a `CompactBannerLayout`, reserves a
Hero-onTop block, paints the portrait-poster movie detail, and then renders the list. Home's wide
Emby path instead uses a 16:9 image above metadata, a watch-state glyph, release date, duration, and
overview. That Home path is the source of truth for this change.

The worktree may contain unrelated in-progress rendering edits. Implementation must preserve those
changes and avoid treating the current checkout as a clean baseline.

## Goals / Non-Goals

**Goals:**

- Make wide Movies a consumer of the existing Hero-onLeft arrangement.
- Make the Home wide selected-Emby hero card one shared rendering/model path used by Home and Movies.
- Keep the right-hand Movies list as the only keyboard-focusable content pane.
- Keep letter pills in the right rail and force the wide Movies list to one column.
- Preserve the existing narrow Movies Hero-onTop fallback and its current movie-detail behavior.
- Keep image fetching, cache keys, metadata order, watch-state rendering, and overview treatment
  identical between Home's card and the wide Movies card.

**Non-Goals:**

- Redesigning Hero-onLeft geometry, colors, borders, spacing, or pill presentation.
- Moving TV shows, podcasts, feeds, home videos, Music, or Audiobookshelf screens.
- Finishing the broader centralized mouse hit-target migration.
- Adding a new media-card abstraction or new domain terminology.
- Changing playback, Service APIs, persistence, or library data fetching semantics.

## Decisions

### 1. Reuse Home's selected-Emby card as the source implementation

The existing Home wide Emby branch is the canonical card. Extract its content preparation and paint
entry points from the Home-specific assembly only as needed so both Home and Movies call the same
implementation. The shared path retains:

- `keep_watching_hero_layout` for title and overview preparation;
- the 16:9 artwork sizing and centered image treatment;
- the `id:pwr_kw` image cache key;
- Movie artwork preference `Backdrop`, `Primary`, `Logo`;
- the watch-state glyph, release-date row, duration row, and overview rendering;
- the same focus boolean semantics used by Home (Library focus brightens the card; Queue focus
  dims it), without making the hero a focus target.

The Movie library supplies its selected `EmbyItem` from the same cursor source the right-hand list
uses. Active inline search must use its result cursor for the card too; the hero must not read the
stale navigation-level cursor when search is active.

**Alternative rejected:** Copying Home's wide branch into a Movies renderer. That would produce two
cards that can drift in image keys, row budgeting, or metadata styling, directly violating the
exact-card requirement.

**Alternative rejected:** Reusing the current `CompactBannerLayout` for wide Movies. That is the
portrait-poster Hero-onTop card and is intentionally a different arrangement/content shape.

### 2. Add Movies as a read-only Hero-onLeft composition

The wide Movies path will compose the existing Hero-onLeft primitives in the same structural order
as Home:

```text
HeroOnLeft
├── left pane: shared Home selected-Emby hero card, read-only
└── right pane
    ├── existing letter-pill bar when eligible
    └── existing Movies list renderer, forced to one column
```

The arrangement owns pane geometry, right-rail pill placement, list-panel surface/border, and the
list width. The Movies screen supplies only the selected Emby hero data, letter-pill data, and movie
rows. The wide list area becomes `LayoutMain.left_area` so existing cursor movement, scrolling,
page sizing, and keyboard activation continue to target the right rail.

The wide read-only hero is not published as the interactive `hero_area` used by Hero-onTop screens.
That preserves the existing Home-wide interaction model: the right list is the browse surface, and
the left card is only a projection.

**Alternative rejected:** Treating the left hero as a second Hero-onLeft focusable pane. Movies has
no track/chapter interaction, so adding pane focus would create state and key behavior with no user
purpose.

### 3. Keep arrangement selection centralized enough for this adoption

Movies will not add a new visual geometry implementation or a second width-specific layout. The
shared Hero-onLeft arrangement decision and height floor remain the source of responsive behavior;
the Movies renderer only supplies its data to that arrangement. Below the shared breakpoint, control
returns to the existing Hero-onTop list path.

The width/height thresholds and Hero-onLeft pane math must stay in the existing shared helpers. No
Movies-local pane ratio, pill-row geometry, or focus-color branch is permitted.

### 4. Preserve the existing right-rail list model

Movies keeps its current list data and pill eligibility rules. At wide Hero-onLeft widths:

- letter pills render through the existing shared pill bar in the right-rail pill slot;
- an active inline search replaces that pill slot with the existing search control;
- letter-grouped or plain movie rows use the existing list renderers with one column;
- list cursor movement and `Enter` continue through the existing Emby library input path;
- the selected movie is resolved from the same active list/search source for both rows and hero.

No new row type, cursor state, or Movie-specific focus state is introduced.

### 5. Verify exactness through model/render checks and temporary captures

The project does not add committed UI snapshots. Verification will combine focused existing-style
render tests with the repository's throwaway capture approach:

- wide Movies and Home Movies Latest use the same selected item and expose the same hero text/card
  content and image cache key;
- moving the Movies list cursor updates the left card even when the selected row is scrolled away;
- wide Movies has a right-rail pill row and one-column list, while narrow Movies keeps Hero-onTop;
- queue focus dims both surfaces without creating hero focus;
- existing narrow Movies activation remains unchanged;
- temporary wide/narrow captures are visually compared and removed rather than committed.

## Risks / Trade-offs

- **Home-specific state leaks into the shared card path** -> Extract only the selected Emby hero data
  and painter; keep Home section/cursor/pill state in Home and pass the selected item as input.
- **Movie library items lack a field used by Home's card** -> Use the existing library fetch fields and
  preserve graceful empty-field behavior; add a targeted parsing/model check before changing API
  requests.
- **The old `hero_area` input rectangle accidentally activates the wide card** -> Leave the wide
  read-only hero out of interactive hero geometry and verify keyboard activation remains list-owned.
- **`render_list` or related files exceed the 800-line cap** -> Put the wide Movies composition in a
  focused render module and keep the dispatch in the existing list entry point minimal.
- **Uncommitted concurrent hero edits conflict with extraction** -> Re-read and integrate the current
  files at implementation time; do not revert or overwrite unrelated worktree changes.

## Migration Plan

1. Add focused render/model tests or temporary capture coverage for the current Home card and the
   intended wide Movies composition.
2. Extract the shared selected-Emby Home card path without changing Home's output.
3. Add the Movies Hero-onLeft dispatch and right-rail list/pill composition; keep the narrow fallback
   on the existing path.
4. Verify targeted tests, formatting, file-size limits, and temporary wide/narrow captures.

Rollback is a single change-level revert: remove the Movies Hero-onLeft dispatch and shared adoption,
returning Movies to its existing Hero-onTop wide path. No persisted data or protocol migration is
required.

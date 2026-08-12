## Context

The prior Audiobookshelf browse change introduced a separate browse state, but its renderer currently flattens shelf, show, and episode rows into a paragraph. The TV Shows tab already has the required screen: a full-width selected-Series hero pinned above a one- or two-column Series list, with the Series image, selector pills, and episode table inside the hero. The redesign must preserve that exact composition without converting Audiobookshelf data into `EmbyItem`.

## Goals / Non-Goals

**Goals:**

- Make the Audiobookshelf podcast tab structurally identical to TV Shows outside explicit data mappings.
- Keep podcast-native identities and read-only progress state.
- Make `All`, `Played`, and `Unplayed` a selected-show episode filter, not a filter of the show catalog.
- Keep episode activation inert and remove shelves from the supported visible model.

**Non-Goals:**

- Audiobookshelf playback, queue admission, stream resolution, progress writes, or live updates.
- A generic provider-neutral media model.
- Cross-Service Home aggregation or audiobook browsing.

## Decisions

### 1. Keep a distinct Audiobookshelf browse state

Extend the existing concrete Audiobookshelf state with an episode-filter value and an episode-selection cursor. Do not route Audiobookshelf rows through the Emby `LibraryTab`, `BrowseLevel`, or TV `SeriesDetail` types. This preserves Service-specific identity and prevents Emby-only actions from appearing valid.

### 2. Share TV geometry while rendering Service-native content

Extract the TV top-hero geometry and shell as shared rendering primitives. The Audiobookshelf renderer uses those exact primitives and the same list column helpers, selected-cell spans, Series image dimensions, selector pills, and episode-table geometry. It supplies its own labels and identities from Audiobookshelf catalog types.

The composition is always vertical: full-width selected-podcast hero above the podcast show list. Audiobookshelf must never select Music's wide horizontal hero layout.

### 3. Place the Audiobookshelf cover in the Series image slot

Fetch only the selected podcast cover through the existing Service-scoped Audiobookshelf image cache. Render it in the same right-aligned rectangle and with the same placeholder and images-disabled row budgeting as the TV Series Primary image.

### 4. Derive filtered episodes from read-only progress

`All` exposes every downloaded episode. `Played` matches completed progress, and `Unplayed` matches episodes without completed progress. Missing progress is unplayed; current-time-only progress remains visible in `All` and `Unplayed` until completion is reported. Filtering never mutates the Service or local playback state.

### 5. Mirror TV selection mode while preserving inert activation

Show activation enters episode-selection mode. Up/down move through episode rows, brackets change the mapped filter, and Escape returns to show selection. Episode activation consumes the input without queue or playback work.

## Risks / Trade-offs

- **[Risk] TV layout behavior drifts independently** -> Share the geometry and visual primitives rather than copying constants or formulas.
- **[Risk] Filter changes invalidate episode selection** -> Reset the episode cursor to the first visible row on every filter change.
- **[Risk] Small terminals cannot show the full detail block and table** -> Use the shared TV hero clamp and suppression behavior so the lower show list always retains a usable row.
- **[Risk] ABS image fetches regress to list thumbnails** -> Request only the selected podcast's cover while rendering the hero.

## Migration Plan

1. Replace the flat mixed renderer with a provider-native top hero and show-only lower list.
2. Add the three-state episode filter and TV-style selection mode.
3. Remove shelf loading, visible rows, and navigation.
4. Verify top-hero geometry, selected cover behavior, filter transitions, identity restoration, and inert activation.

Rollback is limited to restoring the prior Audiobookshelf browse renderer and row model; no persisted data or protocol migration is required.

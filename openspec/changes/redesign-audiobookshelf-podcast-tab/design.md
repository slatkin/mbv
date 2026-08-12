## Context

The prior Audiobookshelf browse change introduced a separate browse state, but its renderer currently flattens shelf, show, and episode rows into a paragraph. The TV library already has the desired interaction seam: a primary series list, a selected-series detail block, selector pills, and a structured episode table. The redesign should reuse those presentation conventions without converting Audiobookshelf data into `EmbyItem`.

## Goals / Non-Goals

**Goals:**

- Give Audiobookshelf podcast libraries a show-first layout that visually and behaviorally parallels TV libraries.
- Keep podcast-native identities and read-only progress state.
- Make `All`, `Played`, and `Unplayed` a selected-show episode filter, not a filter of the show catalog.
- Keep episode activation inert and remove shelves from the supported visible model.

**Non-Goals:**

- Audiobookshelf playback, queue admission, stream resolution, progress writes, or live updates.
- A generic provider-neutral media model.
- Cross-provider Home aggregation or audiobook browsing.

## Decisions

### 1. Keep a distinct Audiobookshelf browse state

Extend the existing concrete Audiobookshelf state with an episode-filter value and filtered episode view or equivalent derived selection. Do not route Audiobookshelf rows through the Emby `LibraryTab`, `BrowseLevel`, or TV `SeriesDetail` types. This preserves provider-specific identity and avoids making Emby-only actions appear valid.

Alternative rejected: convert shows and episodes to `EmbyItem`. That would simplify renderer reuse at the cost of incorrect activation, progress, and identity semantics.

### 2. Model the screen as show catalog plus selected-show detail

The primary rows contain podcast shows only. The selected show's detail area owns its cover, title/author metadata, filter pills, and episode table. Episode rows are rendered inside that detail area and are not interleaved with the show catalog. Personalized shelf entries are excluded from both row generation and navigation.

Alternative rejected: retain a flat list and add indentation or colors. That would not establish the TV-style hierarchy or provide a stable place for the episode filter.

### 3. Reuse shared visual primitives, not provider data types

Use the existing selected-detail block, pill bar, focused row styles, table layout, duration formatting, and image pipeline conventions. The Audiobookshelf renderer supplies its own labels, row identities, and metadata from native catalog types. The episode filter should use the same three-state semantics already used by the Feeds watched filter, with the TV season-pill presentation and keyboard behavior where appropriate.

### 4. Derive filtered episodes from read-only progress

`All` exposes every downloaded episode. `Played` matches completed progress, and `Unplayed` matches episodes without completed progress. Missing progress is unplayed; current-time-only progress remains visible in `All` and `Unplayed` until completion is reported. Filtering never mutates the server or local playback state.

### 5. Preserve the existing activation boundary

Show activation enters or maintains the selected-show detail interaction. Episode activation consumes the key without queue or playback work. This keeps the redesign compatible with the later playback milestone and prevents the current no-op from being mistaken for an accidental quit path.

## Risks / Trade-offs

- **[Risk] The existing Audiobookshelf state and renderer are coupled to shelf rows** -> Make show-only row generation the single source for layout, cursor, and hit testing; remove shelf branches from the visible path.
- **[Risk] Episode filtering changes the visible cursor list while detail requests are pending** -> Keep the selected show identity independent from the filtered episode cursor and clamp/reset the cursor whenever the filter or detail result changes.
- **[Risk] TV rendering assumptions depend on Emby fields** -> Reuse only generic rendering primitives and keep provider-specific metadata extraction in Audiobookshelf code.
- **[Risk] Small terminals cannot show the full detail block and table** -> Follow the existing TV detail row budgeting and preserve a usable show list plus scoped loading/empty states.

## Migration Plan

1. Replace the Audiobookshelf flat-row presentation with show-only catalog rows and a selected-show detail renderer.
2. Add the three-state episode filter and filtered cursor behavior.
3. Remove shelf rows and shelf navigation from the supported UI path.
4. Verify focused/unfocused rendering, filter transitions, selection restoration, empty/loading states, and inert activation.

Rollback is limited to restoring the prior Audiobookshelf browse renderer and row model; no persisted data or protocol migration is required.

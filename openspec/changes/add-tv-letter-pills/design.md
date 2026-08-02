## Context

The application already has a shared letter-range model, pill renderer, server-side range query, keyboard/mouse dispatch, and persisted library-position state for large non-music libraries. The change needs to make that path explicitly correct for TV libraries, whose browse results must represent `Series` rather than episodes or seasons. See `proposal.md` and the TV letter-filtering spec for the user-visible contract.

## Goals / Non-Goals

**Goals:**

- Make the existing alphabet-pill experience available and reliable on large TV library roots.
- Ensure TV range queries operate on series/show names and preserve `SortName` semantics, including leading-article handling.
- Keep the existing movie behavior and shared interaction model intact.
- Cover eligibility, query scope, rendering, selection, cycling, and restoration with focused tests.

**Non-Goals:**

- Changing the pill labels, threshold, or range boundaries for movies.
- Adding alphabet filtering to episodes, seasons, nested folders, searches, home feeds, or music libraries.
- Replacing the existing server-side range query with client-side filtering.

## Decisions

1. **Reuse the shared pill infrastructure.** Update the existing non-music eligibility and TV root loading paths instead of creating a TV-specific pill widget or duplicate filter state. This keeps mouse hitboxes, keyboard cycling, persistence, default selection, and loading behavior consistent with movies.

2. **Scope the TV root to series.** When initializing or refreshing the top-level TV browse level, carry a `Series` item-type scope so the captured total and range fetch represent shows. Nested navigation keeps its existing item-type behavior for seasons and episodes.

3. **Keep range boundaries based on effective `SortName`.** Continue using the existing `NameStartsWithOrGreater` and `NameLessThan` API bounds, because the client and server already align on Emby's `SortName` ordering. The visible list remains sorted and grouped by the same effective key.

4. **Use the true unfiltered TV total for eligibility.** Capture the library total before applying the default `A-C` range, and use that total to decide whether pills are shown. The filtered range count must not change eligibility or the range grouping style.

5. **Test behavior at the action and rendering boundaries.** Add TV fixtures for large-library eligibility and series-scoped selection, plus terminal-render assertions for pill labels and show grouping. Preserve existing movie tests as regression coverage.

## Risks / Trade-offs

- **[Existing TV libraries may expose folders before series]** → Apply the `Series` scope only to the top-level show browse path and verify the current TV navigation shape in fixtures; do not alter nested season/episode navigation.
- **[Emby range bounds depend on server sort semantics]** → Reuse the already validated `SortName` range API and test leading-article titles through the existing effective-sort path.
- **[Large libraries trigger an additional scoped refresh]** → Keep the existing default `A-C` behavior and paginated range fetch so the initial screen does not load the entire library.

## Migration Plan

No data migration is required. Existing saved movie positions remain unchanged. Saved TV positions without a range continue to load normally; eligible large TV roots may then receive the default `A-C` range and persist that state using the existing library-position format.

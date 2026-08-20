## Context

See `proposal.md` for the motivation. The current shared arrangement model treats narrow content as
hero-on-top and builds a row map for ordinary list rows; the earlier library implementation instead
inserted the selected item's detail into the one-column flow. The centralization work also requires
arrangements, rather than individual screens, to own geometry and hit targets.

The change must preserve the existing wide hero-on-top and hero-on-left layouts, all hero content
declarations, and the shared breakpoint. The narrow layout is the only presentation being replaced.

## Goals / Non-Goals

**Goals:**

- Make one shared narrow library arrangement render an active-row inline hero for every library screen.
- Keep variable hero height inside list flow so content below the active item does not jump when a
  different item has different metadata.
- Keep keyboard navigation, scrolling, mouse hit testing, loading states, and selection markers
  coherent when the hero occupies multiple rows.
- Keep wide geometry and non-library screens unchanged.

**Non-Goals:**

- Redesigning hero content, artwork selection, metadata, or typography.
- Changing the shared breakpoint, wide arrangement assignments, or Panel mode semantics.
- Adding a user preference between inline and pinned narrow heroes.
- Changing Home, global search, overlays, settings, or playback surfaces.

## Decisions

### Use a shared narrow inline composition

The responsive arrangement resolver will continue to choose the wide arrangement from the centralized
breakpoint, but library screens below that breakpoint will be composed by one shared inline-hero
path. The path will receive the same selected-item hero declaration used by wide arrangements and will
not branch on library kind for placement.

**Alternative rejected:** Keep hero-on-top and add per-library flags. That would preserve the unstable
height behavior and recreate the screen-specific branching the centralization change removed.

### Treat the hero as a variable-height row block

The narrow list layout will account for the active item's ordinary row followed by its hero block as
one flow segment. The row map will identify the ordinary row and hero-owned rows separately, so cursor
movement addresses media items only while hit testing can distinguish item rows from inert hero space.
The active segment's height is calculated from the existing hero content constraints; no fixed-height
truncation is introduced.

**Alternative rejected:** Overlay the hero on top of list rows. Overlaying avoids row-map changes but
causes hidden content, ambiguous mouse targets, and unstable readability at short terminal heights.

### Keep activation on the media row

The media row remains the selection and activation target. The inline hero is a projection of the
active row, not a second item. Pointer events in the hero block do not select a different item; the
existing row activation behavior remains the source of truth.

**Alternative rejected:** Make the hero a duplicate activation target. That would create inconsistent
single/double-click behavior and duplicate hit-target representations.

### Test geometry and interaction contracts, not snapshots

Verification will cover narrow rendering dimensions, cursor movement, variable hero heights, scrolling,
loading/empty states, and mouse target resolution with focused unit or render tests that match existing
project testing conventions. No new committed UI snapshot assertions are planned.

## Risks / Trade-offs

- [Risk] Variable inline content can reduce the number of visible media rows at short heights. ->
  Mitigation: suppress the hero only when the minimum active row plus minimum hero content cannot fit,
  and retain the ordinary list area in that case.
- [Risk] Existing scroll calculations may assume every row has one media item. -> Mitigation: make the
  shared row map and visible-row accounting treat the inline block as flow rows while keeping cursor
  indices media-item based.
- [Risk] Hero-only rows may receive accidental mouse activation. -> Mitigation: emit common hit targets
  only for the active media row and ordinary item rows; hero space is inert.
- [Risk] Wide layouts could regress while sharing hero content code. -> Mitigation: limit changes to
  narrow arrangement dispatch and add focused wide-layout regression checks.

## Migration Plan

1. Update the shared narrow arrangement and row bookkeeping without changing wide paths.
2. Reuse the existing hero content declarations and migrate each library screen to the shared path.
3. Verify narrow and wide rendering, keyboard navigation, scrolling, loading/empty states, and mouse
   behavior.
4. Roll back by restoring the prior narrow arrangement dispatch if a regression is found; no persisted
   data migration or compatibility step is required.

---
status: accepted
---

# Destination-Embedded Inline Search

Inline Search is a library-scoped capability of the selected searchable Emby
destination, not an independently mounted Interactive Component. The destination
owns the local search control, session, query, candidate selection, loading view,
result cursor and scroll, painting, and event interpretation. Browser owns it for
Normal catalogs and non-TV Emby destinations, MusicWorkspace owns it for grouped
Music, and TvWorkspace owns it in Wide TV; Browser owns it in Normal TV.

## Decision

Each searchable destination embeds the shared `InlineSearch` control and invokes
its arrangement and painter at the destination's own library-list composition
point. The destination supplies the list area for the current presentation:
Normal uses the ordinary list area, while Hero-on-left uses the right library
rail. While search is active, the destination's ordinary list painter does not
paint that same area. Exactly one mounted destination component owns and paints
the
search input and result list for the current breakpoint.

The shell remains responsible for full-library fetches, recursive album-index
construction, loading and stale-completion guards, navigation-stack mutation,
and activation effects. It pushes validated candidate pools and loading state at
lifecycle boundaries; it does not own a second search session or choose an
internal search rectangle. Search activation crosses the boundary as a typed
request.

TV has two destination owners across the responsive breakpoint. A Normal/Wide
change transfers one snapshot (open state, query, stable selected target, and
viewport row offset) from the outgoing owner to the incoming owner and consumes
it once. A normal tab change dismisses search rather than transferring it. Thus
there is still exactly one owner and one painter at every breakpoint.

## Rationale

Embedding search at the destination's composition point keeps placement,
keyboard and mouse interpretation, and painted result geometry under one
authority. It removes the former overlay's paint-derived rectangle and avoids a
shell mirror or two-painter protocol while retaining shared search behavior
across destinations.

## Consequences

`InlineSearch` is a reusable plain control, not mounted, focused, subscribed, or
assigned a `ComponentId`. The destination's existing keyboard router boundary
and mouse eligibility remain authoritative. Inline Search remains distinct from
the cross-library Search sidebar.

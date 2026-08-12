## Context

See `proposal.md` for motivation and `specs/stable-music-library-grouping/spec.md` for the behavior contract.

Music album browse levels arrive in `BrowseLevel.items` in server `SortName` order, generally in pages of 100. The grouped renderer currently clones those items, derives artist/title metadata, sorts the full list, and creates display rows on every terminal draw. `resolve_group_album_artist` also queues per-album artist lookups while rendering. Each `AlbumArtistFetched` event changes the cache that the next draw uses, so a slow stream of results progressively changes group membership and order.

The current display plan mixes two concerns: a source-dependent catalog (artist identity, album display metadata, group boundaries, and sort order) and presentation-dependent rows (selected group window, wrapped text, artwork, track detail, and hit targets). Only the latter needs to react to ordinary redraws.

## Goals / Non-Goals

**Goals:**
- Publish a music album grouping once per settled source snapshot rather than incrementally.
- Remove artist lookup scheduling and full catalog sorting from the steady-state render path.
- Preserve selection, scroll context, and artist-header action membership across an atomic replacement.
- Prefer artist metadata supplied with the browse response, retaining a bounded fallback for servers or folder layouts that do not provide it.

**Non-Goals:**
- Change `[music].levels`, the current artist-sort semantics, or album/track navigation.
- Apply this lifecycle to non-music letter grouping, queue grouping, search, or feed views.
- Add a persistent on-disk artist cache, new user configuration, or animated list transitions.
- Redesign the application's general HTTP concurrency model.

## Decisions

### 1. Model grouped albums as versioned settled snapshots

Each music album browse level will own grouping state with a monotonically advancing source revision. A revision represents the current parent and ordered set of loaded album IDs. It progresses through:

```text
source items change
        |
        v
  resolving candidate -------------------> obsolete
        |                                      ^
        | all identities terminal              | newer source revision
        v                                      |
  settled catalog -----------------------------+
```

The candidate records which albums still need a terminal artist identity. If no settled catalog exists, the renderer shows a dedicated organizing state. If a settled catalog already exists, it remains visible while a newer candidate resolves. A candidate may only replace the visible catalog when its revision still matches the current browse level.

This explicitly separates arrival of individual metadata results from publication of a user-visible ordering. It also lets in-flight requests populate the existing global artist cache without letting stale results mutate the current view.

**Alternatives considered:**
- Apply every result immediately and anchor the cursor: reduces some movement but retains repeated group/header changes and does not satisfy the selected one-commit experience.
- Wait for every request indefinitely: improves metadata completeness but can leave a slow or failed server permanently blocking the library.

### 2. Resolve artist identities before publication, with a bounded fallback

The normal music browse request will request the album artist fields needed to populate the existing media model, eliminating most per-album lookup work. For an album without a usable artist identity, resolution is scheduled from browse-state transitions, never from rendering. Existing bounded artist lookup concurrency remains the fallback path.

The candidate has a short, bounded settling window. A successful lookup, an empty/error result, or expiry makes an album terminal. Expired or unavailable values use the existing deterministic folder-name parse and then `Unknown Artist` fallback. Late results remain cached for a future source revision but do not rewrite the visible catalog.

This favors a prompt, stable first view over waiting for perfect metadata. It also makes the fallback behavior deterministic for a given snapshot.

**Alternatives considered:**
- Rely solely on folder-name parsing: fast and stable, but loses correct grouping for common folder naming variations.
- Keep every fallback request but show partial groups: preserves early content but is the source of the reported jank.

### 3. Cache a source-derived grouped catalog separately from dynamic display rows

Build and retain a grouped catalog when a candidate settles. It contains the resolved artist identity, display title/year, precomputed natural sort keys, sorted album order, and group boundaries. The renderer reads this catalog rather than deriving artist data or sorting raw albums.

Presentation rows remain derived from the catalog because selection, width, artwork availability, and inline track details legitimately affect row height and visible targets. Cache that derived plan for identical presentation inputs where practical, but keep its construction pure: painting must not schedule requests or alter grouping state.

The catalog build is intentionally performed once during a browse/event transition rather than introducing another worker protocol. Browse pages are capped at `PAGE_SIZE` and the principal regression is repeated frame work, not a single catalog build. If measurement after this change shows that one-time catalog preparation blocks input materially, it can be moved behind the existing library event channel without changing the snapshot contract.

**Alternatives considered:**
- Cache the current full display plan only: its dependencies on selection, wrapping, and inline detail make invalidation broad and do not remove duplicate artist derivation cleanly.
- Create a new grouping worker now: would reduce one-time main-thread work but adds cross-thread state and cancellation complexity before evidence that it is needed.

### 4. Anchor atomic replacements by media identity, not raw row offset

Before committing a replacement, capture the selected album ID (or selected artist header's first-album ID) and the nearest visible album anchor. Resolve those identifiers through the new catalog, then derive the new cursor, header selection, and scroll offset so the selected album remains in view at the closest practical screen position. If the selected item no longer exists, use the existing valid-selection fallback.

Artist-header actions and navigation will consume the settled catalog, ensuring their group membership matches the headers and albums on screen rather than a newly recomputed cache state.

**Alternatives considered:**
- Preserve only the numeric `cursor` and `scroll` values: raw item indices and display-row offsets have different meanings after a grouping replacement, causing the visible jump this change is meant to remove.

## Risks / Trade-offs

- [Some Emby servers do not return usable album artist fields for folder-backed albums] -> Retain the existing bounded per-album lookup and deterministic fallback chain.
- [The organizing state can briefly delay the first list] -> Use browse-response metadata first and a bounded settle window; publish fallback identities rather than waiting indefinitely.
- [A newly paged album is temporarily absent from the visible settled catalog] -> Keep the prior catalog usable until the replacement is ready, then atomically anchor the replacement around the current selection.
- [Late results from an old group race with a new group] -> Gate commits by source revision and parent identity; stale results may populate the global cache only.
- [Cached catalogs use additional memory] -> Keep only the active settled catalog and at most one candidate per music browse level; discard superseded candidates.
- [A one-time catalog build may still be noticeable for unusually large responses] -> Measure after removing repeated render work; the source-page cap keeps the initial implementation simple and leaves a worker extraction path open.

## Migration Plan

No persisted-data migration is required. Ship the new state as an internal extension of music browse levels. A rollback can remove the snapshot lifecycle and fall back to the existing raw-item rendering path; cached artist data remains compatible because it retains the current album-ID-to-artist shape.

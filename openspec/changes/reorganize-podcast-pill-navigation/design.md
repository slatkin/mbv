## Context

The podcast tab today is TV-parallel: `PodcastContent` projects a show list (`MediaListCarrier`
over show rows), alphabetical surname buckets as the Selector row, and a selected-show Hero with a
Workspace (`All`/`Played`/`Unplayed` pills over a per-show episode list). Episodes come from a
per-show detail fetch (`podcast_detail` → `/api/items/{id}?expanded=1`) into a detail cache.
Feeds already implements the target shape one column over: one Selector row of watched-filter pills followed by group pills, `FeedDisplayRow` heading rows from `feeds_model.rs`,
read-only hero, direct play activation. `AudiobookshelfDownloadedEpisode` carries
`library_item_id`, `episode_id`, `title`, `published_at` (string), `duration_seconds`; played
state comes from the library progress map keyed `(library_item_id, episode_id)`.

## Goals / Non-Goals

**Goals:**

- The podcast tab renders one flat, grouped episode list scoped by a mutually exclusive
  state-and-show pill selector.
- Podcast grouping reuses `feeds_model.rs` verbatim (five age groups, same day boundaries).
- One bounded expanded-items wire path replaces per-show detail fan-out for browsing.
- Podcast activation lands on the existing play/enqueue boundary with Feeds' semantics.

**Non-Goals:**

- No changes to podcast playback, queueing, source-resolution, or progress-refresh capabilities.
- No Audiobookshelf Books tab changes (surname ranges stay).
- No new daemon persistence; the remembered pill is session memory only.
- No restructure of `library_panel` slot semantics, `pill-selector-presentation`, or the
  `MediaList` heading machinery — all reused as-is.

## Decisions

### D1 — Mirror the Feeds owner shape instead of adapting the TV-parallel one

`PodcastContent` shrinks to the `feeds_content.rs` shape: one episode `MediaListCarrier` with
heading/spacer rows, one combined Selector row, hero without Workspace, leaf activation. The
alternative (keep the show carrier and filter its projected rows) preserves dead machinery —
show hero, season-position filter logic, selection modal — that every other requirement here
deletes. Feeds' `FeedDisplayRow` pattern (headings resolved from the entry slice, indices
unchanged) is copied for `PodcastDisplayRow`; the carrier keeps stable
`(library_item_id, episode_id)` targets so heading insertion cannot shift targeting.

### D2 — One wire path: paged `/api/libraries/{id}/items?expanded=1`

mbv-core gains a paged expanded-items fetch whose wire types extend the existing `ItemsResponse`
shape: `ShowWire.media` gains `episodes` (the existing `EpisodeWire` plus description), so one
response family feeds both the (still-needed) show pill list and the flat episode rows.
Alternative considered — fan-out `podcast_detail` per show on tab open: N+1 requests, no
progressive heading fill, burst server load; rejected. `AudiobookshelfDownloadedEpisode` gains a
description field (hero needs it); `published_at` stays a parsed string as today. Playback-related
episode fields (`audio_file`, image) are only added to the wire if the playback path still needs
them from this endpoint — the existing per-item playback resolution is unchanged.

### D3 — Pills are one selector, resolved in the owner

`build_show_title_buckets` leaves this tab (Books keep `build_surname_buckets`). The selector is
a single `SelectorRow`: `All`/`Unplayed`/`Played` labels first, then per-show titles truncated
with the same `trunc_str` limit Feeds uses for group labels. `SelectorPicked(index)` branches on
`index < 3` exactly as Feeds branches on `WatchedFilter::COUNT`. Filtering composes at projection
time: visible episodes = (all shows | selected show) × (state filter), grouped by
`feed_age_group(pub_date_secs)`; the five headings are omitted when empty (matching Feeds, which
only emits non-empty groups).

### D4 — Session-remembered pill lives in the owner, not persistence

The last active pill index is a field on `PodcastContent`, surviving `set_content` snapshots and
tab switches. Restart resets to `All`. Alternative — persisting in config/state: touches
persistence schemas for near-zero value; rejected (spec says session memory).

### D5 — Activation reuses the podcast episode intents, re-aimed at list rows

Enter/overlay-activation maps to the existing `PodcastEpisodeIntent::OpenOrPlay(target)`;
Ctrl+A to `Enqueue`; context menu unchanged. In non-Wide geometry the episode row is a leaf:
first Enter opens the Library Hero overlay (existing `hero_overlay_available` seam, Feeds
precedent), the overlay's activation plays. In Wide geometry the read-only hero sits beside the
list and browser Enter plays directly. `PodcastEpisodeTarget` (show id + episode id) is already
the right opaque identity; no new message family.

### D6 — The hero projects episode facts over the existing Square shell

`hero_content_abs_show` is replaced by a small episode-hero builder: title = episode title,
credits = show name + author, overview = episode description, plus resume/finished fact. Artwork
keeps the existing show-cover fetch/cache path keyed by the parent show id — no new image
identity family, and the shell's hero image projection continues unmodified.

## Risks / Trade-offs

- [First paint of `All` waits on the first expanded-items page; large libraries fill in over
  several pages] → progressive page landing with headings in place; the empty/loading state is
  scoped to the list slot, Selector row and hero stay live.
- [`expanded=1` pages are heavier than minified show pages (episodes inline)] → bounded page
  size as today; append-once discipline; no cap is accepted deliberately (spec), revisit only if
  a real library hurts.
- [Deleting the detail cache changes which state survives provider refresh] → the
  re-anchor-on-refresh discipline moves from show identity to pill+selection identity; the
  existing stable-selection refresh tests are rewritten around episode identity.
- [Two specs (`audiobookshelf-podcast-browsing`, `audiobookshelf-podcast-library-ui`) carried
  overlapping TV-parallel requirements] → this change collapses them into the Feeds-parallel
  composition in both deltas; sync on archive resolves the drift.
- [`[`/`]` keys on this tab previously cycled episode filters in the hero; with the hero
  workspace gone, their disposition changes] → the pill selector's keyboard handling is defined
  once in the change's routing matrix rows (shared `shell_routing_matrix_tests.rs`), not by
  lingering compatibility arms.

## Migration Plan

Single-client terminal app, no data migration. Ship as one change: wire path, owner rewrite,
render/spec/test updates. Rollback is `git revert`; no persisted state changes.

## Open Questions

- Whether show pills render for shows with zero downloaded episodes (a pill that opens an empty
  list) or only shows with at least one episode. Spec leaves this to the empty-state scenario;
  decide at implementation with a default of "render the pill, show the scoped empty state".

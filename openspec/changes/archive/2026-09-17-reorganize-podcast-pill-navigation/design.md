## Context

The podcast tab today is TV-parallel: `PodcastContent` projects a show list (`MediaListCarrier` over show
rows), alphabetical surname buckets as the Selector row, and a selected-show Hero with a Workspace
(`All`/`Played`/`Unplayed` pills over a per-show episode list). Episodes come from a per-show detail fetch
(`podcast_detail` → `/api/items/{id}?expanded=1`) into a detail cache.

The requested organisation is different in kind: the pill bar itself becomes the browsing axis — state
pills over a flat episode list, then one pill per subscribed podcast. That is the Feeds tab's shape one
column over: one Selector row, `FeedDisplayRow` heading rows from `feeds_model.rs`, a read-only hero, and
direct play activation.

Audiobookshelf's API constrains how those lists can be filled, and the constraint is not optional:

- `GET /api/libraries/{id}/items` returns **shows only**. `LibraryController.getLibraryItems` reads
  `limit`, `page`, `sort`, `desc`, `filter`, `minified`, `collapseseries`, `include` — never `expanded` —
  and every row is serialized through `LibraryItem.getByFilterAndSort` → `li.toOldJSONMinified()`, whose
  podcast media (`Podcast.toOldJSONMinified()`) carries `numEpisodes` and no `episodes` array. No query
  parameter combination makes that route return episodes.
- Episodes exist per show, on `GET /api/items/{id}?expanded=1` (`PodcastEpisode.toOldJSONExpanded`):
  `title`, `description`, `publishedAt` (epoch **milliseconds**), `duration`, `audioFile`, `audioTrack`.
- Those episodes are the **downloaded** ones: `PodcastEpisode` rows are created only when a download
  completes (`PodcastManager.createEpisode` → `createFromRssPodcastEpisode(rssEpisode, podcastId, audioFile)`,
  `audioFile` required). Feed episodes that were never downloaded are not library entries.
- The only library-wide episode queries are the personalized shelves (`continue-listening` = in-progress,
  `listen-again` = finished, `episodes-recently-added`), and they return library items carrying a single
  `recentEpisode`, limited in count — not episode lists.
- Played state needs no new call: `GET /api/me/progress` already gives `(libraryItemId, episodeId) →
  currentTime, isFinished`, and the tab already reconciles daemon-acknowledged progress.

## Goals / Non-Goals

**Goals:**

- The podcast tab renders one flat, grouped episode list scoped by a single mutually exclusive pill bar:
  `All` / `Unplayed` / `Played`, then one pill per subscribed podcast, with `[`/`]` walking that bar.
- Grouping reuses `feeds_model.rs` verbatim (five age groups, same day boundaries).
- Episode loading is lazy and scoped to the active pill: one per-show detail fetch per show per session,
  bounded, appended progressively, refreshed on tab activation and on the refresh key.
- The hero presents the selected episode (no Workspace) over the parent show's cover.
- Activation lands on the existing play/enqueue boundary with Feeds' semantics.

**Non-Goals:**

- No Inline Search for the podcast tab: it has never hosted an `InlineSearchHost` session
  (`shell_inline_search.rs` documents Home/Feeds/podcast/book as non-hosts), and this change does not add one.
- No changes to podcast playback, queueing, source-resolution, or progress-refresh capabilities.
- No Audiobookshelf Books tab changes (surname ranges stay).
- No new daemon persistence; the remembered pill is session memory only.
- No restructure of `library_panel` slot semantics, `pill-selector-presentation`, or the `MediaList`
  heading machinery — all reused as-is.

## Decisions

### D1 — Mirror the Feeds owner shape instead of adapting the TV-parallel one

`PodcastContent` shrinks to the `feeds_content.rs` shape: one episode `MediaListCarrier` with
heading/spacer rows, one pill Selector row, a hero without a Workspace, and leaf activation. Show rows, the
show hero workspace, the season-position filter logic, and the selection modal are deleted rather than
re-pointed — every one of them is dead under the new pill bar. The row list is built by a
`PodcastDisplayRow` builder mirroring `FeedDisplayRow` (headings resolved from the episode slice, indices
unchanged), and the carrier's target is the existing `PodcastEpisodeTarget` (parent `library_item_id` +
`episode_id`), so heading insertion and page arrivals cannot shift targeting.

### D2 — One loading contract: paged show list plus per-show episode fan-out

The show list (which is also the pill list) comes from the existing bounded page fetch
`GET /api/libraries/{id}/items?page=&limit=`; each show's episodes come from
`GET /api/items/{id}?expanded=1`. This is the only complete episode source the server offers (see Context),
and on a personal library it is a handful of requests. The alternative — a single paged "expanded items"
call — does not exist: the library-items route ignores `expanded` and serializes minified media, so a plan
built on it silently returns zero episodes.

`AudiobookshelfDownloadedEpisode` gains `description` (the hero needs it). `published_at` stops being an
opaque string: the wire boundary normalises it to unix seconds, accepting Audiobookshelf's epoch
milliseconds (its actual shape for downloaded episodes), epoch-second numbers/strings, and ISO-8601 or
RFC 2822 text (what the existing fixtures carried); anything else, and a missing date, is `None` and groups
as `Unknown date`. Without that conversion every episode would group as `Unknown date`, because
`feed_age_group` compares against `current_time_secs()`. The ambiguity is resolved once, at the wire
boundary, so no renderer parses a date.

### D3 — One selector, resolved in the owner, keyed by identity

`build_show_title_buckets` leaves this tab (Books keep `build_surname_buckets` and its pills). The selector
is one `SelectorRow`: `All`/`Unplayed`/`Played` first, then per-show pills truncated like Feeds' group
labels. `SelectorPicked(index)` branches on `index < 3` exactly as Feeds branches on `WatchedFilter::COUNT`.

The owner stores the selection as a **value**, not a position: `PillSelection::{State(AudiobookshelfEpisodeFilter),
Show(String /* library_item_id */)}` (the remembered pill is the same value). Stores the index and the
filter silently rebinds whenever the show list grows or re-sorts as pages land — `append_page` sorts shows
by title — so the active view would change under the user. The painted active index is derived from the
value each frame; movement driving persistence or an effect resolves the value from the owner and sends it,
never an index for the shell to re-resolve.

### D4 — `[`/`]` walk the pill bar; pills are one kind

`[`/`]` move through the whole bar — state pills and show pills in the painted order, wrapping — and nothing
else on the tab owns those chords. There is no separate key for the state pills and no per-kind behaviour:
the bar is one selector, so its navigation is one gesture. The pill row also remains pointer-selectable
through the panel's existing `layout.selector_tabs` hit regions.

### D5 — Lazy, scoped episode loading with activation-scoped refresh

The active pill decides what is fetched. A show pill needs that show; a state pill needs every subscribed
show, so the first state-pill view fans out across shows with bounded in-flight requests, appending each
show's episodes as they arrive (no episode cap, no visible reload of already-listed rows). Each show is
fetched at most once per session; later views read the cache and re-filter it through the progress map.

Refresh follows the existing shape: activating the tab refreshes what the active pill needs (the precedent
is `activate_audiobookshelf_position` → `start_audiobookshelf_detail`), and the refresh key reloads
(`audiobookshelf_refresh` clears and re-issues). Every result is reconciled with the Service setup
generation that initiated it, as catalog results already are.

### D6 — Activation reuses the podcast episode intents, re-aimed at list rows

Enter (or double-click) maps to the existing `PodcastEpisodeIntent::OpenOrPlay(target)`; Ctrl+A to
`Enqueue`; the context menu is unchanged. Because episodes are the tab's rows and the hero has no Workspace,
the tab has no inline detail block and no selection modal, and podcast episodes are not hero-bearing
leaves: Enter plays, in every geometry. The queue item's parent-show metadata (`show_title`, `author`,
`cover_path`) is resolved from the episode's own `library_item_id`, not from the current selection —
`selected_show()` no longer describes a queue target once selection is episode identity.

### D7 — The episode hero extends the one existing episode producer

`hero_content_abs_episode` is already the single producer for an Audiobookshelf episode (the queue panel and
Home rows consume it). The tab calls the same producer; this change corrects the facts it emits to what the
episode actually has: title = episode title, metadata rows = podcast name, duration and publish date,
overview = the cleaned episode description, artwork = the parent show's cover through the existing Square
artwork policy and cover fetch/cache keyed by the show's `library_item_id`. There is no credits block and no
author row: no such field exists on the episode payload, and inventing one is exactly how the previous plan
drifted. `hero_content_abs_show` becomes unused and is deleted.

### D8 — Grouping reuses `feeds_model.rs` verbatim

`FeedAgeGroup`/`feed_age_group` keep one implementation; the podcast view supplies the same day-boundary
criteria from unix seconds. Two adaptations are named explicitly so they are not silently re-derived: the
function's visibility widens to the app crate, and the flat episode slice is sorted newest-first **globally**
before grouping (`feed_display_rows` merges only consecutive runs, so an interleaved slice would repeat
headings). Undated episodes sort last and land in `Unknown date`, as in Feeds.

## Risks / Trade-offs

- [One detail request per subscribed show for the state pills] → bounded in-flight requests, append-once,
  and cache-per-session; show pills cost one request, and the fan-out only runs for a state pill. A
  library with hundreds of shows is the pathological case; the pills keep working, they just fill in.
- [The pill selection must survive a growing, re-sorting show list] → it is stored as a value (D3) and the
  painted index is derived per frame; this is the failure mode the previous index-keyed design had.
- [Deleting the show browser and its Workspace changes which state survives provider refresh] → the
  re-anchor-on-refresh discipline moves from show identity to pill identity plus selected episode identity;
  the existing stable-selection refresh tests are rewritten around episodes.
- [A saved library position still names a show] → the stored `focused_item_id` is a show id under the old
  model; restore ignores a value that is not an episode target and the tab starts on the remembered pill's
  first episode. No migration is needed and none is performed.
- [Narrow geometry has no hero pane] → with Enter playing directly, the podcast tab is not hero-bearing and
  does not enter the Library Hero overlay flow; the episode description is reachable where the Wide hero is.
  This is the direct consequence of "the hero no longer needs a workspace" plus "Enter plays".
- [Two specs (`audiobookshelf-podcast-browsing`, `audiobookshelf-podcast-library-ui`) carried overlapping
  TV-parallel requirements] → this change removes the show-browser requirements from both and keeps the
  presentation obligations in one place per capability; sync on archive resolves the drift.

## Migration Plan

Single-client terminal app, no data migration. Ship as one change: wire normalisation, browse-state
restructure, owner rewrite, render/spec/test updates. Rollback is `git revert`; no persisted state changes.

## Open Questions

- Whether show pills render for shows with zero downloaded episodes. Spec default: render the pill and show
  the scoped empty state, since the pill bar is the tab's show list.

## Why

The Audiobookshelf podcast tab's panel pill bar is the Books surname-range machinery pointed at show
titles: on a personal library of a handful of shows it collapses to one or two no-op pills, and the tab
has no keyboard path to it at all (`[`/`]` are consumed by the hero's episode filters). The bar was never
specified for podcasts. This change specifies it from what podcasts actually have — play state and the
subscribed shows — over one flat episode list, and simplifies the hero accordingly.

## What Changes

- **BREAKING** — The podcast tab becomes a flat episode browser: the show list, the selected-show hero
  workspace, and the episode selection modal are removed. Downloaded episodes are the tab's list rows.
- The panel pill bar becomes one selector: the `All` / `Unplayed` / `Played` state pills followed by one
  pill per subscribed podcast. `All` lists every podcast's episodes, `Unplayed` the unplayed and
  in-progress ones, `Played` the finished ones, and a show pill lists that show's episodes. Exactly one
  pill is active; state and show selections do not combine. `[` / `]` move through the pill bar — there is
  one uniform pill kind, with no per-kind key or behaviour.
- Every pill view groups its episodes under the five Feeds age-group headings (`New`, `Recent`,
  `Older than two weeks`, `Older than a month`, `Unknown date`), reusing `feeds_model.rs` criteria verbatim.
- Rows are split rows — podcast name, then episode title, with duration and played/in-progress state — so
  an episode is identifiable on the views that span shows.
- Episodes load lazily for the active pill and without an episode cap: the paged show list comes from
  `GET /api/libraries/{id}/items` (which returns shows only), and each show's episodes from
  `GET /api/items/{id}?expanded=1`, fetched at most once per show per session, appended as they arrive,
  and refreshed when the tab is activated or the refresh key is pressed.
- The hero presents one episode: episode title, podcast name, duration, publish date, and the episode
  description in the overview. No Workspace. The parent show's cover stays in the existing Square image slot.
- Activation: Enter or double-click on an episode row plays it through the existing podcast playback
  admission path; Ctrl+A enqueues; the context menu is unchanged.
- The last active pill is remembered in session memory (survives tab switches, resets to `All` on restart).
- Non-goal: Inline Search. The podcast tab has never hosted an Inline Search session and does not gain one.

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `audiobookshelf-podcast-library-ui`: The alphabetical-panel-pills requirement is removed for podcasts
  (Books keeps its ranges). The show hero, the responsive show-browser composition, the selection modal,
  and the stale read-only activation requirement are removed. Added: the episode hero over the parent
  show's cover, the flat episode browser, and the one state-and-show pill selector with its keyboard path.
- `audiobookshelf-podcast-browsing`: The show-pagination, TV-parallel composition, show-hero artwork,
  season-selector→played-state-filter mapping, and TV-episode-presentation requirements are removed.
  Added: the flat episode feed presentation, the shared list row presentation, and the load contract
  (paged show list plus per-show episode fan-out). Progress, artwork, lifecycle, activation, and
  daemon-reconciliation requirements survive unchanged.
- `library-list-hero`: The narrow-podcast scenario becomes ordinary episode rows with their hero in Wide
  geometry only (no inline replacement, no modal).
- `library-hero-overlay`: Podcast shows leave the "parent with Workspace opens focused Workspace" scenario;
  podcast episodes are not hero-bearing leaves and do not open the overlay.
- `library-panel`: Podcast episodes leave the Wide Workspaces constituent-list clause; the narrow podcast
  scenarios describe ordinary rows with no overlay.

## Impact

- `crates/mbv-core/src/audiobookshelf_catalog.rs` — `AudiobookshelfDownloadedEpisode` gains `description`;
  `published_at` is normalised to unix seconds at the wire boundary (Audiobookshelf reports it as epoch
  milliseconds), so duration/date fields are usable downstream. The paged show list and the per-show
  expanded-item fetch are the existing 2.36 calls; no new endpoint is introduced.
- `src/app/types_audiobookshelf_browse.rs` — podcast browse state becomes the flat episode list plus the
  per-show episode cache and the show list that feeds the pill bar; surname/title buckets leave this tab.
- `src/app/components/podcast_content.rs` — the owner projects rows, headings and the pill selector;
  episode loading becomes lazy and scoped to the active pill; the hero becomes the episode hero.
- `src/app/library_position_state.rs`, `src/app/audiobookshelf_browse_actions.rs`, `src/app/lib_event_actions.rs`
  — the show-selection callers: saved position identity, detail-completion handling, and the queue item's
  parent-show metadata, which becomes a lookup by the episode's own `library_item_id`.
- `src/app/render/screens/feeds_model.rs` — `FeedAgeGroup` and `feed_age_group` are reused verbatim
  (one visibility change); the podcast row builder is new.
- Shell projections, image projection (parent-show cover for the episode hero), and the podcast
  tick-integration tests are reworked; `library_panel` slot semantics are unchanged.
- Specs listed under Capabilities. No changes to podcast playback/queueing/source-resolution,
  `pill-selector-presentation`, `inline-library-search`, or `feeds_model` behaviour.

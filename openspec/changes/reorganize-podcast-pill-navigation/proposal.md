## Why

The Audiobookshelf podcast tab's panel pills are the Books surname-range machinery pointed at show titles: with a personal library's handful of shows they collapse to one or two near-no-op pills, and the tab has no keyboard path to them (`[`/`]` are bound to the hero episode filters). The user has let this slide because the pill bar was never meaningfully specified for podcasts. This change replaces alphabetical filtering with an organisation built on what podcasts actually have: per-show episode feeds with play state and publish dates, structurally matching the Feeds tab that already works this way.

## What Changes

- **BREAKING** — The podcast tab becomes a flat episode browser, mirroring Feeds: the show list, the selected-show hero workspace, and the episode selection modal are removed.
- The panel pill bar becomes one mutually exclusive selector row: `All` / `Unplayed` / `Played` state pills followed by one pill per subscribed podcast show. State pills and show pills do not combine.
- Every pill view lists episodes grouped under the five Feeds age-group headings (`New`, `Recent`, `Older than two weeks`, `Older than a month`, `Unknown date`), reusing `feeds_model.rs` criteria verbatim. `Unplayed` includes missing and incomplete (in-progress) progress; `Played` is completed episodes only.
- Flat episode data arrives through paged `GET /api/libraries/{id}/items?expanded=1` (each item carries its full episode list), filling the list progressively as pages land; no episode cap. The per-show detail fan-out and its cache stay only where still needed.
- The hero focuses on episode information: episode title, description, duration, resume/finished state, plus parent-show credits (show name, author) and the parent show's cover in the existing Square slot. No Workspace.
- Activation: Enter or double-click on an episode row plays it through the existing podcast playback admission path; Ctrl+A enqueues without playing; context menu unchanged.
- The last active pill is remembered in session memory (survives tab switches, resets to `All` on restart).
- Inline search over the tab keeps working, scoped to episode rows.

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `audiobookshelf-podcast-library-ui`: The alphabetical-panel-pills requirement is removed for podcasts (Books keeps its ranges). The hero/workspace requirements are replaced by an episode hero without a Workspace. The selection-modal requirement is removed. The responsive-hero requirement is updated to the Feeds-parallel leaf presentation.
- `audiobookshelf-podcast-browsing`: The TV-parallel composition requirement (Series/Season substitution table) is replaced by the flat episode-feed presentation. The season-selector↔played-state-filter mapping requirement is replaced by panel-pill state filtering. The selection-modal and TV-episode-presentation requirements are replaced by the flat list's row semantics. Progress, artwork, lifecycle, activation, and daemon-reconciliation requirements survive with wording adjusted to the flat list.
- `library-list-hero`: The "Narrow Audiobookshelf podcast" scenario changes from selected-show inline detail + modal to selected-episode inline detail (Feeds-style leaf replacement); the panel-pill clause updates to the new selector.
- `library-hero-overlay`: Podcast shows leave the "parent with Workspace opens focused Workspace" scenario; podcast episodes become hero-bearing leaves (first Enter opens the overlay, second Enter performs the existing activation).
- `library-panel`: Podcast episodes leave the Wide Workspaces requirement's constituent-list clause; the narrow podcast scenarios update from the show+Workspace model to the episode-leaf model (ordinary rows, overlay on Enter).

## Impact

- `crates/mbv-core/src/audiobookshelf_catalog.rs` — new bounded expanded-items wire path (`ItemsResponse` gains episode payloads; `AudiobookshelfDownloadedEpisode` gains description; pagination contract unchanged).
- `src/app/types_audiobookshelf_browse.rs` — browse state restructured from shows+detail-cache to the flat episode list; surname/title buckets gone for podcasts.
- `src/app/components/podcast_content.rs` — owner projects episode rows, headings, and the combined selector; episode focus/filter/hero-workspace logic removed; play/enqueue intents re-aimed at list rows.
- `src/app/render/screens/feeds_model.rs` — reused as-is (no change expected).
- Shell projections, image projection (show cover for episode hero), and the podcast-related tick-integration tests reworked; `library_panel` slot semantics unchanged.
- Specs listed under Capabilities; no changes to podcast playback/queueing/source-resolution specs, `pill-selector-presentation`, or `feeds_model` behavior.

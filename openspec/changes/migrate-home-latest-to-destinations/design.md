# Design

## Context

See `proposal.md` — Why. `fetch_home` (`src/app/library_load_actions.rs`) currently collects Continue Watching plus `get_latest_episodes(view_id, 30)` for TV and `get_latest(view_id, 30)` for every other visible non-playlist Emby library; Audiobookshelf's cached Newest Episodes shelf and the Feeds tab's loaded combined entries are separately merged into Home. `Model.home_content`, `tv_latest_snapshots`, and `acknowledged_home_latest_sources` hold Home/TV presentation and marker state. Music has a tree browser behind group pills; Movies/Home Videos/Generic have flat browse owners; podcasts and Feeds have distinct embedded owners. All destinations already use the Library Panel selector/list slots.

**Remote-response evidence and scope:** This change reuses existing endpoints and parsed data, without introducing an API query or assuming a new response shape. The archived `2026-08-16-extend-home-latest-abs-feeds/design.md` records a live Audiobookshelf 2.36.0 `/api/libraries/{id}/personalized` response: podcast `Newest Episodes` entries carry episode metadata, while book libraries have no comparable recency shelf. The Emby TV Latest request and behavior are already exercised by `rework-tv-library-pills`; non-TV `get_latest` is already called for every eligible Emby view by `fetch_home` today. Feeds Latest comes from locally loaded entries, not a new endpoint. If a destination requires a different remote request to meet the spec, **pause and probe the real Service, record the response here, and revise the plan before writing code**; never turn that probe into a live automated test.

## Goals / Non-Goals

**Goals:** One destination-owned selection and one row flow per active mode; preserve each source's data/activation behavior while eliminating Home duplicates. Preserve the existing launch window and sticky selector rules.

**Non-Goals:** A new Latest backend, book-library recency, a second Music tree, cross-Service Continue Watching, or changing TV's Upcoming/series hierarchy.

## Decisions

### D1. Keep Latest at each destination's existing selector boundary

Each embedded content owner supplies its Latest pill through the Library Panel's `SelectorRow`, switches the same destination's list-slot content, and translates stable item actions. TV keeps its existing `TvContentMode::Latest`; Movies/Home Videos/Generic add a Latest choice to their letter/group selector without changing the legacy letter/group defaults; Music adds a flat Latest flow in the list slot alongside its existing group/tree browsing (not a parallel mounted owner); podcasts add `Latest` to the closed pill selection; Feeds adds it to its existing selector without changing the watched-filter/group state when returning. Inline Search continues to supersede the selector while active. Alternative — reuse Home as a hidden owner and route its cursor into libraries — violates component-local state authority and makes destination selection dependent on another surface.

### D2. Move snapshots to their actual sources, not to another copy of Home

Keep an Emby Latest snapshot keyed by library identity and populate it with the existing TV/non-TV library-scoped requests; TV's snapshot may be reused but must no longer project into Home. Audiobookshelf Latest reads the existing per-library shelf cache; Feeds Latest reads the already loaded combined entries. Emby destination refresh updates its own Latest source; Feeds remains manual-refresh-only: merely opening or choosing Latest does not fetch a feed, while `r` refresh updates the loaded entries. Podcast refresh currently reloads shows but not the Newest Episodes shelf; refetching the shelf is separate issue #763, not part of this change. An Emby destination can load Latest without Home initialization. Home fetches only Continue Watching; its legacy Latest section composition, splice, and per-provider Home update events are retired rather than left running unseen. Alternative — keep Home's fetch and just hide its pills — wastes requests, retains divergent owners, and breaks Home-independent loading.

### D3. One launch marker and acknowledgement per source

Reuse `HomeLatestLaunchWindow`'s timestamp semantics (renaming Home-specific identifiers as appropriate). Source-qualified marker computation happens on the destination Latest snapshot; acknowledgement remains shell-owned across async completion and refresh, keyed by Emby view id, Audiobookshelf library id, or Feeds. A selected Latest pill is acknowledged even if data arrives later; changing to another pill does not retroactively mark new data after the launch instant. No Home marker or synchronization to Home remains. Alternative — each component tracking visited flags — reintroduces the async-replacement bug the TV migration already solved.

### D4. Sunset the option at the boundary

Remove `hidden_latest` from the config model, Settings selector/control, parsing and save output, Home fetch filters, and tests. Existing TOML keys are tolerated as unknown on load and omitted on save; `hidden_libraries` remains intact. Retired Home Latest launch-state identities fall back to Continue Watching; destination launch snapshots use stable selector identities rather than positional indices. Add a stable Latest selector identity for each eligible non-TV destination so exit/relaunch can restore it when that destination was selected at exit, without changing existing saved identities; other podcast tabs still start on All after a restart. TV continues its existing position persistence. Alternative — keep a dormant `hidden_latest` field for compatibility — silently leaves users believing it works.

### D5. Destination presentation, not Home presentation

The destination's normal `MediaList` row/projection produces the existing Home Latest title parts and provider date gutter. Podcast and Feeds reuse their current Hero and list arrangements; TV keeps its flat Latest/Upcoming geometry and direct-play rules; Music's group tree remains intact for non-Latest modes. Use the existing Library Panel slots at Wide and Narrow; no destination paints its own panel or invents a second keyboard router. Tests belong to row/selector owners, focused render buffers, and mounted `Application::tick()` for routing/launch state.

## Risks / Trade-offs

- Music's group/tree selector currently has no flat Latest arm; switching views must preserve its existing tree state and actions without a second owner. → Test round-trips between Latest and group browsing at Narrow and Wide and confirm one list painter.
- Old Home Latest selection and `hidden_latest` settings may be persisted. → Gracefully fall back on load, tolerate old TOML on read, omit it on save; cover both with hermetic tests.
- Feeds refresh and podcast shelf completion are async relative to selection. → Derive Latest rows from the existing loaded snapshots and keep acknowledgement keyed by source, not row offset. On Feeds Latest, `w` leaves Latest, restores the prior group, and cycles its prior watched filter, matching the existing Feeds chord; shelf refetch on podcast refresh is tracked separately in #763.
- Active `migrate-tv-library-to-tree-browser` work touches TV's content owner. → Keep Latest flat and avoid overlapping tree mechanics; review the delta against that change's full artifacts before implementing.
- Existing `home-latest-sections` Purpose will be stale after archive once its Latest requirements are removed. → Update the main spec's Purpose during the close-out sync to describe Home's Continue Watching contract; do not edit the main spec during this planning phase.

## Migration Plan

Land source/selector identity first, then each destination's Latest view, then remove Home Latest population and the obsolete setting once destination routes exist. Migrate launch-state reads with fallback before removing old writes. Run focused component and mounted-tick tests plus `cargo nextest run -p mbv` and `cargo nextest run -p mbv-core`; manually inspect Home and each destination at Narrow and Wide before archiving any visible TUI change. Reverting the change restores the previous Home sections and config behavior; no persistent media data is rewritten.

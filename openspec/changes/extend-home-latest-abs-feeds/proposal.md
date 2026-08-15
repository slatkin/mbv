## Why

This is a two-part change. First: Home's population model is Emby-only at the architecture level — `fetch_home()` hard-fails without an Emby client, and the async handler that fills Home when Emby connects (`apply_emby_bootstrap()`) wholesale-replaces Home's data, both because Emby was Home's only provider when they were written. That leaves Home short of the already-shipped `service-independent-startup` spec (#503), which expects browsing to work with any subset of Remote Services configured — Home just never caught up. Second, and only buildable on top of the first: Home's "Latest" pills (#543) only ever show Emby content, so Audiobookshelf podcast libraries and Feeds — which the app already browses and plays elsewhere — have no presence on Home at all.

Decoupling Home's population from Emby is the primary design decision here, not a prerequisite footnote: without it, Audiobookshelf/Feeds Latest pills would only ever appear for users who also have Emby configured and connected, which defeats the point of adding them.

## What Changes

### Part 1 — Decouple Home's population from Emby's connection state

- Split `fetch_home()`'s Emby dependency so its Emby-derived portion (`continue_items`, Emby `latest` entries) is skipped, not a hard `Err`, when no Emby client exists; every other portion of Home population no longer depends on Emby being configured, connected, or reachable.
- Drop the `if self.emby_client().is_some()` gate around Home's initial population at TUI entry (`src/app/mod.rs`), so Home populates from whatever Services are actually available at that moment.
- Change `apply_emby_bootstrap()` (the async handler that runs once Emby's independent startup connection completes) to merge its Emby-derived entries into Home by source identity instead of replacing Home's data outright, so Emby connecting after other Services have already populated Home does not clear them.
- Continue Watching stays explicitly out of scope and Emby-only for this change: `continue_items` still populates only when an Emby client exists.

### Part 2 — Add Home's Audiobookshelf Latest pill

- Give each Audiobookshelf **podcast** library its own Home "Latest" pill, populated from `AudiobookshelfClient::shelves_bounded`'s `Newest Episodes` shelf (the ABS-native recency concept for podcasts). Audiobookshelf **book** libraries get no Latest pill in this change — ABS's `/personalized` has no reliable recency shelf for books (verified against a live server), and book libraries aren't surfaced in the app yet pending #536.
- Fetch each Audiobookshelf podcast library's `Newest Episodes` shelf asynchronously off the existing ABS catalog-completion path (mirroring the existing per-library `start_audiobookshelf_shows` spawn), not as a blocking call during TUI startup, and cache the result so `fetch_home()` can rebuild that pill's entries (e.g. after a `hidden_latest` change) without a new network fetch.
- Rewrite `ShelfWire`/`ShelfEntryWire`/`AudiobookshelfShelf` parsing to match ABS 2.36.0's actual `/personalized` wire (`entities` key, full minified library-item entries with a top-level `recentEpisode`, `media.metadata.{title,author}` and `media.coverPath`) instead of the pre-fix shape that the server never returns — the earlier parser always errored, so no ABS data ever reached Home.
- Give every section in `home.latest` a pill that always renders, empty or not (Continue Watching convention), so a bare Audiobookshelf library or an unloaded Feeds pill is still visible as `(empty)` rather than vanishing.
- Retype `HomePane.latest` from `Vec<(String, String, Vec<EmbyItem>, usize)>` to use the existing cross-provider `QueueItem` enum, keyed by a new `HomeLatestSource` (Emby library / Audiobookshelf library / Feeds), so Home automatically gains the future `AudiobookshelfBook` variant when #536 lands.
- Play/enqueue for an Audiobookshelf Home item constructs its `QueueItem` directly and submits it through a new shared queue-submission primitive extracted from `play_selected_audiobookshelf_episode`'s existing tail logic. Emby items keep using the existing `play_item`/`select_home` path unchanged. (A cursor-borrowing delegate, the way Continue Watching reaches Emby's resume path, does not generalize here: the Audiobookshelf tab resolves its "selected item" through a filtered `episode_filter`/`visible_episodes()` view, so a cursor borrowed from Home could point at an item that filter would hide.)
- Add a minimal, provider-generic selected-item detail treatment (title, duration, cover if available) and a generic list-row renderer for non-Emby Home rows, rather than extending the Emby-specific `render_compact_detail` path.
- Extend `hidden_latest` (already name-matched, no schema change) to also match Audiobookshelf library names via the existing settings multiselect overlay.

### Part 3 — Add Home's Feeds Latest pill

- Fold Feeds in as **one** pill ("Latest Feeds"), wrapping the Feeds tab's existing flattened, newest-first `all_entries` — not one pill per subscription. No new fetch: Feeds already loads independently of Emby, so this reads `FeedTabState.all_entries` as it stands whenever Home population runs.
- Extend the shared queue-submission primitive built in Part 2 to `feed_tab_play_selected`/`feed_tab_enqueue_selected`'s near-duplicate tail logic, so Home can play/enqueue a Feed item the same way. (Same reasoning as Part 2: the Feeds tab resolves its "selected item" through `watched_filter`+`selected_group`/`visible_entries()`, so Home cannot borrow its cursor.)
- Reuse Part 2's generic Home row/detail renderer for Feed items; confirm a Feed entry with no known duration or artwork degrades cleanly rather than erroring.
- Extend `hidden_latest` to also match a synthetic `"Feeds"` pseudo-name.

## Capabilities

### New Capabilities
- `home-latest-sections`: Home's per-destination "Latest" pills (Emby library, Audiobookshelf podcast library, and the single flattened Feeds pill), including population, hiding via `hidden_latest`, selection/detail display, and play/enqueue for any pill's items — populated independently of Emby's connection state.

### Modified Capabilities
- `service-independent-startup`: Adds scenarios to the existing "TUI entry is independent of Remote Services" requirement making explicit that Home browsing (Audiobookshelf and Feeds Latest pills) is available without Emby, and that Emby connecting later does not remove Latest pills populated from other Services.

## Impact

- **Code — Part 1**: `src/app/library_load_actions.rs` (`fetch_home`: Emby portion becomes conditional), `src/app/mod.rs` (drop the startup gate), `src/app/app_emby_service_completion.rs` (`apply_emby_bootstrap`: merge instead of replace), `crates/mbv-core/src/service_runtime.rs` (`EmbyBootstrap.latest` shape), `src/app/types_playback.rs` (`HomePane.latest` retype to `HomeLatestSource`/`QueueItem`, needed to key the merge).
- **Code — Part 2**: `crates/mbv-core/src/audiobookshelf_catalog.rs` (`ShelfEntryWire`/`ShelfWire` widening), `src/app/run_loop_drains.rs`/`service_startup.rs`/`types_events.rs` (new Audiobookshelf shelf-fetch off the existing catalog-completion path, plus a small shelf cache), `src/app/library_load_actions.rs` (`fetch_home`: Audiobookshelf pill population), `src/app/home_actions.rs` (shared submit primitive extracted from `audiobookshelf_browse_actions.rs`), `src/app/render/home.rs` plus a new small render module for non-Emby Home rows (both `home.rs` and `render/audiobookshelf.rs` are within ~170 lines of the 800-line file cap).
- **Code — Part 3**: `src/app/library_load_actions.rs` (`fetch_home`: Feeds pill population), `src/app/feed_tab_actions.rs` (adopt Part 2's shared submit primitive), `crates/mbv-core/src/config_types_paths.rs`/settings multiselect (no schema change, `"feeds"` pseudo-name).
- **Behavior**: Home browsing (Latest pills) works with any combination of configured Services, including none. Audiobookshelf podcast libraries and Feeds gain Home Latest pills; Audiobookshelf book libraries and Continue Watching stay Emby-shaped/Emby-only for now. Existing Emby Home play/enqueue/routing behavior is unchanged.
- **Data/API**: New Audiobookshelf wire parsing for `/personalized` shelf entries. No changes to Emby or Feed data shapes.
- **Risk**: Medium. Part 1 changes the merge semantics of an existing async completion handler and a startup gate every Home population path relies on; Parts 2 and 3 touch the Home cursor/section/play model used by every existing Home interaction. All three parts are needed together — Parts 2 and 3's new pills are only reachable through Part 1's population model, and Part 3 reuses Part 2's shared submit primitive and renderer rather than duplicating them.

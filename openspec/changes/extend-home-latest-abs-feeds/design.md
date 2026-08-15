## Context

Home currently has exactly two writers, and both were built when Home had one provider (Emby). `fetch_home()` (`src/app/library_load_actions.rs:309-375`) requires an Emby client and returns `Err` for the whole function otherwise; every caller besides TUI entry (`ws_event_actions.rs:149`, three sites in `context_menu_actions.rs`, `library_browse_actions.rs:260`, the `hidden_latest` settings multiselect at `render/overlays/multiselect.rs:150`) discards that `Result` (`let _ = self.fetch_home();`), so it silently no-ops without Emby. `apply_emby_bootstrap()` (`app_emby_service_completion.rs:243-280`) is the async handler Emby's independent startup connection calls on completion (`emby_client()` is reliably `None` at the very first rendered frame — `mod.rs:302-332` — while Emby connects on a background thread, per #503); it replaces `self.home.latest` wholesale from `EmbyBootstrap.latest: Vec<(String, String, Vec<EmbyItem>)>`.

`service-independent-startup` (#503, already shipped) requires Home browsing to work without Emby configured or reachable. Home's population model never actually met that bar — it happened to be invisible only because Home had nothing else to show. Adding Audiobookshelf and Feeds content to Home makes the gap observable and consequential: without fixing population first, the new pills would only ever appear for users who also have Emby.

The cross-provider `QueueItem` enum (`crates/mbv-core/src/playback_queue_items.rs:132`) already wraps `Emby(Box<EmbyItem>) | Feed(FeedEntry) | Audiobookshelf(AudiobookshelfQueueItem)` and exposes `title()`, `duration()`, `display_name()`, `artwork_url()`, `is_audio()`/`is_video()`, `played()`, `playback_position_ticks()` — a provider-generic read interface Home rendering can use as-is. `FeedTabState.all_entries` (`src/app/types_feed_tab.rs:59`) is already the flat, cross-subscription, newest-first Feed list, populated independently of Emby via `spawn_idle_feed_fetch`/`refresh_feeds`. The image pipeline (`src/app/images.rs:519-590`) already dispatches per-item fetches by an `ImageSource` enum (`Emby` / `Audiobookshelf{..}` / disk-cache/URL fallback), used today by the podcast tab.

`play_selected_audiobookshelf_episode` (`audiobookshelf_browse_actions.rs:186`) and `feed_tab_play_selected`/`feed_tab_enqueue_selected` (`feed_tab_actions.rs:196+`) each resolve a `QueueItem` from their tab's own filtered cursor, then run near-identical tail logic: existing-slot lookup by content identity, append-if-absent, cursor/active-slot set, `submit_queue`/`queue_play_slot`, rollback+flash on rejection.

`AudiobookshelfClient::shelves_bounded` → `GET /api/libraries/{id}/personalized` (`crates/mbv-core/src/audiobookshelf_catalog.rs:219-228, 357-388`) is called from nowhere in `src/`. Verified against a live server (ABS 2.36.0): podcast libraries return a `Newest Episodes` shelf with entries carrying full embedded `media.metadata` and `recentEpisode`; book libraries return no comparable recency shelf. The in-repo `ShelfEntryWire` currently discards everything except `{type, libraryItemId, episodeId}`. Audiobookshelf's catalog already follows an async-completion pattern this can reuse: once ABS authentication completes, `apply_audiobookshelf_completion` spawns `start_audiobookshelf_catalog`, whose completion (`run_loop_drains.rs:52-99`) populates `audiobookshelf_libraries` and spawns one `start_audiobookshelf_shows` fetch per library.

## Goals / Non-Goals

**Goals:**

- **Primary**: make Home's population model provider-independent — Home browsing (Latest pills, and whichever Services are configured) works with any combination of Emby, Audiobookshelf, and Feeds, including none of them, matching `service-independent-startup` (#503).
- Make `HomePane.latest` provider-generic (`QueueItem`-backed) so Audiobookshelf podcast and Feeds Latest pills are additive data, not a parallel Home model, once population no longer requires Emby.
- Give Home a single, safe way to play/enqueue any pill's item regardless of provider, without depending on another tab's current filter state.
- Keep Emby Home behavior (routing, series auto-play-next, session handoff, and population once Emby is Ready) byte-for-byte unchanged.

**Non-Goals:**

- Audiobookshelf book libraries in Home. Blocked on #536; ABS has no reliable "latest" shelf for books, so this would need mbv's own `sort=addedAt` query mirroring `EmbyClient::get_latest` — separate follow-up work sized only after #536 lands.
- Continue Watching for Audiobookshelf or Feeds. `continue_items` stays Emby-only for now — it populates only when an Emby client exists, same as today. An Audiobookshelf/Feeds-only user sees Latest pills but no Continue Watching section; unifying resume/progress across providers is separate follow-up work.
- Feature-parity detail/expanded view for non-Emby Home rows. `render_compact_detail` is keyed on `lib_idx`/nav-stack and is Emby-specific; this change adds a minimal generic detail (title, duration, cover if available) rather than extending that path.
- Any change to Emby Home play/enqueue routing, session handling, or series auto-play-next.
- A fully live-reactive Home. Latest pills refresh on Home's existing refresh triggers (TUI entry, manual refresh, context-menu actions, ws events, settings changes) plus each provider's own connection-completion event; this does not add a new streaming/subscription mechanism to push a Feeds-tab background refresh onto Home while the user is idle on it.

## Decisions

### Part 1 — Decoupling Home's population from Emby

### 1. `fetch_home()`'s Emby-derived portion becomes conditional, not a whole-function gate

Split `fetch_home()`'s single `let Some(client) = self.emby_client() else { return Err(...) }` guards into per-portion conditionals: build `continue_items` and the Emby `latest` entries only `if let Some(client) = self.emby_client()`, otherwise leave that portion empty; every other portion of `home.latest` is built unconditionally from whatever local state exists. The function stops returning `Err` for "no Emby" — every existing call site that already discards the `Result` now actually updates Home instead of silently no-oping. The TUI-entry call site (`mod.rs:317-332`) drops its `if self.emby_client().is_some()` gate and calls `fetch_home()` unconditionally; this is not a new blocking call, because `emby_client()` is reliably `None` at that exact point (Emby's connection was just kicked off asynchronously on the previous lines), so the Emby portion is skipped there exactly as it already would be.

Alternative considered: leave `fetch_home()` Emby-gated and give other Services their own separate population function, called only from non-startup sites. Rejected — this would create two different code paths with different Emby-availability semantics for what is otherwise the same "populate Home" operation, and every one of the six existing call sites would need updating twice instead of once.

### 2. Retype `HomePane.latest` to `Vec<(String, HomeLatestSource, Vec<QueueItem>, usize)>`

Add a small `HomeLatestSource` identifying which provider/library a pill belongs to (Emby library id / Audiobookshelf library id / Feeds), replacing the current bare Emby library id `String`, and switch the item type from `EmbyItem` to the cross-provider `QueueItem`. This is Part 1 infrastructure, not a Part 2/3 feature: it is what gives Decision 3's merge something to key on, before either Audiobookshelf or Feeds writes a single entry. `fetch_home()`'s existing Emby population keeps working unchanged, now wrapping each `EmbyItem` in `QueueItem::Emby`. `home_current_item`, `home_section_range`, `home_new_sections`, and `home_select_section` change their element type but keep their existing index-flattening logic unchanged — they already treat sections generically.

Alternative considered: a Home-only enum instead of reusing `QueueItem`. Rejected — `QueueItem` already exposes `title()`, `duration()`, `display_name()`, `artwork_url()`, etc., and a second enum would need its own conversions to/from `QueueItem` at every play/enqueue boundary for no benefit, plus it would miss the free `AudiobookshelfBook` variant once #536 lands.

### 3. `apply_emby_bootstrap()` merges Emby entries into `home.latest` instead of replacing it

Change `EmbyBootstrap.latest` (`crates/mbv-core/src/service_runtime.rs`) to carry enough per-view identity to key a merge (view id, title, items). `apply_emby_bootstrap()` removes any existing `HomeLatestSource::Emby(_)` entries from `self.home.latest` and inserts the new ones at their previous positions where possible, leaving any `HomeLatestSource::Audiobookshelf(_)`/`HomeLatestSource::Feeds` entries untouched. This is required, not optional, once `home.latest` is provider-generic (Decision 2) — the existing wholesale-replace is correct only when Emby is the sole writer, which stops being true the moment Part 2 lands. Verifiable in isolation: a unit test can construct a `HomePane` fixture with a synthetic `HomeLatestSource::Audiobookshelf`/`::Feeds` entry and assert it survives `apply_emby_bootstrap()`, without needing Part 2/3's real fetch code to exist yet.

### Part 2 — Add Home's Audiobookshelf Latest pill

### 4. Audiobookshelf's Home shelf fetch follows the existing catalog-completion pattern, cached for `fetch_home()` to rebuild from

When the Audiobookshelf catalog completion resolves (`run_loop_drains.rs:52-99`, where `audiobookshelf_libraries` is set and one `start_audiobookshelf_shows` fetch is already spawned per library), also spawn one `shelves_bounded` fetch per podcast library, delivered via a new `LibEvent::AudiobookshelfShelfFetched { generation, library_id, result }` variant (same shape as the existing `AudiobookshelfShowsFetched`). Its handler stores the resulting `Newest Episodes` items in a small per-library shelf cache and upserts `home.latest`'s `HomeLatestSource::Audiobookshelf(library_id)` entry directly (inserting it if Home has never seen that library before), guarded by the same `SetupGeneration` staleness check every other ABS completion already uses. `fetch_home()`'s own Audiobookshelf portion rebuilds that pill's entries from the same cache (re-applying `hidden_latest`) rather than issuing a new network fetch — this is what lets `fetch_home()` stay safe to call unconditionally (Decision 1), including at TUI entry, without reintroducing a blocking call.

### 5. Play/enqueue by constructing and submitting a `QueueItem`, not by borrowing another tab's cursor

`home_play`/`home_enqueue` keep their existing Continue Watching branch (cursor swap into `select_home`) unchanged, because `continue_items` is unfiltered and that swap already works today. For a `latest` pill item, dispatch on the resolved `QueueItem`:

- `QueueItem::Emby(item)` → existing `self.play_item(*item)` / `self.do_enqueue_folder(*item)`, unchanged.
- Anything else (`QueueItem::Audiobookshelf(_)` today, `QueueItem::Feed(_)` once Part 3 lands) → a new shared `submit_queue_item(item: QueueItem, start_playback: bool)` helper, extracted from `play_selected_audiobookshelf_episode`'s existing tail logic (existing-slot lookup by `content_id()`, append-if-absent, cursor/active-slot set, `submit_queue`/`queue_play_slot`, rollback+flash on rejection). Home calls it with the `QueueItem` it already holds; the Audiobookshelf tab is refactored to call the same helper after it resolves its own filtered selection, so its behavior is unchanged (same code, one call site instead of a Home-specific duplicate). The `home_play`/`home_enqueue` dispatch itself doesn't need to change again in Part 3 — it already routes "anything non-Emby" through the shared helper.

Alternative considered: borrow-and-delegate by temporarily writing into the target tab's cursor/filter state, mirroring the Continue Watching swap. Rejected on inspection — `AudiobookshelfBrowseState.episode_selection` is only meaningful under the tab's current `episode_filter`. A Home-selected item might not exist in that filtered view at all, so the swap would need to also override the filter and locate the item's position inside a differently-ordered view, then restore all of it — more state to juggle than Continue Watching's single-field swap, and still fragile against a concurrent background refresh.

### 6. Widen Audiobookshelf shelf wire parsing to keep the fields Home needs

Change `ShelfEntryWire`/`ShelfWire` deserialization to also capture `media.metadata.{title,author,coverPath}` and `recentEpisode.{title,publishedAt,audioFile.duration}`, and change `shelves()`/`AudiobookshelfShelf` to surface a fully-populated `AudiobookshelfQueueItem` per entry instead of bare `library_item_id`/`episode_id`. This is real parsing work, not free reuse of an existing typed shape — the current `ShelfEntryWire` intentionally discards the payload.

### 7. Minimal generic detail treatment and a shared row renderer for non-Emby Home rows

`render_selected_home_video_detail`/`render_compact_detail` stay Emby-only and unchanged. A selected non-Emby Home row instead renders a small generic block using only `QueueItem`'s existing accessors (`title()`/`display_name()`, `duration()`, `artwork_url()` if `Some`) — no per-provider metadata in this change. Both the generic list-row renderer and this detail block are built here, in `src/app/render/home_latest_row.rs`, kept separate from `home.rs` (638/800 lines) and `render/audiobookshelf.rs` (628/800 lines, both with headroom but not much). Part 3 reuses this module for Feed items rather than adding a second one.

### 8. `hidden_latest` extension for Audiobookshelf: name-matched, no schema change

Audiobookshelf library names are matched into `hidden_latest` the same lowercased way Emby view names are today.

### Part 3 — Add Home's Feeds Latest pill

### 9. Feeds' Home entry is read from existing local state, not fetched separately

Home's Feeds pill is built by cloning `FeedTabState.all_entries` as it stands whenever Home population runs. No new Feeds fetch is introduced, and no async completion/cache mechanism like Decision 4's is needed: Feeds already loads independently of Emby via `spawn_idle_feed_fetch`/`refresh_feeds`, so `fetch_home()` can read it directly. A corollary is that the Feeds pill only reflects a Feeds-tab background fetch that completed *before* the next Home population trigger fires; that gap is accepted (see Non-Goals) rather than closed with new cross-tab notification plumbing.

### 10. Feeds tab adopts Part 2's shared submit primitive

`feed_tab_play_selected`/`feed_tab_enqueue_selected`'s near-duplicate tail logic (the same shape as `play_selected_audiobookshelf_episode`'s, per Decision 5) is refactored to call the same `submit_queue_item()` helper Part 2 built, after resolving its own filtered selection (`watched_filter`+`selected_group`/`visible_entries()`) unchanged ahead of the call. Home's `home_play`/`home_enqueue` needs no further change — its "anything non-Emby" branch already reaches `QueueItem::Feed(_)`.

### 11. `hidden_latest` extension for Feeds: synthetic `"Feeds"` pseudo-name

A literal pseudo-name `"Feeds"` is added for the single Feeds pill, matched the same lowercased way. A real Emby or Audiobookshelf library literally named "Feeds" would collide and become hideable/unhideable together with the Feeds pill; this is called out as an accepted, documented edge case rather than solved with a namespaced key.

## Risks / Trade-offs

- **Part 1 changes shared, load-bearing code**: the merge semantics of an existing async Emby-completion handler, and a startup gate every Home population path relies on. Mitigated by keying every writer's changes to `home.latest` by `HomeLatestSource` (Decision 2) so each writer only ever touches its own entries, and by reusing the existing `SetupGeneration` staleness guard each provider's completion handler already checks.
- **Home cursor/section model touches every existing Home interaction.** Retyping `latest` and its consumers is a wide, mechanical change; mitigated by keeping the flattening/index logic itself unchanged and covering it with focused tests per provider kind.
- **A second cache (Decision 4) for Audiobookshelf shelf data, alongside `home.latest` itself.** Extra state to keep in sync, in exchange for `fetch_home()` never needing a blocking Audiobookshelf network call. Mitigated by treating the cache as write-once-per-fetch, read-many, with `home.latest` always rebuilt from it rather than mutated ad hoc.
- **Queue-submission extraction changes an existing call site (Audiobookshelf in Part 2, Feeds in Part 3), not just Home's new one.** Mitigated by extracting the shared helper as a faithful lift of existing logic and keeping the existing tests as regression coverage for each refactor.
- **Audiobookshelf `/personalized` shape is undocumented outside this exploration's live-server check.** Mitigated by fixture-testing the widened parser against the recorded `shelves.json` fixture and treating any shelf other than `Newest Episodes` as unused.
- **Reduced Latest pill count if an Audiobookshelf server exposes no `Newest Episodes` shelf** (e.g., very old ABS versions). Home simply shows no pill for that library, same as an Emby library returning zero latest items today.
- **Feeds pill can go stale while the user is on Home** (Decision 9, accepted). If this proves confusing in practice, the follow-up is a Home refresh when `FeedTabState.rebuild_all_entries()` runs, not a new fetch mechanism.

## Open Questions

None.

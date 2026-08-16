## Part 1 — Decouple Home's population from Emby

### 1. Make `fetch_home()`'s Emby dependency conditional

- [x] 1.1 Split `fetch_home()`'s Emby guards so the Emby-derived portion (`continue_items`, Emby `latest` entries) is skipped, not an `Err`, when `self.emby_client()` is `None`.
- [x] 1.2 Drop the `if self.emby_client().is_some() { ... } else { flash }` gate around the initial `fetch_home()` call at TUI entry (`src/app/mod.rs:317-332`); call `fetch_home()` unconditionally there.
- [x] 1.3 Confirm every existing `let _ = self.fetch_home();` call site (`ws_event_actions.rs`, `context_menu_actions.rs` x3, `library_browse_actions.rs`, `render/overlays/multiselect.rs`) now runs Home population even with no Emby Service configured.

### 2. Retype `HomePane.latest` to be provider-generic and mergeable

- [x] 2.1 Add a `HomeLatestSource` type (Emby library id / Audiobookshelf library id / Feeds) in `src/app/types_playback.rs`, and change `HomePane.latest` to `Vec<(String, HomeLatestSource, Vec<QueueItem>, usize)>`.
- [x] 2.2 Update `fetch_home()`'s existing Emby population to wrap each `EmbyItem` in `QueueItem::Emby` keyed by `HomeLatestSource::Emby`.
- [x] 2.3 Update `home_current_item`, `home_section_range`, `home_new_sections`, `home_select_section`, `home_visible_indices` (`src/app/home_actions.rs`) to operate on `QueueItem` instead of `EmbyItem` (index-flattening logic itself is unchanged).
- [x] 2.4 Update every other reader of `HomePane.latest` (render, context menu, help) to compile against the new element type.

### 3. Merge, don't replace, Emby's async Home data

- [x] 3.1 Change `EmbyBootstrap.latest` (`crates/mbv-core/src/service_runtime.rs`) to carry enough per-view identity to key a merge.
- [x] 3.2 Change `apply_emby_bootstrap()` (`src/app/app_emby_service_completion.rs`) to remove and reinsert only `HomeLatestSource::Emby(_)` entries in `home.latest`, leaving any other entries untouched.

### 4. Verify Part 1

- [x] 4.1 Unit tests: `fetch_home()` populates and refreshes with no Emby Service configured, producing no Emby-related error, using a `HomePane` fixture with a synthetic non-Emby `HomeLatestSource` entry present before and after the call.
- [x] 4.2 Unit tests: `apply_emby_bootstrap()` on a `HomePane` fixture containing a synthetic `HomeLatestSource::Audiobookshelf`/`::Feeds` entry adds/updates only `HomeLatestSource::Emby(_)` entries and leaves the synthetic entry untouched.
- [x] 4.3 Unit tests: `apply_emby_bootstrap()` with no prior non-Emby data still populates Emby's pills correctly (no regression for the existing Emby-only path).

## Part 2 — Add Home's Audiobookshelf Latest pill

### 5. Widen Audiobookshelf shelf parsing

- [x] 5.1 Rewrite `ShelfWire`/`ShelfEntryWire` in `crates/mbv-core/src/audiobookshelf_catalog.rs` to match ABS 2.36.0's actual `/personalized` shape: shelf carries `entities` (not `entries`), each entity is a full minified library item `{ id, mediaType, media, recentEpisode }` with `recentEpisode` as a top-level sibling of `media` (no per-entry `type` tag), capturing `media.metadata.{title,author}`, `media.coverPath`, and `recentEpisode.{id,title,description,publishedAt,audioFile.duration}`.
- [x] 5.2 Update `AudiobookshelfShelf`/`AudiobookshelfShelfEntry` so a `Newest Episodes` entry carries enough fields to build an `AudiobookshelfQueueItem` (library_item_id, episode_id, title, show_title, author, duration, cover_path, published_at) without a follow-up fetch.
- [x] 5.3 Update `tests/fixtures/audiobookshelf/shelves.json` and add/extend a fixture test asserting the widened parse against the recorded live-server shape.
- [x] 5.4 Verify a shelf other than `Newest Episodes` (e.g. `Continue Listening`, `Discover`) parses without error and is simply unused by Home.

### 6. Fetch and cache Audiobookshelf shelf data asynchronously

- [x] 6.1 Add a `LibEvent::AudiobookshelfShelfFetched { generation, library_id, result }` variant (`src/app/types_events.rs`).
- [x] 6.2 Spawn one `shelves_bounded` fetch per podcast library alongside the existing `start_audiobookshelf_shows` spawn in the Audiobookshelf catalog-completion handler (`src/app/run_loop_drains.rs:74-82`).
- [x] 6.3 Handle `LibEvent::AudiobookshelfShelfFetched` by storing the `Newest Episodes` items in a per-library shelf cache and upserting `home.latest`'s `HomeLatestSource::Audiobookshelf(library_id)` entry directly, guarded by the existing `SetupGeneration` staleness check.

### 7. Populate Home's Audiobookshelf pill on refresh, and wire `hidden_latest`

- [x] 7.1 Update `fetch_home()` to rebuild the Audiobookshelf portion of `home.latest` from the shelf cache (Task 6.3), re-applying `hidden_latest`, without issuing a new network fetch.
- [x] 7.2 Extend the `hidden_latest`/`hidden_libraries` filtering in `fetch_home()` to also apply to Audiobookshelf podcast library names.

### 8. Shared queue-submission primitive

- [x] 8.1 Extract the tail of `play_selected_audiobookshelf_episode` (`src/app/audiobookshelf_browse_actions.rs`) — existing-slot lookup by `content_id()`, append-if-absent, cursor/active-slot set, `submit_queue`/`queue_play_slot` (or append-only for enqueue), rollback+flash on rejection — into a shared `submit_queue_item(item: QueueItem, start_playback: bool) -> bool` helper.
- [x] 8.2 Update `play_selected_audiobookshelf_episode` and its enqueue counterpart to call the shared helper, keeping their own filtered-selection resolution (`episode_selection`/`episode_filter`) unchanged ahead of the call.
- [x] 8.3 Update `home_play`/`home_enqueue` (`src/app/home_actions.rs`): keep the existing Continue Watching cursor-swap branch; for a `latest` pill item, dispatch `QueueItem::Emby` through the existing `play_item`/`do_enqueue_folder`, and everything else through the new shared helper.
- [x] 8.4 Run the existing Audiobookshelf play/enqueue tests to confirm the extraction is behavior-preserving.

### 9. Home rendering for the Audiobookshelf pill

- [x] 9.1 Add `src/app/render/home_latest_row.rs` with a generic list-row renderer for a `QueueItem` (title/`display_name()`, duration, marker/selection styling matching the existing Home row look).
- [x] 9.2 Add a generic hero detail for a selected non-Emby Home item (yellow bold wrapped title, show name, duration subtitle, blank separator, wrapped overview block, 16:9 image filling the column) matching the Emby Keep Watching hero structure, called instead of `render_selected_home_video_detail`/`render_compact_detail` when the selected item isn't `QueueItem::Emby`. Add `QueueItem::overview()` and carry `recentEpisode.description` on `AudiobookshelfQueueItem` so ABS episodes render an overview.
- [x] 9.6 Convert the Audiobookshelf description's HTML to terminal text at parse time (`html_to_text` in `mbv-core`, reused in `decode_entities`' module): `<p>`/`<br>`/block tags to paragraph breaks, decoded entities, links as `text (URL)`, inline formatting/images dropped.
- [x] 9.3 Wire cover-art loading for Audiobookshelf Home rows through the existing `images.rs` `ImageSource::Audiobookshelf` path.
- [x] 9.4 Update `render/home.rs`'s pill-row and item-list dispatch to call the new generic renderer for non-Emby pills, keeping the existing Emby two-column/hero code path unchanged.
- [x] 9.5 Remove the `!items.is_empty()` gate from `render_home_section_pills_row`, `render_home_list`, `home_new_sections`, and `home_section_is_valid` so every section in `home.latest` (Emby view, Audiobookshelf podcast library, Feeds) always renders its pill and an `(empty)` section when bare, matching the Continue Watching convention (Decision 12).
- [x] 9.7 Sort `home.latest` by a canonical provider rank (Emby, Audiobookshelf, Feeds) at the end of `merge_home_sections`, so async completion order never reorders the pill row (Decision 13).
- [x] 9.8 Truncate the generic Home hero's description to 200 display columns with an ellipsis before wrapping (Decision 14).
- [x] 9.9 Persist the selected Home pill by `HomeLatestSource` identity (`home_section` pref) and restore it once the matching section populates (Decision 15).

### 10. Verify Part 2

- [x] 10.1 Unit tests: Audiobookshelf pill construction/filtering (including `hidden_latest`), flat-cursor navigation across an Emby + Audiobookshelf mix, per-pill cursor restoration across `fetch_home()` refresh.
- [x] 10.2 Unit tests: playing/enqueueing an Audiobookshelf item from Home does not mutate the Audiobookshelf tab's own cursor/filter.
- [x] 10.3 Focused render tests for the generic Home row/detail renderer using Audiobookshelf items (with and without known duration/cover).

## Part 3 — Add Home's Feeds Latest pill

### 11. Populate Home's Feeds pill

- [x] 11.1 Update `fetch_home()` to add one Feeds entry cloning `FeedTabState.all_entries` into `QueueItem::Feed`.
- [x] 11.2 Extend the `hidden_latest` filtering in `fetch_home()` to also match the literal `"feeds"` pseudo-name.
- [x] 11.3 Auto-fetch feeds asynchronously at startup (flash-free `start_feed_fetch()` at TUI entry) and rebuild the Home Feeds pill when feed fetches drain, so the pill and Feeds tab are populated shortly after launch instead of staying empty until manual refresh.

### 12. Extend the shared submit primitive to Feeds

- [x] 12.1 Update `feed_tab_play_selected`/`feed_tab_enqueue_selected` (`src/app/feed_tab_actions.rs`) to call `submit_queue_item()` (built in Part 2), keeping their own filtered-selection resolution (`cursor`/`visible_entries()`) unchanged ahead of the call.
- [x] 12.2 Run the existing Feeds play/enqueue tests to confirm the refactor is behavior-preserving.

### 13. Confirm Home rendering covers Feed items

- [x] 13.1 Confirm the generic Home row/detail renderer (built in Part 2) renders Feed entries correctly; a Feed entry with no artwork degrades to no image, not an error.

### 14. Verify Part 3

- [x] 14.1 Unit tests: the "Feeds" pill reflects `FeedTabState.all_entries` newest-first, independent of the Feeds tab's own `selected_group`/`watched_filter`.
- [x] 14.2 Unit tests: playing/enqueueing a Feed item from Home does not mutate the Feeds tab's own cursor/selected group/filter.
- [x] 14.3 Focused render test for a Feed entry in the generic Home row/detail renderer (no known duration, no artwork).

## Final Verify

- [x] 15.1 Run `rtk cargo fmt --all -- --check`.
- [x] 15.2 Run `rtk cargo check --workspace --all-targets` and `rtk cargo clippy --workspace --all-targets`.
- [x] 15.3 Run `rtk cargo nextest run -p mbv-core` and the crate covering `src/app` (Home, Audiobookshelf, Feeds tests).
- [x] 15.4 Run `rtk make check-code-file-lines` to confirm `home.rs`, `audiobookshelf.rs`, and the new `home_latest_row.rs` stay under the 800-line cap.

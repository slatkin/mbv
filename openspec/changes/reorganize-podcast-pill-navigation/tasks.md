## 1. mbv-core wire path

- [ ] 1.1 Extend the expanded-items wire types so paged library items carry podcast episodes (extend `ItemsResponse`/`ShowWire` with the `EpisodeWire` episode list plus episode description); add `description` to `AudiobookshelfDownloadedEpisode`. Verify with new unit tests in `audiobookshelf_catalog_tests.rs` covering an expanded page, a missing-episodes item, and malformed responses (`cargo nextest run -p mbv-core`).
- [ ] 1.2 Add the paged expanded-items catalog fetch with the existing bounded-pagination contract (page/limit/total, one fetch in flight, stale-generation reconciliation unchanged). Verify with unit tests asserting page boundary, dedupe-on-append, and protocol errors (`cargo nextest run -p mbv-core`).

## 2. Browse state restructure

- [ ] 2.1 Restructure `AudiobookshelfBrowseState` for the flat episode list: replace show rows + surname/title buckets + detail cache with the episode list fed by expanded pages; keep `(library_item_id, episode_id)` identity, `needs_page` discipline, and stable selection across page arrivals. Verify by rewriting `types_audiobookshelf_browse.rs` unit tests (grouping boundaries, progressive append, selection survival, refresh-removes-selected) and deleting the bucket tests (`cargo nextest run -p mbv`).
- [ ] 2.2 Implement display-row grouping: reuse `feeds_model.rs` `FeedAgeGroup`/`feed_age_group` for podcast episode headings (`PodcastDisplayRow` headings + spacers omitted when a group is empty). Verify with a unit test mirroring `display_rows_insert_non_selectable_groups_without_changing_indices` on podcast fixtures, including undated episodes → `Unknown date` (`cargo nextest run -p mbv`).

## 3. PodcastContent owner rewrite

- [ ] 3.1 Rewrite `PodcastContent` to the Feeds-owner shape: one episode `MediaListCarrier`, combined `SelectorRow` (state pills `All`/`Unplayed`/`Played` then per-show pills, show titles truncated like feed-group labels), hero without Workspace, leaf activation. Remove episode-filter/hero-workspace/selection-modal logic. Verify `content()` projection tests: pills order and truncation, grouped rows, hero facts/credits, empty and loading list slots (`cargo nextest run -p mbv`).
- [ ] 3.2 Implement state-and-show pill selection: `SelectorPicked` branch (`index < 3` = state filter, otherwise show pill), mutually exclusive active pill, session-remembered pill surviving `set_content` and tab switches, reset to `All` on restart (owner construction). Verify with unit tests for each spec scenario (`cargo nextest run -p mbv`).
- [ ] 3.3 Re-aim activation: list-row Enter/overlay activation → `PodcastEpisodeIntent::OpenOrPlay(target)`, Ctrl+A → `Enqueue`, context menu unchanged; wire the `hero_overlay_available` leaf seam (first Enter opens the Library Hero overlay in non-Wide geometry). Verify with owner unit tests plus a real `Application::tick()` integration test in `tests_tick_integration_podcast.rs` through the shell sync pass (`cargo nextest run -p mbv`).
- [ ] 3.4 Hero projection: episode hero builder (episode title, description, duration, resume/finished fact, show name + author credits) over the existing Square shell with the parent show's cover fetch/cache. Verify hero projection unit tests (facts, credits, images-disabled budgeting, selection-change refresh) (`cargo nextest run -p mbv`).

## 4. Shell and rendering integration

- [ ] 4.1 Update shell sync/push paths and image projection for the flat list: expanded-page completion pushes, pill remembered across destination switches, stale-page rejection after Service replacement. Verify with tick-integration tests through the shell sync pass (`cargo nextest run -p mbv`).
- [ ] 4.2 Update keyboard routing: the podcast tab's `[`/`]` disposition and any compatibility/fall-through arms move into shared routing-matrix rows in `shell_routing_matrix_tests.rs`. Verify the matrix rows pass (`cargo nextest run -p mbv`).
- [ ] 4.3 Inline search over the flat list: search slot scoped to episode rows, placeholder claims and resolve assertions updated from show rows to episode rows. Verify with the inline-library-search scenarios (`cargo nextest run -p mbv`).
- [ ] 4.4 Paint coverage: prove the podcast Panel paints its complete placement (Selector row, grouped list, Wide hero without Workspace) and that one painter owns each surface in Wide and non-Wide geometry; update the buffer/pinning tests that referenced show rows or surname buckets. Verify with `cargo nextest run -p mbv` (`tests_tick_integration_podcast.rs`, panel paint tests).

## 5. Cleanup and gates

- [ ] 5.1 Delete dead podcast machinery: surname/title bucket use in the podcast tab, show-hero workspace code paths, season-position filter logic, and now-unfed shell message arms (every boundary-crossing request variant gets an exhaustive arm or a documented no-op). Verify `cargo clippy --workspace --all-targets -- -D warnings` and no `#[cfg_attr(not(test), allow(dead_code))]` gates left on deleted items.
- [ ] 5.2 Full gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv` and `-p mbv-core`, and `openspec validate --all` green; then sync deltas and archive the change.

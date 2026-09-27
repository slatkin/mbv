# Design

## Context

See proposal.md (Why). Today `handle_lib_event` routes 35 flat `LibEvent` variants to named `handle_<variant>` methods that already live next to their state (`event.rs`, `event/browse_loads.rs`, `event/audiobookshelf.rs`, `cursor.rs`, `state/music_artist_detail.rs`). The handlers stay as they are; only the type and the routing change.

Constraints from D5 of `2026-09-27-decompose-app-god-type` that this design keeps: exhaustive routing, no wildcard arms, no `unreachable!()`, no `Option<LibEvent>` passthroughs, intentionally ignored variants get explicit documented no-op arms. The pre-D5 family dispatchers failed these because they matched the *flat* enum and passed "not mine" through; typed family enums make each family match exhaustive by construction.

## Goals / Non-Goals

**Goals:** every library-event routing function under ~10 cyclomatic and under clippy's `too_many_lines`, with the approved `expect` deleted and no new suppression.

**Non-Goals:** changing handler bodies, payload fields, handler names, or shell drain structure (see proposal Non-goals).

## Decisions

### D1. Nest families in the type, not in the dispatcher

`LibEvent` gains one variant per family, each wrapping a family enum defined in `src/app/state/events.rs` next to `LibEvent`. Families group by the owning state/handler file:

| Top-level variant | Family enum variant | Old flat variant |
|---|---|---|
| `Browse(BrowseEvent)` | `Loaded` | `Loaded` |
| | `PageAppended` | `PageAppended` |
| | `Refreshed` | `Refreshed` |
| | `SearchItemsLoaded` | `SearchItemsLoaded` |
| | `AllItemsPrefetched` | `AllItemsPrefetched` |
| | `FeedHomeVideoAggregated` | `FeedHomeVideoAggregated` |
| | `NavigateTo` | `NavigateTo` |
| | `RestoreLibraryPosition` | `RestoreLibraryPosition` |
| `Music(MusicEvent)` | `AlbumIndexBuilt` | `AlbumIndexBuilt` |
| | `RecursiveAlbumActivated` | `RecursiveAlbumActivated` |
| | `AlbumArtistLevelFetched` | `AlbumArtistLevelFetched` |
| | `GroupWarmupListed` | `MusicGroupWarmupListed` |
| | `AlbumTracksFetched` | `AlbumTracksFetched` |
| | `ArtistTracksFetched` | `ArtistTracksFetched` |
| | `ArtistArtworkFetched` | `ArtistArtworkFetched` |
| `Series(SeriesEvent)` | `DetailFetched` | `SeriesDetailFetched` |
| | `SeasonEpisodesFetched` | `SeriesSeasonEpisodesFetched` |
| `Audiobookshelf(AudiobookshelfEvent)` | `DetailFetched` | `AudiobookshelfDetailFetched` |
| | `ShowsFetched` | `AudiobookshelfShowsFetched` |
| | `BooksFetched` | `AudiobookshelfBooksFetched` |
| | `ShelfFetched` | `AudiobookshelfShelfFetched` |
| | `BookDetailFetched` | `AudiobookshelfBookDetailFetched` |
| | `ProgressAcknowledged` | `AudiobookshelfProgressAcknowledged` |
| | `BookProgressAcknowledged` | `AudiobookshelfBookProgressAcknowledged` |
| `Playlist(PlaylistEvent)` | `ListLoaded` | `PlaylistsLoaded` |
| | `ListLoadError` | `PlaylistsLoadError` |
| | `ItemsLoaded` | `PlaylistItemsLoaded` |
| | `ItemsLoadError` | `PlaylistItemsLoadError` |
| | `Renamed` | `PlaylistRenamed` |
| | `Deleted` | `PlaylistDeleted` |
| `ModelContent(ModelContentEvent)` | `EmbyLatestSnapshotFetched` | `EmbyLatestSnapshotFetched` |
| | `HomeContentRefreshed` | `HomeContentRefreshed` |
| | `HomeContentCleared` | `HomeContentCleared` |
| `QueueEnriched { items }` | — | unchanged |
| `Error(String)` | — | unchanged |

Payload fields, field types, doc comments and the `#[rustfmt::skip]` on `QueueEnriched` move verbatim. Family variants drop the family word so clippy's `enum_variant_names` stays clean (every Audiobookshelf variant shared the prefix). All five family enums derive `Debug` like `LibEvent`, and are re-exported wherever `LibEvent` is (`src/app.rs` imports it from `state::events`).

`ModelContent` groups by *who applies it*, not by domain: all three variants are applied by the shell drain against Model-owned state, and the App's arm today is a no-op for each. One family means one truthful no-op arm on the App side.

`QueueEnriched` and `Error` stay top-level: `Error` is produced from ~28 sites across every family and handled uniformly; `QueueEnriched` is the queue's, not a library family's. A one-variant family would add a wrapper without removing an arm.

**Alternative rejected: keep `LibEvent` flat, split the match into family functions.** Any split over a flat enum needs either wildcards or a "not mine" return — the exact shape D5 deleted. **Alternative rejected: a `LibEvent::family()` classifier.** It is itself a 35-arm match; complexity moves, it doesn't drop. **Alternative rejected: `From<FamilyEvent> for LibEvent` impls.** Five impls to save typing at producers; explicit `LibEvent::Browse(BrowseEvent::Loaded { .. })` is greppable and costs nothing.

### D2. Dispatchers live next to their handlers

- `handle_lib_event` (`event.rs`): 8 arms — six `LibEvent::<Family>(ev) => self.handle_<family>_event(ev)`, `QueueEnriched`, `Error`. The `ModelContent` arm is `LibEvent::ModelContent(event) => drop(event)` with a comment that the shell drain applies Model-owned content (merging the three existing comments). The `#[expect(clippy::too_many_lines, …)]` is deleted.
- `handle_browse_event(&mut self, ev: BrowseEvent)` in `event/browse_loads.rs`.
- `handle_audiobookshelf_event(&mut self, ev: AudiobookshelfEvent)` in `event/audiobookshelf.rs`.
- `handle_music_event`, `handle_series_event`, `handle_playlist_event` in `event.rs`.

Each family dispatcher is one exhaustive, wildcard-free match that moves the corresponding arms of today's `handle_lib_event` verbatim (field destructuring and handler call unchanged). Visibility: family dispatchers are private to `dispatch::library::event` (`pub(super)` where they sit in a child module), since only `handle_lib_event` calls them.

### D3. Producers and shell patterns: mechanical rewrite

Every construction becomes `LibEvent::<Family>(<FamilyEvent>::<Variant> { .. })`; every pattern gains the same wrapper. `shell/run/drains.rs` keeps its arm order and bodies; `shell/inline_search.rs`'s `matches!` list and `NavigateTo` destructure, and `shell/run.rs`'s `RestoreLibraryPosition` extraction, update patterns only. The spec `playlist-management` names `LibEvent::PlaylistRenamed`/`PlaylistDeleted` as an implementation detail inside a scenario (already stale: it shows an `id` field the variant never had); behaviour is unchanged, so no delta — noted here instead.

## Risks / Trade-offs

- [Wide mechanical diff (~40 files) hides a behaviour edit] → tasks forbid body edits; the compiler proves every site was visited; review reads `event.rs`/`drains.rs` for arm-order parity.
- [Nested patterns in the drain get longer] → accepted; they remain explicit.
- [A future variant is added to the wrong family] → families map to owning handler files (D1 table); that rule goes in a doc comment on `LibEvent`.

## Migration Plan

Single commit per task group; no persistence or protocol involvement, so rollback is `git revert`.

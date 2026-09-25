# Design

## Context

`start_audiobookshelf_shelves` (`src/app/dispatch/session/service_startup.rs`) fetches one library's `/personalized` shelves on a worker thread and sends `LibEvent::AudiobookshelfShelfFetched { generation, library_id, result }`. It is called only from catalog startup (`src/app/dispatch/run_loop/drains.rs`). The handler (`src/app/dispatch/library/event/audiobookshelf.rs`) already drops results whose generation the runtime no longer accepts and, on `Ok`, overwrites `audiobookshelf_shelf_cache[library_id]`; `Err` leaves the cache untouched. `audiobookshelf_refresh` (`src/app/dispatch/audiobookshelf/browse.rs`), reached from the refresh key for a podcast tab, clears browse state and calls only `start_audiobookshelf_shows`.

`PodcastContent::set_content` keeps `PillSelection::Latest` across content pushes, and Latest acknowledgement lives in the shell's `acknowledged_home_latest_sources`, which refresh never touches.

## Goals / Non-Goals

**Goals:**
- Refresh re-requests the refreshed library's shelf with the current generation.
- Hermetic tests prove the request is issued and Latest survives it.

**Non-Goals:**
- Periodic or background shelf refresh; refetch on tab activation.
- Book-library refresh (book libraries have no shelf fetch).
- Changing the shelf handler, the cache shape, or the stale-generation guard.

## Decisions

- **D1 — Reuse the existing request and handler.** Add one `start_audiobookshelf_shelves(config, generation, library_id, lib_tx)` call in `audiobookshelf_refresh`, beside the existing `start_audiobookshelf_shows` call, using the same `library_id` and `generation` it already computes. No new event, state, or seam.
- **D2 — Do not clear the cache on refresh.** The old shelf stays visible until the replacement arrives, and a failed refetch keeps it (the handler already ignores `Err`). Clearing would flash an empty Latest and lose data on transient failure.
- **D3 — Test via the real worker's fast-fail path.** The default test config has no `audiobookshelf_setup`, so the spawned worker fails immediately with a Protocol error and sends `AudiobookshelfShelfFetched` without any network. The test receives from `app.lib_rx` with `recv_timeout` (the existing pattern in `library_navigate_reveal_worker.rs`) and asserts the library id and generation. Selection/acknowledgement survival is tested in the tick harness by delivering the completion event directly.

## Risks / Trade-offs

- A startup shelf response and a refresh response share a generation; if the startup one lands last it wins. Both are fresh reads of the same server state within seconds, so this is accepted rather than adding a per-request token.

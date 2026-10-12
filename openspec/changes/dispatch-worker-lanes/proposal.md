# Proposal

## Why

TUI dispatch starts a new detached OS thread for each remote request (about 50 sites in `src/app/dispatch/`, `src/app/state/`, `src/app/shell/` and `src/app/infra/image_fetch/fetch*`). Thread creation itself is cheap: about 10–30 µs, and stacks are committed lazily. The real costs are these:

- In-flight requests have no upper limit. Fast browsing or typing sends one HTTP request per step to the server at the same time. Thread count and server load rise and fall with input speed.
- Thread count is not steady, so `/proc` thread counts and RSS from thread stacks are hard to read when you diagnose memory.

Image fetches already have a limit (`MAX_IMAGE_FETCHES = 6`). The rest of dispatch has none.

This change does not drop superseded requests. Each site keeps its own completion bookkeeping (for example `album_tracks_loading`, and nav-stack levels with `loading: true` that only the matching result clears). A dropped job would leave that state stuck. See design.md D4.

## What Changes

- Add a small std-only worker pool to `src/app/infra/`. It has fixed, named lanes, each with a fixed worker count and a FIFO job queue.
- Lanes are split by remote, so a stalled server blocks only its own lane: `Emby`, `Audiobookshelf`, `Feeds` (arbitrary RSS hosts), and `Background` (warmups, capability registration, fire-and-forget progress reports, library refresh, desktop notifications).
- A job panic is caught and logged, and the worker keeps serving. Today a panic kills only its own detached thread, so this keeps the same isolation.
- Move the request-shaped spawn sites onto lanes. Long-lived threads and already-bounded paths keep their own threads (see design.md).

No user-visible behavior changes beyond timing. Every submitted job still runs and sends its result, as it does today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is an internal execution change with no spec-level behavior, so `.openspec.yaml` sets `skip_specs: true`.

## Impact

- New `src/app/infra/workers.rs` with `src/app/infra/workers/tests.rs`.
- `App` (or its channel bundle) owns one `Workers` value.
- Spawn sites in `src/app/dispatch/{library,feeds,audiobookshelf,queue,session,notify.rs,cast_status.rs}`, `src/app/state/music_artist_detail.rs`, `src/app/shell/{feeds_manage.rs,run/drains.rs}` and `src/app/infra/image_fetch/fetch*.rs` change from `std::thread::spawn` to a lane submit.
- No new dependencies.
- Not in scope: `crates/` (the daemon, player, ws and cast crates have their own long-lived threads), and the `run_with_hard_bound` helper in `mbv-net`, which keeps its internal thread.

# Tasks

## 1. Lane pool

- [ ] 1.1 Add `src/app/infra/workers.rs` with `Lane`, the per-lane worker-count constants, `Workers` and `Workers::submit` as in design D1: lazy start on the first submit, named threads `mbv-<lane>-<n>`, `catch_unwind` around each job with a `tracing::error!` that names the lane, and a logged retry when a worker fails to start. Register the module in `src/app/infra.rs`. Verify: `cargo check -p mbv` passes.
- [ ] 1.2 Add `src/app/infra/workers/tests.rs` with three tests, one contract each. (a) A lane runs at most its worker count of jobs at once: block jobs on a gate channel, receive N "started" signals, then assert that a "started" signal from job N+1 is not available yet. (b) A panicking job does not stop the lane: a job submitted after the panic still runs. (c) No worker thread starts before the first submit to that lane. Use no sleeps. Coordinate only with channels. Verify: `cargo nextest run -p mbv workers` passes.
- [ ] 1.3 Add a `Workers` field to `App` next to `channels`, built in `src/app/state/construct.rs`. Verify: `cargo nextest run -p mbv` passes. Commit group 1.

## 2. Emby lane: library and browse sites

- [ ] 2.1 Move the Emby sites in `dispatch/library/load.rs` (except 512), `dispatch/library/search.rs`, `dispatch/library/browse/{search,navigation,tv,loading}.rs` and `state/music_artist_detail.rs` from `std::thread::spawn` to `self.workers.submit(Lane::Emby, …)`. Keep each closure body unchanged. Verify: `cargo nextest run -p mbv` passes, and `rg "thread::spawn" src/app/dispatch/library src/app/state/music_artist_detail.rs` matches nothing.
- [ ] 2.2 Move `infra/image_fetch/fetch.rs:34,73,116`, `dispatch/feeds/feed.rs:196`, `dispatch/queue/playlist_mutation.rs`, `dispatch/session/command.rs` and `dispatch/session/service_startup.rs:392,438` to `Lane::Emby`. `playlist_mutation.rs` has free functions, so pass `&Workers` in from the caller. Verify: `cargo nextest run -p mbv` passes. Commit group 2.

## 3. Audiobookshelf and Feeds lanes

- [ ] 3.1 Move the Audiobookshelf sites from design D2 (`dispatch/audiobookshelf/browse.rs`, `dispatch/library/load.rs:512`, `dispatch/session/service_startup.rs:122,149,219,251,281,301,321`) to `Lane::Audiobookshelf`. Free functions in `service_startup.rs` take `&Workers` from their callers. Verify: `cargo nextest run -p mbv` passes.
- [ ] 3.2 Move `dispatch/feeds/feed_tab.rs:129`, `dispatch/feeds/feed.rs:435` and `shell/feeds_manage.rs:259` to `Lane::Feeds`. Verify: `cargo nextest run -p mbv` passes. Commit group 3.

## 4. Background lane and remaining threads

- [ ] 4.1 Move `dispatch/cast_status.rs:198,219`, `dispatch/notify.rs:37,53`, `dispatch/session/emby_service_completion.rs:80,205`, `shell/run/drains.rs:506` and `infra/image_fetch/fetch/level_warmup.rs:37,183` to `Lane::Background`. Verify: `cargo nextest run -p mbv` passes.
- [ ] 4.2 Add a one-line comment that says why at each site that keeps its own thread (design D2: `infra/signals.rs`, `state/types/cast.rs`, `dispatch/cast.rs` ×3, `infra/image_fetch/protocol.rs`). Verify: `rg -l "thread::spawn" src/app -g '!**/tests*'` lists only `signals.rs`, `state/types/cast.rs`, `dispatch/cast.rs`, `image_fetch/protocol.rs` and `infra/workers.rs`.
- [ ] 4.3 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` and verify that both are clean. Commit group 4.

## Workflow follow-up

- Archive this change after review.

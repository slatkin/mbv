# Design

## Context

See proposal.md (Why) for the motivation.

- Each dispatch site clones a sender (`self.channels.lib_tx`, `search_tx` and others), calls `std::thread::spawn`, runs one blocking client call, and sends the result. The threads are detached and never joined.
- Most `spawn_*` methods take `&self`, so a submit must work through a shared reference.
- The Emby HTTP agent uses a 5 s connect timeout and a 30 s global timeout (`crates/mbv-emby/src/client_auth.rs:39`). Audiobookshelf calls use `*_bounded` variants, which already run inside `mbv_net::bounded::run_with_hard_bound` with its own inner thread.
- `src/app/infra/image_fetch/protocol.rs:494` already has a limit (`MAX_IMAGE_FETCHES`) and catches panics (H9). It is the precedent for panic handling.
- App unit tests construct many `App` values. CI runs nextest with `--test-threads=4` because of an fd budget (`build.yml`). Eager worker threads per `App` would multiply threads in tests.

## Goals / Non-Goals

**Goals:**

- A fixed upper limit on concurrent dispatch requests per remote.
- A stalled server blocks only its own lane.
- No change to what any site sends, or when it sends relative to its own request.

**Non-Goals:**

- No cancellation or replacement of queued or running jobs (D4).
- No change to `crates/` threads, cast threads, signal threads or the image protocol path.
- No async runtime.

## Decisions

### D1. A std-only lane pool in `src/app/infra/workers.rs`

```rust
pub(in crate::app) enum Lane { Emby, Audiobookshelf, Feeds, Background }

pub(in crate::app) struct Workers { /* one LaneQueue per Lane */ }

impl Workers {
    pub(in crate::app) fn submit(&self, lane: Lane, job: impl FnOnce() + Send + 'static);
}
```

- Each lane has an `mpsc::Sender<Job>` and a receiver shared by its workers as `Arc<Mutex<mpsc::Receiver<Job>>>`. Several workers share one receiver, so they need the `Arc`. No owned-data alternative exists for a multi-consumer std channel. `Job` is `Box<dyn FnOnce() + Send>`.
- Fixed worker counts, as constants next to `Lane`: Emby 4, Audiobookshelf 2, Feeds 2, Background 2. Emby is the main browse Service and has the most sites. The other lanes see low traffic.
- Workers start **lazily**: the first `submit` to a lane starts that lane's workers. An `App` that never submits starts no threads, which keeps the test thread count as it is today.
- Workers are named threads (`mbv-<lane>-<n>`), so `top -H` and crash logs show the lane.
- Each job runs inside `catch_unwind(AssertUnwindSafe(job))`. A panic logs `tracing::error!` with the lane name, and the worker takes the next job. Today a panic kills only its own detached thread. This keeps that isolation and keeps the worker.
- If a worker thread fails to start (`thread::Builder::spawn` returns `Err`), log the error and keep the job in the queue. The next `submit` retries the start.
- `submit` takes `&self`. A send fails only after all workers have exited, which happens only after `Workers` is dropped. Log that case. It cannot happen while `App` is alive.
- `App` owns one `Workers` next to `channels`. When `App` drops, the senders drop, the workers finish their queued jobs and exit, and no one joins them. This matches today's detached threads.

Rejected alternative: add `rayon` or `threadpool`. `rayon` is in `Cargo.lock` only as a transitive dependency. A dependency for about 60 lines of std code goes against the project's minimal-dependency rule. `rayon`'s work-stealing is built for CPU work, not for blocking I/O.

Rejected alternative: one shared pool. A stalled Audiobookshelf or RSS host would hold workers and delay Emby browsing. Lanes per remote prevent that.

### D2. Lane assignment

| Lane | Sites |
|---|---|
| Emby | `state/music_artist_detail.rs:208`; `dispatch/feeds/feed.rs:196` (Emby-backed); `dispatch/queue/playlist_mutation.rs:235,259,283`; `dispatch/session/command.rs:33,113`; `dispatch/library/load.rs:165,182,204,234,455`; `dispatch/library/search.rs:10,93,132,222`; `dispatch/library/browse/search.rs:132,169`; `dispatch/library/browse/navigation.rs:331`; `dispatch/library/browse/tv.rs:227,285,337`; `dispatch/library/browse/loading.rs:208,315,376`; `infra/image_fetch/fetch.rs:34,73,116`; `dispatch/session/service_startup.rs:392,438` |
| Audiobookshelf | `dispatch/audiobookshelf/browse.rs:81,144`; `dispatch/library/load.rs:512`; `dispatch/session/service_startup.rs:122,149,219,251,281,301,321` |
| Feeds | `dispatch/feeds/feed_tab.rs:129`; `dispatch/feeds/feed.rs:435`; `shell/feeds_manage.rs:259` |
| Background | `dispatch/cast_status.rs:198,219`; `dispatch/notify.rs:37,53`; `dispatch/session/emby_service_completion.rs:80,205`; `shell/run/drains.rs:506`; `infra/image_fetch/fetch/level_warmup.rs:37,183` |

Line numbers are from commit `181febf5e`. Find each site by its enclosing function if the lines have moved.

`level_warmup` sends Emby requests, but it goes on Background. It is a large prefetch (`Limit=100000`), and on the Emby lane it would delay user-initiated browsing.

**Stays on its own thread** (each gets a one-line comment that says why):

- `infra/signals.rs:54`: a long-lived loop.
- `state/types/cast.rs:150`: a long-lived Cast transport.
- `dispatch/cast.rs:108,149,216`: Cast discovery (3 s by design), item resolution and connect. These are rare, user-initiated and long-blocking. They would hold a lane worker for seconds.
- `infra/image_fetch/protocol.rs:494`: already limited by `MAX_IMAGE_FETCHES` and already catches panics.

### D3. A lane is chosen at the call site

Each site names its lane in its `submit` call. Lane choice depends on which remote the job talks to, and only the call site knows that. A lookup table elsewhere would duplicate the table above in code.

### D4. Rejected: latest-wins replacement of queued jobs

Replacing a queued job with a newer one would cut wasted requests during fast browsing. But sites depend on a completion for every request:

- `fetch_album_tracks` inserts into `album_tracks_loading` and only the result removes the entry. A dropped job blocks that album's fetch for good.
- `spawn_browse` results clear a nav-stack level with `loading: true` through `handle_loaded_level`.

Safe replacement would need a "dropped" completion event per site. That is a larger change with its own risk. Leave it out. Record it as a possible follow-up if burst traffic shows up in practice.

## Risks / Trade-offs

- [A server that accepts connections but then stalls holds lane workers for up to the 30 s global timeout. Later jobs on that lane queue behind them, so a burst can wait several timeout periods. Today each request times out on its own after 30 s] → The connect timeout (5 s) covers a dead host. The long wait only happens on a half-alive server, and only on that server's lane. If it shows up in practice, lower the Emby global timeout for browse calls. Do not add more workers.
- [Queueing changes completion order between jobs on one lane] → Completion order across concurrent detached threads was never guaranteed, so no handler can rely on it. With FIFO per lane, ordering only gets more regular.
- [Hidden blocking inside a job (for example `run_with_hard_bound`'s inner thread) still creates threads] → Accepted. The pool limits the outer request threads. Audiobookshelf's inner bounded thread is out of scope.
- [A job that never returns holds a worker forever] → Every remote call in these sites has an agent timeout or a hard bound. Cast's long-blocking calls stay off the lanes (D2).

## Migration Plan

Move sites lane by lane. Each group is a separate commit that compiles and passes tests. Rollback is a revert of each group. No persisted state or protocol changes.

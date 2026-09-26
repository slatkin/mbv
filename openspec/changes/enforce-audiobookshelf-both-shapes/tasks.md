# Tasks

## 1. Pin the wire format before touching the type

- [ ] 1.1 In `crates/mbv-core/src/playback/tests/persistence.rs`, add golden tests: one Audiobookshelf episode and one book `QueueItem` serialize to exact JSON strings (capture them from the current derive), and those strings deserialize back to the same shape. Verify: `cargo nextest run -p mbv-core persistence` passes on unchanged code.

## 2. Nest the shapes (source-of-truth type)

- [ ] 2.1 In `crates/mbv-core/src/playback/queue/items.rs`, add `pub enum AudiobookshelfItem { Episode(AudiobookshelfQueueItem), Book(AudiobookshelfBookQueueItem) }`, replace the `Audiobookshelf` and `AudiobookshelfBook` variants with `Audiobookshelf(AudiobookshelfItem)`, move the shape-shared accessors onto `AudiobookshelfItem` and delegate to them from `QueueItem`, keep `QueueItemKind` flat and derive it in `kind()`, and make `is_audiobookshelf()` cover both shapes while deleting `is_audiobookshelf_any()` / `is_audiobookshelf_book()`. Verify: `items.rs` compiles in isolation (other crates may still fail).
- [ ] 2.2 Replace the derived `Serialize` with a hand-written one that emits the unchanged flat `kind` tags, and update `Deserialize` to build the nested shapes. Verify: the task 1.1 golden tests pass unchanged.
- [ ] 2.3 Replace `mpv_url_for_queue_item` in `crates/mbv-core/src/player/run/queue.rs` with a `MpvUrlSource` input built from `QueueItem::mpv_url_source()` (returns `None` for Audiobookshelf), remove both `unreachable!()` arms, and have callers reject `None` through the existing `CommandRejected`/log path. Add a unit test showing a book yields `None`. Verify: `rg 'unreachable!' crates/mbv-core/src/player/run/queue.rs` finds none, and the test passes.

## 3. Fix call sites (compiler-driven)

- [ ] 3.1 Fix every `mbv-core` player/, cast/ and remote_player/ compile error. Rewrite `is_audiobookshelf_any()` as `is_audiobookshelf()`, and rewrite paired `Audiobookshelf(_) | AudiobookshelfBook(_)` arms as `Audiobookshelf(_)`. Shape-specific sites (`sources.rs`, `reporting.rs`, `decisions.rs`, `player.rs`, `types.rs`, `proxy.rs` capability gates, `cast/dispatch.rs`) match `AudiobookshelfItem::Episode` or `Book` explicitly. Verify: no errors in those modules under `cargo check -p mbv-core`.
- [ ] 3.2 Fix the remaining `mbv-core` errors (daemon/, config/, ctrl, playback/) the same way. `daemon/control_queue.rs` keeps its two separate capability gates by matching the nested shapes. Verify: `cargo check -p mbv-core` and `cargo nextest run -p mbv-core` pass.
- [ ] 3.3 Fix the `src/app` and `crates/mbvd` errors (`dispatch/audiobookshelf/*`, `context_menu_capabilities.rs`, tests). Verify: `cargo check --workspace --all-targets` and `cargo nextest run --workspace` pass.
- [ ] 3.4 Check that no deleted predicate is left anywhere. Verify: `rg 'is_audiobookshelf_any|is_audiobookshelf_book|QueueItem::AudiobookshelfBook'` returns nothing, and `cargo clippy --workspace --all-targets -- -D warnings` plus `cargo fmt --all -- --check` are clean.

## 4. Retire the invariant

- [ ] 4.1 Delete `docs/invariants/04-audiobookshelf-means-both-shapes.md` and remove it from any index in `docs/invariants/`. Update the `CONTEXT.md` **QueueItem** entry to name `AudiobookshelfItem` (Episode | Book). Verify: `rg -n "04-audiobookshelf" docs CONTEXT.md AGENTS.md` returns nothing.
- [ ] 4.2 Run `make check-code-file-lines` and split `items.rs` along the `AudiobookshelfItem` seam if it exceeds 800 lines. Tick 04 on #810 and comment on #806 with the commit. Verify: the check passes and the issue comments are posted.

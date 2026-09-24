# Design

## Context

See proposal.md for why this change exists. Current wiring, verified 2026-09-24:

- `config.rs` is 33 lines: 14 `include!`s plus `pub mod tests { include! ×10 }`, all gated on `cfg(any(test, feature = "test-support"))`. The app reaches `mbv_core::config::tests::*` helpers through that feature, so the public path `config::tests` must survive.
- `daemon.rs` is 21 lines: 8 `include!`s plus `mod tests { include! ×9 }`. `daemon_control.rs:1` includes `daemon_control_queue.rs`, and `daemon_core.rs:622` includes `daemon_core_ctrl_spawn.rs`.
- `api.rs` is 15 lines: 6 `include!`s plus 3 test `include!`s. `api_types.rs:553` pulls in `api_types_parsing.rs` via `#[path]`.
- `audiobookshelf.rs` wires `audiobookshelf_catalog.rs` and `audiobookshelf_playback.rs` (plus their tests) via `#[path]`. `audiobookshelf_socket.rs` is a separate `pub mod` at the crate root.
- `ctrl.rs:801` and `keybinds.rs:1030` each `include!` one test file.
- `player/mod.rs:274-295` `include!`s 7 sibling files and 7 test files. `player/run/mod.rs` `include!`s 6 files. `playback/queue.rs:11` includes `queue_items.rs`. `playback/tests/playback_queue_tests.rs` includes 3 more.
- `remote_player/connect` (already `connect/mod.rs` after `tidy-repo-layout`) includes `tests.rs`. `crates/mbvd/src/main.rs:532` includes `tests.rs`.
- `lib.rs` already has a compatibility-module precedent (`player_owner_state`), which this change does not copy.

## Goals / Non-Goals

**Goals:** make the filesystem mirror the module tree; get rid of `include!()` and `#[path]` in `mbv-core`/`mbvd`; give each family a directory with its tests beside it; keep `mbv_core::{api, config, daemon, player, playback, ctrl, keybinds}::X` paths unchanged.

**Non-Goals:**
- Splitting files that are over the 800-line cap (`daemon_tests_queue_ops.rs`, `daemon_tests.rs`, `daemon_control.rs`, `keybinds.rs`, `ctrl.rs`). A pure move doesn't change their size, and the pre-PR cap check handles them separately. Do not split during this change.
- Renaming any type, function or domain term.
- Changing `examples/`.

## Decisions

1. **Target tree.** Drop the family prefix, keep the rest of each file name, and use the `mod.rs` style (see `tidy-repo-layout`, Decision 3).
   ```
   api/            mod.rs types.rs types_parsing.rs client_{auth,library,playlists,reporting,sessions}.rs
                   tests/{mod,parsing,client,failure}.rs
   audiobookshelf/ mod.rs catalog.rs catalog_books.rs playback.rs socket.rs
                   tests/{mod,catalog,playback}.rs
   cast/           mod.rs client.rs discovery.rs dispatch.rs
   config/         mod.rs types_{paths,setup,queue_state,feed}.rs launch_state.rs test_support.rs paths.rs
                   state.rs credentials.rs emby_admin.rs parse.rs save.rs {audiobookshelf,emby}_lifecycle.rs
                   tests/{mod,settings,keybinds,library,paths,paths_env,credentials,paths_migration,
                          script_source,emby_admin,launch_state}.rs
   ctrl/           mod.rs tests.rs
   daemon/         mod.rs context.rs core.rs core_ctrl_spawn.rs run.rs run_shutdown.rs audiobookshelf.rs
                   control.rs control_queue.rs ws.rs reconciliation.rs ctrl.rs <event-loop file>
                   tests/{mod,basic(ex daemon_tests),audio_only,ctrl_auth,playback_intent,feed,
                          service_independent,abs_queue,abs_queue_progress,queue_ops}.rs
   keybinds/       mod.rs tests.rs
   player/ player/run/ playback/   (existing dirs; include! → mod only)
   ```
   - Files that stay flat at the root: `applog`, `bounded`, `feed_entry_state`, `id_types`, `mock_http`, `service_runtime`, `stream`, `ws`.
   - `daemon_tests.rs` becomes `tests/basic.rs`, because `tests/tests.rs` would be meaningless.
   - The event-loop file from `split-daemon-event-loop` (`daemon_loop.rs`) becomes `daemon/event_loop.rs`, because `loop` is a reserved word.
2. **Two phases per family: move first, then convert.**
   - Phase A is a pure `git mv` with `include!`/`#[path]` strings updated. It has zero semantic change and is trivial to review.
   - Phase B turns `include!` into `mod`.
   - Doing both at once would mix a large rename diff with visibility edits and hide mistakes.
3. **Conversion pattern (Phase B).**
   - In the parent: `mod child;` plus `pub use child::*;`. Use `pub(crate) use` when the child only holds crate-private items. The glob keeps every existing `module::Item` path working, so callers outside the family don't change.
   - Items that siblings reach through the old shared namespace become `pub(super)` (within the family) or `pub(crate)`, only where `cargo check` reports an error. Don't widen anything preemptively.
   - Tests: `#[cfg(test)] mod tests;` in the parent. `tests/mod.rs` does `use super::*;` and declares one `mod` per file, and each test file starts with `use super::*;`.
   - `config::tests` keeps its `cfg(any(test, feature = "test-support"))` gate and stays `pub`.
4. **Update callers; don't alias.** The only paths that change are `cast_*`, `audiobookshelf_socket` (public, about 20 caller files) and the crate-private `daemon_ctrl`. This is a workspace-internal crate, so a compatibility module like `player_owner_state` would just be dead weight.
5. **`daemon_ctrl` → `daemon/ctrl.rs`.** If declaring `mod ctrl` inside `daemon` collides with an existing `use crate::ctrl` in a daemon file, name the file `daemon/ctrl_link.rs` instead and record the choice in tasks.md. Don't spend time on the naming beyond that.
6. **Each family is one task and one commit,** so a failure bisects to one family and a reviewer reads one directory at a time.

## Risks / Trade-offs

- [Glob re-exports can create ambiguous-name errors where two siblings define the same private helper name.] The compiler reports every one. Rename nothing: make one of the helpers module-private (drop it from the glob with an explicit `pub use child::{A, B}` list) instead.
- [Visibility widening could leak internals across the crate.] Prefer `pub(super)` over `pub(crate)`, and never make something `pub` that wasn't already.
- [Merge conflicts with other in-flight `mbv-core` changes.] See the sequencing rule in the proposal. Check `openspec list` before starting.
- [Doc paths go stale.] Task 9 updates the file-path citations in docs and specs.

## Migration Plan

Land family by family on `main` (commit-per-family). Roll back any family with `git revert` on its commit. No runtime migration is involved.

# Tasks

Every task: pure moves plus the compile-forced edits only, with no renames of types, functions or domain terms. Gate each task with `cargo check --workspace --all-targets` and `cargo nextest run -p mbv-core` (plus `-p mbv` where callers change), then `cargo fmt`, then commit. Target paths are in design.md, Decision 1. Use `git mv` so history follows the files.

## 0. Preconditions

- [x] 0.1 Confirm `split-daemon-event-loop` and `tidy-repo-layout` are archived, and that `openspec list` shows no other active change editing `crates/mbv-core/src/`. Verify: `ls openspec/changes` shows neither of those two change directories outside `archive/`.

## 1. `api/`

- [x] 1.1 Phase A: `git mv` `api.rs` → `api/mod.rs`, `api_types.rs` → `api/types.rs`, `api_types_parsing.rs` → `api/types_parsing.rs`, each `api_client_*.rs` → `api/client_*.rs`, and `api_tests_parsing.rs`, `api_tests_client.rs`, `api_failure_tests.rs` → `api/tests/{parsing,client,failure}.rs`. Update the `include!`/`#[path]` strings. Verify: gate green, and `ls crates/mbv-core/src/api_*` matches nothing.
- [x] 1.2 Phase B: replace every `include!`/`#[path]` in `api/` with `mod` + glob re-export (design Decision 3). Create `api/tests/mod.rs`. Verify: gate green, and `rg -n 'include!|#\[path' crates/mbv-core/src/api` is empty.

## 2. `config/`

- [x] 2.1 Phase A: `git mv` `config.rs` → `config/mod.rs`, the 14 included `config_*.rs` → `config/<name without prefix>.rs` (`config_test_support.rs` → `config/test_support.rs`), and the 10 `config_tests_*.rs` → `config/tests/<suffix>.rs`. Update the `include!` strings. Verify: gate green, and `ls crates/mbv-core/src/config_*` matches nothing.
- [x] 2.2 Phase B: convert to `mod`s. `config::tests` stays `pub` under `cfg(any(test, feature = "test-support"))`. Also check the "Included via include!" header comments in `config_state.rs`, `config_paths.rs`, `config_credentials.rs` and `config_emby_admin.rs`: they are now false, so delete them. Verify: gate green, `cargo nextest run -p mbv` green (the app uses `config::tests`), and `rg -n 'include!|#\[path|Included via' crates/mbv-core/src/config` is empty.

## 3. `daemon/`

- [x] 3.1 Phase A: `git mv` `daemon.rs` → `daemon/mod.rs`, each `daemon_<x>.rs` → `daemon/<x>.rs` (including `daemon_ctrl.rs` → `daemon/ctrl.rs`, and `daemon_loop.rs` → `daemon/event_loop.rs` if it exists), `daemon_tests.rs` → `daemon/tests/basic.rs`, and `daemon_tests_<x>.rs` → `daemon/tests/<x>.rs`. Change `lib.rs` `pub(crate) mod daemon_ctrl` to a declaration inside `daemon/mod.rs`, and update `crate::daemon_ctrl::` paths. Verify: gate green, and `ls crates/mbv-core/src/daemon_*` matches nothing.
- [x] 3.2 Phase B: convert `daemon/mod.rs`, `daemon/control.rs` (which includes `control_queue.rs`) and `daemon/core.rs` (which includes `core_ctrl_spawn.rs`) to `mod`s. If `mod ctrl` collides, apply design Decision 5 and note the chosen name here. Verify: gate green, and `rg -n 'include!|#\[path' crates/mbv-core/src/daemon` is empty. — No `mod ctrl` collision occurred. Deviation from the literal rg-empty check: `#[path = "core_ctrl_spawn.rs"]` (in core.rs) and `#[path = "control_queue.rs"]` (in control.rs) remain, plus `#[path = "loop.rs"] mod r#loop;` (in tests/mod.rs, reserved word). These are accepted, unavoidable per Decision 1's flat-file mandate: a non-`mod.rs` file's submodule resolves to a subdirectory unless `#[path]` overrides it, and Decision 1 forbids new subdirectories other than `tests/`. Bare `include!` is fully removed.

## 4. `audiobookshelf/` and `cast/`

- [x] 4.1 `git mv` `audiobookshelf.rs` → `audiobookshelf/mod.rs`, `audiobookshelf_{catalog,catalog_books,playback,socket}.rs` → `audiobookshelf/{…}.rs`, and the two `*_tests.rs` → `audiobookshelf/tests/{catalog,playback}.rs`. Replace the `#[path]`s with `mods`. Remove `pub mod audiobookshelf_socket` from `lib.rs`, declare `pub mod socket` in `audiobookshelf/mod.rs`, and update callers (`rg -l audiobookshelf_socket src crates`). Verify: gate green plus `cargo nextest run -p mbv`, and `rg -n 'audiobookshelf_socket|#\[path' crates src/app --glob '*.rs'` shows no `mbv-core` hits. — One `#[path = "catalog_books.rs"]` remains in `catalog.rs` (declaring a flat sibling from a non-mod.rs family member), accepted as the same unavoidable Decision 1 flat-file exception used in daemon/ (3.2).
- [x] 4.2 `git mv` `cast_{client,discovery,dispatch}.rs` → `cast/{client,discovery,dispatch}.rs`. Add `cast/mod.rs` with three `pub mod`s, replace the three `lib.rs` lines with `pub mod cast;`, and update callers (`rg -l 'cast_(client|discovery|dispatch)' src crates`). Verify: gate green plus `cargo nextest run -p mbv`, and that `rg` returns only non-path string hits, if any.

## 5. `ctrl/` and `keybinds/`

- [x] 5.1 `git mv` `ctrl.rs` → `ctrl/mod.rs`, `ctrl_tests.rs` → `ctrl/tests.rs`, `keybinds.rs` → `keybinds/mod.rs`, and `keybinds_tests.rs` → `keybinds/tests.rs`. Replace each `mod tests { include!(…) }` with `#[cfg(test)] mod tests;`, moving the block's `use` lines into `tests.rs`. Verify: gate green, and `rg -n 'include!' crates/mbv-core/src/{ctrl,keybinds}` is empty.

## 6. `player/`, `player/run/`, `playback/`

- [x] 6.1 Convert `player/run/mod.rs`'s 6 `include!`s to `mod` + glob re-export. Verify: gate green, and `rg -n 'include!' crates/mbv-core/src/player/run` is empty.
- [x] 6.2 Convert `player/mod.rs`'s 7 production `include!`s (including `run/mod.rs`, which becomes `mod run;`) and its test block (7 includes) into `mod`s plus `player/tests/mod.rs`. Verify: gate green, and `rg -n 'include!' crates/mbv-core/src/player` is empty.
- [x] 6.3 Convert `playback/queue.rs`'s `include!("queue_items.rs")` and the `#[path = "tests/playback_queue_tests.rs"]` chain (3 nested includes) into `mod`s under `playback/tests/`. Verify: gate green, and `rg -n 'include!|#\[path' crates/mbv-core/src/playback` is empty.

## 7. `remote_player` and `mbvd`

- [x] 7.1 Replace `include!("tests.rs")` in `remote_player/connect/mod.rs` and in `crates/mbvd/src/main.rs:532` with `#[cfg(test)] mod tests;`. Verify: `cargo check --workspace --all-targets` and `cargo nextest run -p mbvd -p mbv-core` pass, and `rg -n 'include!\(|#\[path' crates` is empty. — remote_player/tests.rs moved to connect/tests.rs (it was included from connect/, so the file follows its declaring module; no #[path] needed). The row's literal #[path]-empty gate becomes true via the follow-up user-directed elimination unit (the four pre-existing daemon/audiobookshelf #[path]s were removed there; nesting flattened to siblings of their parents).

## 8. Final gates

- [ ] 8.1 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace` all pass. `ls crates/mbv-core/src` shows only `lib.rs`, the 8 flat files from design Decision 1, and directories.

## 9. Docs

- [ ] 9.1 Update `mbv-core` file-path citations to the new paths: `rg -n 'crates/mbv-core/src/(api|config|daemon|audiobookshelf|cast|ctrl|keybinds)[_a-z]*\.rs' AGENTS.md CONTEXT.md docs openspec/specs .agents openspec/changes --glob '!openspec/changes/archive/**'`. Don't edit archived changes. Verify: that `rg` returns only paths that exist (`test -e` each hit), then commit.

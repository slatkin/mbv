# Proposal

## Why

`crates/mbv-core/src/` is 80 flat files. Filename prefixes stand in for module directories: `config_*` has 25 files, `daemon_*` 21, `api_*` 9, `audiobookshelf_*` 7, `cast_*` 3. About 75 `include!()` calls stitch these families into single namespaces (`config.rs`, `daemon.rs`, `api.rs`, `player/mod.rs`, `player/run/mod.rs`, `ctrl.rs`, `keybinds.rs`, `playback/queue.rs`). As a result:

- the filesystem doesn't match the module tree,
- no file has a visibility boundary, since every included item shares one namespace, and
- rust-analyzer and the compiler report errors against the including file.

Two independent layout audits (2026-09-24) flagged this.

## What Changes

- Move each prefix family into a directory module and drop the prefix from the file names:
  - `api/`, `audiobookshelf/`, `cast/`, `config/`, `ctrl/`, `daemon/` and `keybinds/` are created.
  - `player/`, `player/run/` and `playback/` already exist and only get the conversion below.
- Move each family's tests into `<module>/tests/` (or `<module>/tests.rs` for a single file).
- Replace every `include!()` in `mbv-core` and `mbvd` with real `mod` declarations. The parent re-exports each child with a glob, so existing `mbv_core::config::X`-style paths still resolve. Items used across sibling files are widened to `pub(super)` or `pub(crate)` where the compiler requires it.
- **BREAKING (workspace-internal only):**
  - `mbv_core::cast_client`, `cast_discovery` and `cast_dispatch` become `mbv_core::cast::{client, discovery, dispatch}`.
  - `mbv_core::audiobookshelf_socket` becomes `mbv_core::audiobookshelf::socket`.
  - The crate-private `daemon_ctrl` moves under `daemon/`.
  - Callers in `src/` and `crates/mbvd` are updated in the same change. No compatibility aliases are added.

No behaviour, protocol, persistence or config-format change.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is a pure refactor, so `skip_specs: true`.

## Impact

- Every file under `crates/mbv-core/src/` except the ones that stay flat: `applog`, `bounded`, `feed_entry_state`, `id_types`, `mock_http`, `service_runtime`, `stream`, `ws`, `lib`.
- `crates/mbvd/src/main.rs` (`include!("tests.rs")`).
- About 20 caller files in `src/` and `mbv-core` that name the moved `cast_*` or `audiobookshelf_socket` paths.
- `CONTEXT.md`, `AGENTS.md`, `docs/` and `openspec/specs/` where they cite moved `mbv-core` file paths.
- **Sequencing:**
  - Start after `split-daemon-event-loop` is archived, because it rewrites `daemon_run.rs` and adds `daemon_loop.rs`.
  - Start after `tidy-repo-layout`, which fixes the `remote_player/connect` module style first.
  - Don't run it alongside any other change that edits `crates/mbv-core/src/`.
- Umbrella: sibling of `tidy-repo-layout` and `modularize-app-layout`.

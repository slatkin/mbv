# Proposal

## Why

`src/app/` has 234 `.rs` files and about 85k lines in one directory. Filename prefixes stand in for modules: roughly 25 `shell_*`, 35 `*_actions`, 17 `types_*`, 8 `input_*`, and about 95 `tests_*`/`*_tests` files.

- **`src/app/mod.rs` is not a thin module root.** Its 445 lines hold 97 module declarations plus:
  - process statics (`QUIT_REQUESTED`, `TERMINAL_GONE`) and 8 `cfg(test)` override statics,
  - signal handlers and the quit watchdog,
  - `init_terminal`/`restore_terminal`, `open_url`, an `impl App` block and the breakpoint constants.
- **Tests are wired through indirection.** `mod.rs` `include!`s `app_test_modules.rs`, which declares about 70 test modules via `#[path]`. Another roughly 100 `#[path]` attributes and 8 `include!`s are spread across `src/app`, `components/` and `render/`. One of them (`#[path = "shell_library_panel.rs"]`) just repeats the default path.
- **Nothing groups the modules.** Shell code, dispatch arms, state types, input policy, infrastructure and tests all share one flat namespace, so none of them can have a visibility boundary.

Two independent layout audits (2026-09-24) agreed on this diagnosis and on the target folders.

## What Changes

- Split `src/app/*.rs` into five folders, as a by-layer first pass:
  - `shell/`: the TuiRealm `Model` and its per-destination/overlay projections.
  - `dispatch/`: `Action` handling, `*_actions`, run-loop, session and service effects.
  - `state/`: `App`, the ex-`types_*` files and shell-owned state.
  - `input/`: router, key policy, chord resolver and key tables.
  - `infra/`: terminal, signals, images, visualizer, feed parsing and shared utilities.
- Drop only the redundant family prefix or suffix from moved files (`shell_queue.rs` → `shell/queue.rs`, `queue_actions.rs` → `dispatch/queue/queue.rs`). No type, function or domain-term renames.
- Shrink `src/app/mod.rs` to module declarations and re-exports:
  - Terminal and signal code moves to `infra/`.
  - The test seams move to a `cfg(test)` module.
  - The breakpoint constants move to `layout`.
  - `spawn_search_sidebar_query` moves to its dispatch file.
- Replace `app_test_modules.rs` and every `#[path]`/`include!` under `src/app` with real module files:
  - A test that belongs to one module becomes `<module>/tests.rs` (the module becomes a directory).
  - Cross-cutting App tests go in `app/tests/`, grouped as `tick_integration/`, `routing_matrix/`, `library_position/`, `queue/`, `feeds/`, `podcast/` and `route_state/`.
- Keep the paths that code outside `app` uses stable through `pub(crate) use` re-exports: `crate::app::{App, Model, components, render, images, layout, palette, ui_util, SidebarId, capture_launch_window, current_launch_secs, set_mouse_capture}`.
- Update `AGENTS.md`, the `mbv-frontend` skill, `docs/architecture/` and `openspec/specs/` where they cite moved paths.

No behaviour change and no change to the interactive architecture. Router, key policy and component ownership are exactly as before.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is a pure refactor, so `skip_specs: true`.

## Impact

- All of `src/app/*.rs`, plus the test wiring in `src/app/components/` and `src/app/render/` (their production layout otherwise stays as it is).
- `AGENTS.md` (repository map, router/key-policy/tick-test paths), `.agents/skills/mbv-frontend/SKILL.md`, `docs/architecture/interactive-surface-ledger.md`, `docs/invariants/`, and `openspec/specs/**` path citations.
- **Sequencing:**
  - Start after `tidy-repo-layout`.
  - Preferably start after `modularize-mbv-core-layout`, which rewrites some `mbv_core::cast_*` paths in `src/app`.
  - Start only when no other active change edits `src/app/`. Today `gate-all-queue-replacements`, `coverage-quick-wins` and `three-line-flat-list` would conflict with the moves.
- Umbrella #772: sibling of `tidy-repo-layout` and `modularize-mbv-core-layout`.

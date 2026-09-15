## Context

See `proposal.md`. 29 flat `player_*`/`playback_*` files in `mbv-core/src/`.
All are `pub mod` or `pub(crate) mod` declarations in `lib.rs`. Callers
throughout `mbv-core` and `src/app/` import via `crate::player::*`,
`crate::playback_queue::*`, etc.

## Goals / Non-Goals

**Goals:**
- Group related files into directory modules for browsability.
- Split `player_runtime_controller.rs` at the controller/submit seam.
- Preserve all public re-exports so callers need zero path changes.

**Non-Goals:**
- Logic refactoring, API changes, or new abstractions.
- Surveying other flat-file clusters (`config_*`, `daemon_*`) — separate pass.
- Changing visibility (`pub` / `pub(crate)`) of any item.

## Decisions

### D1: Directory modules with re-exporting `mod.rs`

Each directory's `mod.rs` declares submodules and re-exports public items at
the same path callers already use. For example, `lib.rs` keeps
`pub mod player;` and `player/mod.rs` does `pub use types::*;` etc., so
`crate::player::PlayerCommand` still resolves.

*Alternative:* Rust 2018 `player.rs` + `player/` layout. Works identically but
the convention in this repo already uses `mod.rs` elsewhere.

### D2: `player_runtime_controller.rs` split seam

The file has two concerns:

1. **Controller** — `Player` struct definition, `new()`, builder methods
   (`with_video_cache`, `with_audio_device`), credential/runtime updates,
   `send_command`, convenience wrappers (`next`, `previous`, `set_paused`,
   `stop`, `stop_for_shutdown`), `set_initial_queue`, `headless_for`,
   `QuitHandle`, `WakeupWriter`, `make_wakeup_pipe`.

2. **Submit** — `submit_queue` (the ~290-line cold-start/hot-path method),
   `play`, `play_queue`, `queue_append`, `assign_slot_ids`.

Both are `impl Player` blocks. The split is at the blank line after
`headless_for` (~line 325). `submit.rs` gets its own `impl Player` block
importing what it needs from `controller.rs` via `super::`.

### D3: `run/` submodule inside `player/`

The `player_run_*.rs` files form their own cohesive cluster (the mpv event
loop internals). Nesting them as `player/run/` with `mod.rs` declaring the
`PlaybackRun` struct makes the two-level grouping explicit:
`player/` = the subsystem, `player/run/` = the hot loop.

### D4: Test files in `tests/` subdirectories

`player_tests_*.rs` → `player/tests/`. `playback_queue_tests*.rs` →
`playback/tests/`. Each is a `#[cfg(test)] mod` declared from the parent
`mod.rs`. Keeps test files visually separate from production code.

### D5: `git mv` for blame preservation

Every file move uses `git mv` so `git log --follow` and `git blame` track
through the rename. The controller split is the only file that needs a
copy-then-edit rather than a pure move.

## Risks / Trade-offs

- **Merge conflicts with in-flight changes.** Three active changes touch
  player files (`right-size-tui-presentation-tests`,
  `unify-semantic-input-arbitration`). → Mitigation: land this before or
  after those, not concurrently. Pure renames rebase cleanly with
  `git rebase -X rename-threshold=50%`.
- **Path churn in imports.** Internal `use crate::player_*` paths need
  updating. → Mitigation: re-exports in `mod.rs` mean only `lib.rs` module
  declarations and intra-crate `use` paths change; the public API surface
  is unchanged.

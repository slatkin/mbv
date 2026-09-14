## Why

The `mbv-core/src/` directory has 29 `player_*.rs` and `playback_*.rs` files as
flat siblings alongside unrelated modules (`api.rs`, `config_*.rs`, `ctrl.rs`,
`daemon_*.rs`). This makes the mpv integration hard to browse and isolate
visually. `player_runtime_controller.rs` (706 lines) combines Player
construction/lifecycle with queue submission — two distinct concerns that should
be separate files. Pure tech-debt cleanup with no product impact.

## What Changes

- Move all `player_*.rs` files into a `player/` directory module.
- Move all `playback_*.rs` files into a `playback/` directory module.
- Move all `remote_player*.rs` files into a `remote_player/` directory module.
- Split `player_runtime_controller.rs` into `controller.rs` (Player struct,
  construction, lifecycle, command forwarding, ~330 lines) and `submit.rs`
  (submit_queue, play, play_queue, queue_append, ~370 lines).
- Nest `player_run_*.rs` files into a `player/run/` submodule.
- Group all `player_tests_*.rs` and `playback_queue_tests*.rs` into `tests/`
  subdirectories within their respective modules.
- Rewire `mod` declarations in `lib.rs`; re-export public items so callers
  are unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a pure structural refactor — no behavior changes. `skip_specs: true`
is set in `.openspec.yaml`.

## Impact

- Affected code: `crates/mbv-core/src/lib.rs` module declarations, all internal
  `use` paths referencing the moved modules. No public API change — re-exports
  preserve the current `mbv_core::player::*` / `mbv_core::playback_queue::*`
  surface.
- No dependency, protocol, or runtime behavior changes.
- Git history: `git mv` preserves blame for each file.

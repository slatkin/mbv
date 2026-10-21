# Proposal

## Why

Issue #956 removed the `_` arms from the `Command` sub-dispatchers by giving each one a typed
sub-enum. `Command` stayed flat, so `App::dispatch` now holds a one-arm-per-variant translation
match: 27 arms, cyclomatic 28, over the 25 commit cap. A wildcard-free match over a flat enum
with more than 24 variants always breaks the cap. The translation also holds two
`unreachable!` arms for commands that `Model` handles before it delegates. The structure is the
smell: `Command` is a flat list that the code re-groups by domain at every use.

## What Changes

- `Command` becomes a two-level enum. `Command::Shell(ShellCommand)` holds the commands only
  `Model` handles (`OpenHelp`, `RefreshCurrentView`). `Command::App(AppCommand)` holds the rest.
- `AppCommand` has six variants: `Visual`, `Playback`, `Navigation` and `Panel` (each wrapping
  its sub-enum), plus `QueuePlayCursor(usize)` and `Quit`.
- `App::dispatch` takes `AppCommand`. The translation match and both `unreachable!` arms are
  deleted. `Model::dispatch_router_command` matches `Command` exhaustively, with no `_` arm.
- The sub-enums `PlaybackCommand` and `NavigationCommand` collide with the *Avoid* list of
  *Transport command* in `CONTEXT.md`. All four sub-enums are renamed with the suffix
  `Intent`, and `CONTEXT.md` gains a *UI command* term for the nested `Command`.
- `From` impls from each sub-enum into `Command` keep construction sites short.
- **BREAKING** (internal only): the `Command` variant shape changes. Every construction and
  pattern site in `src/app/` and `crates/mbv-components/` changes. User-visible behavior does
  not change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a pure refactor. `.openspec.yaml` sets `skip_specs: true`.

## Impact

- `src/app/dispatch/action.rs`: enum definitions, `dispatch`, sub-dispatchers.
- `src/app/shell/arbitration.rs`: `dispatch_router_command` loses its `_` arm.
- `src/app/input/key_policy.rs` (26 sites), `router.rs`, and the keybind action tables.
- `src/app/shell/playback.rs`, `queue.rs`, `src/app/dispatch/mouse_gestures.rs`,
  `context_menu/actions.rs`, `crates/mbv-components/src/status_bar_panel.rs`.
- Tests that build or match `Command` (about 50 sites under `src/app/tests/`, `shell/tests.rs`,
  `dispatch/action/tests.rs`, `key_policy/*_tests.rs`).
- `CONTEXT.md` (new term), `docs/recipes.md` (the keybind-action and ctrl-variant recipes name
  the `Command` variant site).
- Unblocks `land-tier2-ast-grep-tripwires` (its `App::dispatch` complexity gate).

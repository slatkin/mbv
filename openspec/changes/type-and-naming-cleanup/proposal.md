## Why

Three follow-ups from the PlaybackSession state-machine work (#449) remain: bare-string IDs that can be silently crossed, an `is_queue_mode` AtomicBool that duplicates `origin == PlaybackOrigin::Queue`, and the `power` prefix naming a concept that no longer exists. Each is a source of either silent bugs (IDs, origin) or reader confusion (power), and all three are pure refactors with no behavioral change.

## What Changes

- **ID newtypes.** Introduce `ItemId`, `MediaSourceId`, and `EmbySessionId` newtypes in mbv-core. Starting point: `player_runtime.rs:200` where three interchangeable Strings sit in `Arc<Mutex<(String, String, String)>>`. Migrate the ~123 bare-String ID sites in mbv-core; the TUI layer (`src/`) can follow later.
- **Eliminate `is_queue_mode`.** Remove the `AtomicBool` from `PlaybackRun` and `RuntimeController`. Derive it from `origin == PlaybackOrigin::Queue` at each use site (currently ~13 refs across three files). The hand-sync at `player_run_queue.rs:23` and the independent writes at `player_runtime_controller.rs:341,499` all disappear.
- **Drop the `power` prefix.** Rename `power_*` identifiers and filenames to their plain equivalents (`render_power_queue` → `render_queue`, `power_home_actions` → `home_actions`, etc.). ~93 non-test refs, ~207 with tests, 11 files carry it in the filename. `powerline` (render/indicators.rs — Nerd Font separator glyphs) is a different word and must not be touched.

## Capabilities

### New Capabilities

_None — pure refactor._

### Modified Capabilities

_None — no behavioral changes. `skip_specs: true` is set._

## Impact

- **mbv-core:** ID newtypes touch ~123 sites. Origin dedup touches 3 files (~13 refs). Both are compile-verified — illegal assignments stop building.
- **src/ (TUI):** The `power` rename touches ~11 filenames and ~93 non-test refs. Mechanical but wide blast radius. Must not touch `powerline`.
- **Tests:** Existing tests pass unchanged (IDs and origin are internal; power rename is purely cosmetic). Test files that carry `power` in their name get renamed.
- **No API, protocol, or dependency changes.**

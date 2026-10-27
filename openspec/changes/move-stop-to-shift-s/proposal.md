## Why

`Esc` is two things today. In every leaf it means back, close, or clear. In the
router it is also the default Stop chord. Commit `a65d18b06` made the two meet:
a second `Esc` inside a 600 ms window stops playback from any focus. That adds
shell timing state (`last_esc`), a post-router match on `leaf_stops`, and a
pending rewrite (`keyboard-resolution-seam`) to move the clock into a new seam.
A dedicated Stop chord removes the overlap, so `Esc` can stay a plain leaf key.

## What Changes

- **BREAKING**: the default `stop` chord changes from `Esc` to `Shift+S`. `Esc`
  no longer stops playback. There is no `Esc` alias.
- `Shift+S` stops on that press, from any focus where the Playback gate is open.
  It needs no second press and no leaf disposition.
- Remove the double-Esc stop window: `last_esc`, `DOUBLE_ESC_STOP_WINDOW`, the
  reset in `ui_state_reset`, and the `Deferred(Stop)` rewrite with `leaf_stops`
  in `Model::router_outcome`.
- The playback panel stops claiming `Esc`. It no longer maps `Esc` to Stop and
  no longer consumes it, so `Esc` falls through as an unhandled key there.
- Keep the router's deferred-candidate rule for a configured `Esc` stop chord.
  A user config with `stop = "Esc"` still loads. That `Esc` stops on the press
  where the focused component leaves it unhandled, and a consumed `Esc` cancels
  it. There is no window and no timing state. This is the behavior the main
  `semantic-input-arbitration` spec already states.
- Update the daemon-intents scenario that names `Escape` as a stop input.
- Docs follow-up: ADR 0023 Amendment (300 ms and the clock text) and the
  `CONTEXT.md` *Arbitration fold* entry.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `configurable-keybinds`: adds a requirement. The default Stop chord is not
  `Esc`, and it fires immediately. `Esc` keeps its leaf meaning in every focus.
- `semantic-input-arbitration`: the "Context-sensitive global actions are
  deferred candidates without timing state" requirement keeps its rule. Its
  Escape scenarios are reworded to a stop chord configured as `Esc`, because
  `Esc` is no longer the default.
- `daemon-playback-intents`: the "Legacy double-key confirmation is absent"
  scenario names the Stop chord, not `Escape`.

## Impact

- `crates/mbv-keybinds/src/registry.rs`: `stop` default chord. The comment on
  `prefix_addressable` names `Esc` as the reason; reword it.
- `crates/mbv-keybinds/src/tests.rs`: default-chord table and the override-count
  test that uses `stop = Esc` as a "redundant default-equal override".
- `src/app/shell/arbitration.rs`, `src/app/shell.rs`,
  `src/app/shell/ui_state_reset.rs`: the window and its clock go away.
- `src/app/input/router.rs`, `src/app/input/key_policy.rs`: comments only.
  The `Esc` deferral and the `sessions_sidebar_escape` order stay.
- `crates/mbv-components/src/library_playback_panel.rs`: `Esc` handling.
- Tests: `src/app/shell/tests.rs`, `src/app/tests/tick_integration.rs`,
  `src/app/tests/routing_matrix/playback.rs`, `src/app/shell/settings.rs`.
- Help text needs no edit. The help overlay reads the chord from the registry.
- No change to `mbv-ctrl`, `mbvd`, queue state, or playback lifecycle. The
  `keyboard-resolution-seam` change loses its Escape-clock work. Reduce it after
  this change is accepted.

# Proposal

## Why

In the pinned panel the collapsed width (40 columns) is a mini view, and mini view starts
queue-only — a glanceable now-playing surface. The expanded width (120 columns) is where browsing
makes sense. Today reaching the library from the pinned panel means remembering that `x` cycles
panel modes at 80+ columns and that `Ctrl+e` must be pressed separately, and the cycle's `both`
state is cramped next to tiled windows anyway. One key that flips between "collapsed queue" and
"expanded library" matches how the panel is actually used.

## What Changes

- While pinned, `x` stops cycling panel modes and becomes the pinned view toggle: from the
  collapsed width it expands the panel to `cols_expanded` (the same resize path as `Ctrl+e`) and
  shows library-only with library focus; from the expanded width it collapses to `cols` and shows
  queue-only with queue focus.
- `Ctrl+e` keeps its existing behaviour in pinned mode: a plain width toggle that leaves the
  displayed panel mode alone.
- The three-state cycle and the mini-view `x` toggle become unreachable while pinned; both are
  unchanged outside the pinned panel.
- No new keybind action, config key, or CLI flag: the existing `panel_mode_cycle_x` action gains a
  pinned branch at dispatch.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `panel-mode`: the three-state `x` cycle requirement is scoped to non-pinned launches; new
  requirement `Pinned launches bind x to the pinned views`.
- `pinned-launch`: `Pinned panel width toggle` clarifies that `Ctrl+e` changes only the width and
  never the displayed panel mode.

## Impact

- `src/app/dispatch/action.rs` (`cycle_panel_mode` gains the pinned branch; new `pinned_view_toggle`
  next to `toggle_pinned_width`), `README.md` / help label wording if they describe `x`'s cycle
  unconditionally.
- No keybind registry, key-policy, pinwin, protocol, or daemon changes.

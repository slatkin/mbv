# Proposal

## Why

On a short terminal the QueueColumn's visual slot (now-playing artwork or the visualizer) takes
12–24 rows. That leaves the queue list with only a few rows, and today there is no way to get
those rows back while playback runs. The user needs a manual escape: hide the slot, keep
everything else in now-playing visible. `h` is unbound since the TuiRealm migration dropped the
old "hide playback panel" toggle, and it is the historical key for hiding playback chrome.

## What Changes

- A new configurable keyboard action, `hide_visual_slot`, default chord `h`, shows or hides
  the QueueColumn's visual slot. The header row and the transport (title, controls,
  `pos / dur`) stay visible.
- While the slot is hidden it reserves zero rows, as it does while idle:
  - Below 100 columns the transport moves up directly under the header.
  - At 100 columns and wider the transport takes the full slot-region width, and the band
    is the transport's own height.
- While the slot is hidden:
  - The slot's artwork is not fetched.
  - The PipeWire visualizer worker does not run.
  - `v` does nothing: the artwork/visualizer selection is left as it was.
- The hidden state persists across launches in the prefs file. The artwork/visualizer
  selection stays session-local, as now.
- The global `h` wins over Feeds' local `h` (a vim-style "move up" alias). Feeds is not edited
  in this change; removing all vim-style navigation and aligning shortcuts across library
  screens is tracked separately in #794.
- No automatic hiding based on terminal height.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-playback-panel`: adds the user-hidden visual slot requirement (zero rows, transport
  placement at both breakpoints, header and transport kept, persists across launches).
- `system-audio-visualizer`: while the slot is hidden, `v` does nothing and capture does not
  run; no artwork fetch for a hidden slot.

`configurable-keybinds` gains a declared action but no requirement changes: its requirements
govern the table, not its rows.

## Impact

- `crates/mbv-core/src/keybinds/registry.rs`: new `KeybindAction` row.
- `src/app/input/key_policy.rs`: new `KeyPolicyEntry` + binding → new `Command`.
- `src/app/dispatch/action.rs`: new `Command` arm.
- `src/app/state/app_struct.rs`, `construct.rs`: new App flag, loaded from prefs.
- `src/app/input.rs` (`save_prefs`): writes the flag.
- `src/app/shell/chrome_panels.rs`: card geometry sync and slot paint skip the slot when hidden.
- `src/app/render/arrangements/chrome.rs` (`compute_chrome_geometry` input): the
  `card_height` it receives is 0 while hidden, which already produces the collapsed layout.
- `src/app/shell/queue.rs`: artwork projection skipped while hidden.
- `src/app/infra/visualizer.rs`: `v` does nothing and the worker does not run while hidden.
- Help sidebar and settings Keys screen pick up the new action from the registry with no
  extra code.

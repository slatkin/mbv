# Proposal

## Why

pinwin 0.4.0 changes from a terminal library into a layer-shell app host for any Wayland app.
It removes the library API, the pty hand-over and the instance socket, and mbv uses all three
(issue #953). mbv stays on the 0.3.4 tag until it migrates, but 0.3.x gets no more work. pinwin
0.4.1 (slatkin/pinwin#55) adds what mbv needs to drive its own panel: `PINWIN_PANEL` and
`PINWIN_SOCKET` in the app environment, size and reserve mode in one request,
`pinwin run --show-existing`, and gutters as run options and a live `set-gutters` command.

## What Changes

- **BREAKING** mbv no longer links pinwin. The `pinwin` crate goes out of `Cargo.toml`, and the
  `pinwin` program (0.4.1 or later) becomes an optional runtime dependency that mbv runs.
- **BREAKING** `mbv --pin` becomes a launcher. It runs
  `pinwin run --name mbv --show-existing <options> -- <pin_terminal> <mbv>`, so the TUI runs in a
  terminal that pinwin docks as the panel. mbv opens no pty and gives no stdio to a panel. A
  second `--pin` shows the running panel.
- mbv knows that it runs pinned from `PINWIN_PANEL`, not from a hand-over. It sends every panel
  request with the `pinwin` CLI through `PINWIN_SOCKET`.
- New `[panel] pin_terminal` argv prefix: the terminal that runs mbv in the panel. It must open
  its window in the process that `pinwin run` starts.
- **BREAKING** `[panel] cols` and `cols_expanded` (terminal columns) become `width` and
  `width_expanded` in logical pixels. mbv ignores the old keys with a logged warning.
- The four `[panel]` gutters stay in mbv with their F2 rows. `--pin` passes them as
  `--gutter-*` run options, and a gutter step while pinned runs `pinwin set-gutters`.
- `x` while pinned sends one request: `set-size <width_expanded> --reserve cover` to expand,
  `set-size <width> --reserve push` to collapse. Tiled windows do not move.
- **BREAKING** The `pinned_width_toggle` keybind (`Ctrl+e`) goes away. `x` is the only pinned
  width control.
- `mbv --toggle` runs `pinwin toggle` for the mbv panel. It keeps its failure report.
- F2 `Side` changes the running panel with `pinwin set-edge`. Accent rows map to `--focus-ring`
  and still apply at the next launch.
- The pinned-launch rule for a hidden expanded panel is corrected: hiding releases the strip at
  any width. pinwin 0.3.4 already does this, and the spec text did not match it.
- The Pin swap passes `MBV_SWAP_TOKEN` through the launcher to the pinned Client. The Owner
  process does not inherit `PINWIN_PANEL` or `PINWIN_SOCKET`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pinned-launch`: the pin flag becomes a pinwin launcher; start failures, layout settings
  (pixel widths, `pin_terminal`), the toggle, the focus accent, push and cover by
  width and the hidden-panel rule change; the width-toggle requirement is removed; the
  library-isolation requirement becomes "mbv links no panel library".
- `panel-mode`: `Pinned launches bind x to the pinned views` uses pixel widths and one pinwin
  request in place of the removed width toggle.

## Impact

- Code: `src/pin.rs` (rewrite: argv builders and `pinwin` CLI calls), `src/main.rs` (launcher
  path, no `PinnedPanel`), `App.pinned_panel` and `pinned_width`,
  `src/app/dispatch/action.rs` and `settings.rs`, `src/local_daemon.rs` (Owner spawn env),
  `crates/mbv-config` (`[panel]` keys, parse, save), `crates/mbv-keybinds` and
  `src/app/input/key_policy.rs` (`pinned_width_toggle` removed), the F2 Panel page in
  `mbv-ui-model`, `mbv-components` and `mbv-render`.
- Dependencies: `pinwin` crate and its Wayland, font and raster stack leave the build. `pinwin`
  0.4.1 or later becomes a PKGBUILD `optdepends`.
- Docs: `CONTEXT.md` (Pinned panel, Pinned panel width, Focus accent, Pinned launch),
  `AGENTS.md` (pinwin entry), invariants 17 and 20 (pinwin tween wording), a new ADR for the
  move from library to app host.
- Users: existing `[panel]` column widths stop working; compositor bindings to
  `mbv --toggle` keep working; a binding that rebinds `pinned_width_toggle` becomes an unknown
  keybind action.

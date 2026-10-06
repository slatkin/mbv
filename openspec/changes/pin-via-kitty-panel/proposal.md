# Proposal

## Why

The pinned panel depends on pinwin. pinwin is a GTK4 layer-shell panel that embeds libghostty-vt, which is only a terminal model. pinwin must build the rest of a terminal on top of it: drawing, fonts, input encoding and the image protocol. A build needs Zig 0.16, GTK4, gtk4-layer-shell and pango, and a cold build fetches and compiles ghostty. mbv carries about 250 lines of `unsafe` pty hand-over code for it. kitty's `kitten panel` is a layer-shell panel that kitty maintains as part of a complete terminal, and kitty lists niri as fully working. This change replaces pinwin with `kitten panel` so that panel work stops being terminal work.

## What Changes

- `mbv --pin` stops hosting a panel in its own process. It starts `kitten panel`, which runs a second mbv process in a kitty layer-shell surface. The outer process waits for kitty and exits with its status. **BREAKING**: the Pin flag requirement "same process, no re-launch of mbv" no longer holds.
- The pinned mbv resizes and re-docks its panel through kitty remote control (`kitten @ resize-os-window --action=os-panel`) on a per-launch socket.
- Remove the `pinwin` dependency. mbv no longer links GTK4, gtk4-layer-shell, pango, cairo or libghostty-vt, and the build no longer needs Zig.
- Delete the pty hand-over code: `openpty`, `TIOCNOTTY`, the stdio `dup2` and the `TERM`/`COLORTERM` override. kitty gives the pinned mbv a real terminal.
- kitty becomes a runtime dependency of `--pin` only. Plain `mbv`, `mbvd` and the Owner process do not need it.

Features lost against pinwin:

- **BREAKING**: The focus accent. kitty draws no border around a single-window panel. The `accent`, `accent_color` and `accent_width` keys and their three F2 rows are removed. mbv ignores old keys in `config.toml` and drops them on the next save.
- **BREAKING**: The animated width change. `Ctrl+e` and the width rows snap to the new width.
- **BREAKING**: Layout checking. pinwin refused a layout that left the output no width, and mbv showed a toast. kitty does not check, so mbv can only report a remote-control failure.
- The panel font and colors follow `kitty.conf`, not the Ghostty configuration.
- A launch failure after kitty starts (for example, no layer-shell) no longer falls back to the terminal. Only failures that mbv finds before it starts kitty fall back.

Features kept:

- Left or right docking, the collapsed and expanded widths, and live resize.
- All four gutters, and push and cover modes.
- On-demand keyboard focus, and focus in and out reports.
- kitty graphics, the kitty keyboard protocol, SGR mouse and truecolor.
- The panel stays on its first monitor.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pinned-launch`: Seven requirements change to describe a kitty-hosted panel. They are the Pin flag, the start failure, the `[panel]` layout keys, the width toggle and the covering mode, and two removals: build isolation and the focus accent. One requirement is added for the private remote-control socket.

## Impact

- `src/pin.rs`: rewritten as the kitty launcher and remote-control client. Most `unsafe` code goes.
- `src/main.rs`, `src/app/state/app_struct.rs`, `src/app/shell/settings.rs`: the pinned handle type and the apply path.
- `crates/mbv-config` (`panel.rs`, `parse.rs`, `save.rs`), `crates/mbv-components/src/settings.rs`, `crates/mbv-render` F2 Panel rows: accent removal.
- `Cargo.toml`, `Cargo.lock`: the `pinwin` dependency goes.
- `.github/workflows/build.yml`: drop `zig gtk4 gtk4-layer-shell` and the two `.deb` GTK `Depends` assertions. Packaging lists kitty as optional.
- `CONTEXT.md` (*Pinned panel*, *Focus accent*), `AGENTS.md` (the pinwin repository-map entry), `README.md`, `docs/invariants/17-resize-neither-blanks-nor-wipes-art.md`.
- The in-flight change `pin-focus-request` uses `pinwin::Panel::request_focus()`, which kitty does not have. It stays on hold until this change lands, then needs a new plan. niri gives an on-demand layer surface keyboard focus only on a click, for kitty as for pinwin, so this change does not solve focus. This change does not edit `pin-focus-request`.
- No change to `mbv-daemon`, `mbvd` or the Owner process.

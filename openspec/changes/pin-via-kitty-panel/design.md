# Design

## Context

Today `mbv --pin` calls `pinwin::Panel::start` in its own process. mbv opens a pty pair, gives the master fd to pinwin, detaches its controlling terminal and moves its stdio to the slave (`src/pin.rs`). pinwin paints the grid with libghostty-vt in a GTK thread. Live changes go through `Panel::apply_layout_animated`. If the output cannot hold the layout, it returns `PinwinError::InvalidLayout`.

`kitten panel` inverts this model. kitty owns the pty and runs the program as its child. kitty 0.49.2 is on the development machine. Its relevant options are `--edge=left|right`, `--columns`, `--margin-*`, `--layer`, `--focus-policy`, `--exclusive-zone` with `--override-exclusive-zone`, and `--listen-on` with `-o allow_remote_control=socket-only`. A running panel changes with `kitten @ resize-os-window --action=os-panel --incremental <option>=<value> ...`, which takes the same option names.

## Goals / Non-Goals

**Goals:**

- `mbv --pin` and the desktop entry keep working on niri with the kept features in the proposal.
- No GTK, Zig or libghostty-vt in the build.

**Non-Goals:**

- A focus request or a one-panel rule. That is the scope of `pin-focus-request`, which is on hold.
- A focus accent drawn by the mbv TUI. mbv already gets focus reports, so a later change can add one.
- Width animation through repeated remote-control resizes.
- Support for terminals other than kitty.

## Decisions

### D1. Two processes: launcher and pinned mbv

The outer `mbv --pin` is a launcher. It reads the configuration. It makes sure that `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` are set and that `kitten` is on `PATH`. Then it spawns:

```text
kitten panel --edge=<side> --columns=<cols> --layer=top --focus-policy=on-demand
  --margin-top=<gt> --margin-bottom=<gb> --margin-left=<gl> --margin-right=<gr>
  [--exclusive-zone=0 --override-exclusive-zone]          # cover = true
  -o allow_remote_control=socket-only
  --listen-on=unix:$XDG_RUNTIME_DIR/mbv-pin-<launcher pid>.sock
  -- <current_exe> [--log-level <level>]
```

The launcher sets `MBV_PINNED=1` in the kitty environment and waits for kitty. It exits with the kitty exit status. The launcher starts no Owner process and no TUI.

The pinned mbv is the child. It reads `MBV_PINNED` and `KITTY_LISTEN_ON` once at start. Then it removes both from its own environment, so the Owner process and mpv do not inherit them.

Alternative considered: `exec` kitty in place of the launcher. It is one process fewer. But if stdin is not a terminal, mbv then cannot notify after a kitty failure.

Alternative considered: `--detach`. It breaks "the terminal waits until mbv exits".

### D2. `PinnedPanel` becomes the socket address

`PinnedPanel` holds the remote-control address from `KITTY_LISTEN_ON`. `apply_layout` runs `kitten @ --to=<address> resize-os-window --action=os-panel --incremental edge=<side> columns=<n> margin-top=... margin-bottom=... margin-left=... margin-right=...` and waits for it. A non-zero exit becomes the error string, and the existing toast path shows it. The call is synchronous, like the current blocking `apply_layout_animated`. It is a short local round trip.

Alternative considered: speak the kitty remote-control protocol on the socket directly. It removes a process spawn per resize but adds a JSON framing that kitty can change. Resizes happen on a key press, so the spawn cost does not matter.

Dropping the `pinwin::Panel` handle closed the panel. If its child exits, kitty closes the panel. mbv needs no drop action.

### D3. Socket in the runtime directory

The socket is a file in `$XDG_RUNTIME_DIR`, which has mode 0700. An abstract socket (`unix:@name`) has no file permissions. Any local user can connect to one, resize the panel or run `launch` on it.

### D4. Reservation width

pinwin reserved the docking-edge gutter, the panel width and the far gutter. The kitty default zone for an edge panel is the panel size, and layer-shell adds the docking-edge margin. The far gutter is then probably not reserved, so a positive far gutter overlaps the tiled windows. Task 1.1 probes this on niri.

If the probe confirms the overlap, the pinned mbv computes the zone in pixels from the cell width (`TIOCGWINSZ` `ws_xpixel / ws_col`). It sends `exclusive-zone=<px> override-exclusive-zone=yes` with every apply. It also does one apply right after start. If the probe shows that niri reserves the far margin already, this step is not built.

### D5. Accent removal

`PanelConfig` loses `accent`, `accent_color` and `accent_width`. The parser ignores unknown keys already, so an old `config.toml` still loads. The save path writes `[panel]` in full and so drops the keys. The three F2 rows and the "applies on the next launch" toast go.

### D6. `TERM` and stderr

kitty sets `TERM=xterm-kitty`, so `PanelEnv` goes. The pinned mbv keeps the stderr redirect to the crash log. mpv and other native code can still write to stderr and draw over the TUI.

## Risks / Trade-offs

- [kitty is missing or too old for `--action=os-panel`] → The launcher looks for `kitten` before it starts anything. The README gives the minimum version. A remote-control failure shows a toast.
- [The user `kitty.conf` changes the panel look, for example a background opacity] → Accept. It is the terminal configuration of the user.
- [A user `kitty.conf` sets `allow_remote_control=no`] → kitty applies `-o` overrides after `kitty.conf`, so the command line wins. Task 1.2 confirms it.
- [Negative gutters] → layer-shell allows negative margins. Task 1.2 confirms that kitty accepts them in `--margin-*` and in `resize-os-window`.
- [A layout that leaves no output width] → mbv cannot find this before it applies. The compositor shows a broken layout until the user steps the value back. The proposal lists this loss.
- [Two mbv processes in the process tree] → The launcher holds no Service, Player or lock. It only waits.

## Migration Plan

One commit series on `main`. Rollback is a revert, which restores the pinwin dependency. Users who build from source no longer need Zig or the GTK packages. Packagers add kitty as an optional dependency. The pinwin repository is not changed by this change.

## Open Questions

- The exact minimum kitty version for `resize-os-window --action=os-panel`. Task 1.3 reads the kitty changelog and records it.

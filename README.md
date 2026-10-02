## mbv

A TUI front-end for mpv with Emby, Audiobookshelf, media RSS feed support. You can download it. It's a vibe-coded funtime hobby though. You don't wanna.

### Instructions
You turn it on and put in some stuff and there it is.

### Pinned panel

`mbv --pin` runs the TUI docked to a screen edge in a Wayland layer-shell panel instead of
drawing in the terminal. The bundled desktop entry launches it that way (`Exec=mbv --pin`,
`Terminal=false`); quitting mbv removes the panel. Plain `mbv` always runs in the current
terminal, including over ssh and inside tmux.

The panel needs a Wayland session that provides layer-shell (for example niri, sway or
Hyprland). GNOME and X11 do not, and are unsupported from the launcher: the panel fails to
open, mbv records the reason in its log, sends a desktop notification and exits with status 1.
Run plain `mbv` in a terminal on those sessions. The GTK4 and gtk4-layer-shell
runtime libraries are required; building from source additionally needs Zig and
the GTK4 and gtk4-layer-shell development packages.

### Panel layout

`mbv --pin` reads the `[panel]` section of `config.toml`. The same values can be changed while
mbv runs in F2's Panel page (the numeric rows step by 1, or by 10 with Shift).

| Key | Meaning | Range | Default |
| --- | --- | --- | --- |
| `side` | Screen edge the panel docks to | `"left"` or `"right"` | `"left"` |
| `cols` | Panel width in terminal columns | 1–65535 | `40` |
| `gutter_top` | Space above the panel | any integer, may be negative | `0` |
| `gutter_bottom` | Space below the panel | any integer, may be negative | `0` |
| `gutter_left` | Space to the left of the panel | any integer, may be negative | `0` |
| `gutter_right` | Space to the right of the panel | any integer, may be negative | `0` |

Gutters are logical pixels. An out-of-range or malformed value falls back to its default with a
logged warning, without changing the other keys.

### What's broken
Presumably a lot of stuff.

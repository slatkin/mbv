# Design

## Context

See proposal.md for the reason. Today `src/pin.rs` links pinwin 0.3.4. A `--pin` launch opens a
pty, starts `pinwin::Panel` on the master, and moves its own stdio to the slave. `App` holds the
`Panel` handle and calls `apply_layout_animated` for `x`, `Ctrl+e` and F2 Panel rows. The
instance socket serves `--toggle` and the duplicate `--pin`.

pinwin 0.4.1 (slatkin/pinwin#55) gives mbv these facts to build on:

- `pinwin run [options] -- <command>` asks the per-display daemon for a panel, sets
  `WAYLAND_DISPLAY` to the panel's own display, sets `PINWIN_PANEL` and `PINWIN_SOCKET`, and
  replaces itself with the command. The first top-level window of the command becomes the
  panel. The panel ends when the command has exited and no Wayland connection to it is left.
- Run options: `--name`, `--edge`, `--size <N>px`, `--reserve push|cover`, `--keyboard`,
  `--focus-ring off|#RRGGBB:<w>`, `--show-existing`. Gutters exist only in pinwin's config file.
- Client commands (`toggle`, `set-size … --reserve … --ms …`, `set-edge`, …) exit 0 on
  success, 1 with pinwin's message on stderr for a daemon error or no daemon, and 2 for a usage
  error. Clients find the daemon through `PINWIN_SOCKET` when it is set.
- A covering panel holds the strip from its last pushing size. A change from cover back to push
  at that size moves no tiled window. Hiding releases the strip at any width.

## Goals / Non-Goals

**Goals:**

- Keep `x` (expand over the tiled windows, collapse back, view pairing) and the Pin swap.
- Leave the mbv build with no panel library.
- Keep every pinwin call in `src/pin.rs`, the only pinwin boundary.

**Non-Goals:**

- A native Wayland backend or an embedded terminal in mbv.
- Gutter settings in mbv, or writes to pinwin's config file.
- Pinned behavior for a resize that mbv did not request (a compositor key, the pinwin tray).
  Such a resize gets the normal width rules only.
- Runtime focus-ring changes.

## Decisions

### D1. A terminal hosts the TUI in the panel

`--pin` runs `pinwin run … -- <terminal argv> <mbv exe>`. The terminal argv comes from the new
`[panel] pin_terminal` prefix, else `TERMINAL` followed by `-e`, else the launch fails with a
reason that names both, as Unpin does. The mbv exe follows the `launch_exe` rule in
`src/local_daemon.rs`: `current_exe()`, or `/proc/<launcher pid>/exe` when the file was replaced
(`… (deleted)`). The launcher waits for its child (D2), so that path stays valid while the
terminal starts mbv, and a Pin swap starts the Owner's own build (`pin-swap`).

The terminal must open its window in the process that `pinwin run` starts (Ghostty needs
`--gtk-single-instance=false`). That is why `pin_terminal` is separate from `[panel] terminal`:
the Unpin prefix can legally give the launch to a running copy.

Alternatives: a native ratatui Wayland backend, or the 0.3.4 terminal stack moved into mbv.
Both bring a font, raster and input stack into mbv, which pinwin itself just dropped. Rejected.

### D2. `--pin` spawns `pinwin run` and waits for it

The launcher first checks what it can check with no Wayland code: `WAYLAND_DISPLAY` is set, a
terminal argv resolves, and `pinwin` starts. A failure there takes today's start-failure path
(terminal fallback when stdin is a terminal, else a notification and exit 1).

Then it spawns `pinwin run` as a child, waits, and exits with the child's status. A non-zero
status is logged, and reported with a notification when stdin is not a terminal, because a
desktop launch has no stderr to read (no layer-shell, an invalid pinwin config). After the spawn
there is no terminal fallback: the status cannot tell a pinwin failure from a terminal that
ended with an error after a long session.

Alternative: `exec` into `pinwin run`. It saves one idle process, but a desktop launch on a
compositor without layer-shell then fails with no notification. Rejected.

### D3. Pinned means `PINWIN_PANEL` is set

At start, on the main thread and before any thread exists, mbv reads `PINWIN_PANEL` and
`PINWIN_SOCKET` into a `OnceLock` and removes both from its environment, the same way it claims
`MBV_SWAP_TOKEN`. A non-empty `PINWIN_PANEL` means pinned, and its value is the panel name for
every request. So:

- The Owner process, mpv and every other child do not inherit the panel identity. A stay-alive
  Owner cannot think that it is pinned.
- A terminal that gave the launch to a running copy starts mbv without the variable, so mbv runs
  as an unpinned Client, which matches what the user sees.
- The `HANDED_OVER` flag, the pty code, `PanelEnv` and the stderr redirect go away.

The `--pin` launcher sets `MBV_SWAP_TOKEN` on the `pinwin run` child from the claimed token, so
the Pin swap reaches the pinned Client.

### D4. Panel requests run the `pinwin` CLI

Each request is one synchronous `pinwin <command>` child with `PINWIN_SOCKET` set from D3. Exit 0
is success. Exit 1 gives pinwin's stderr line as the Warning toast text. Exit 2 or a spawn
failure gives a fixed toast that names the cause (usage mismatch, `pinwin` not installed). The
argv builders are pure functions, and unit tests cover them. The process call is the mocked
boundary.

Alternative: write JSON lines to the daemon socket. It gives typed error kinds that mbv only
shows as text, and it copies pinwin's wire format into mbv with no compiler check. Rejected.

### D5. `[panel]` keys

| Key | Value | Default | Applied |
|---|---|---|---|
| `side` | `left`, `right` | `left` | `--edge`; while pinned `pinwin set-edge` |
| `width` | logical pixels, 1..=65535 | 400 | `--size`; `x` collapse; F2 while pinned |
| `width_expanded` | logical pixels, 1..=65535 | 1100 | `x` expand; F2 while pinned |
| `accent`, `accent_color`, `accent_width` | as today | as today | `--focus-ring`, next launch only |
| `pin_terminal` | argv prefix | unset | D1 |
| `terminal` | argv prefix | unset | Unpin and Open, unchanged |

`cols`, `cols_expanded` and the four `gutter_*` keys are read only to log one warning that names
them, and are then ignored. A save writes `[panel]` in full and so drops them. mbv-config checks
ranges only, and pinwin decides whether a width fits. F2 width rows step by 10, or 100 with
Shift. The gutter rows go away.

### D6. `x` keeps today's model with one request

`PinnedWidth` stays the runtime state: every pinned launch starts collapsed. `x` while pinned
sends `set-size <panel> <width_expanded>px --reserve cover` to expand, or
`set-size <panel> <width>px --reserve push` to collapse. On exit 0, `apply_pinned_view` sets the
view and focus as today. On a failure, a Warning toast shows the reason and nothing changes. The
terminal resize arrives later as a normal `Resize` event. pinwin sends one new size per change,
and invariant 17 still applies.

The view follows the request, not the resize. A resize that mbv did not request does not
change `PinnedWidth`, so the next `x` can ask for the width the panel already has. It then only
sets the matching view, and the `x` after that is correct.

Alternative: derive the view from the resize crossing 80 columns. It fails for a collapsed
width of 80 columns or more, which `panel-mode` allows. A rule based on the resize direction
fails on a terminal font zoom. Rejected.

`pinned_width_toggle`, `TogglePinnedWidth`, `toggle_pinned_width` and the "needs a pinned
launch" toast are removed. `note_pinned_width_applied` keeps only the width record: the
same-iteration pty-size adoption existed for the 0.3.4 in-process resize, and the size now
comes from the terminal.

### D7. F2 Panel rows while pinned

A `width` step sends the collapsed request of D6 with the candidate value, and a
`width_expanded` step sends the expanded one, so the edited value is the width that pinwin
validates (as today). A `Side` step sends `set-edge`. Only an accepted request saves the value.
Accent rows save and show the "next launch" toast, as today. Unpinned, every row saves with no
request.

### D8. `--toggle` and the duplicate `--pin`

`mbv --toggle` runs `pinwin toggle mbv` with the inherited environment (a compositor key runs
outside the panel, so pinwin finds the daemon from `WAYLAND_DISPLAY`). On a non-zero status it
reports pinwin's message by today's rule (stderr on a terminal, else a notification) and exits
1. The launcher always passes `--name mbv --show-existing`, so a second `--pin` shows the
running panel and exits 0 inside the daemon, with no race.

### D9. No pinwin crate

`Cargo.toml` drops `pinwin`. `libc` stays for `src/main.rs` and `src/app/infra/signals.rs`. The
PKGBUILD lists `pinwin>=0.4.1` in `optdepends`. `AGENTS.md` describes pinwin as an external
program with a minimum version, called only from `src/pin.rs`. A new ADR records the move from
library to app host and D1 to D4.

## Risks / Trade-offs

- [pinwin changes its CLI and mbv breaks at runtime, with no compiler check] → One minimum
  version in the PKGBUILD and `AGENTS.md`; the argv builders keep every flag in one file; a
  manual pinned check at each pinwin release.
- [A `pin_terminal` that gives the launch to a running copy leaves the panel empty] → D3 makes
  that mbv run unpinned, so the failure is visible and safe; the `pin_terminal` docs name the
  rule and the Ghostty flag.
- [A `pinwin` call blocks the UI thread] → The pinwin client has a bounded reply time, and the
  calls come only from key presses and F2 steps, as `apply_layout_animated` did.
- [Users lose `cols` widths and gutters] → One logged warning that names the keys; release
  notes give the new keys and the pinwin `[panel.mbv]` table.
- [A rebound `pinned_width_toggle` becomes an unknown keybind action] → Release notes. Only a
  user who rebound that key sees it.
- [`x` goes the wrong way once after an outside resize] → Accepted (D6).

## Migration Plan

1. Wait for pinwin 0.4.1 with #55.
2. Land this change and release it with notes for the `[panel]` keys, `pin_terminal`, gutters
   in pinwin's config, `Ctrl+e`, and the pinwin dependency.
3. Rollback: revert the change, which restores the `pinwin` 0.3.4 crate dependency.

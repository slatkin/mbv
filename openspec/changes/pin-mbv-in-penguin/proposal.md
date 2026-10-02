# Proposal

Issue: #864

## Why

Running mbv pinned in penguin today means two launcher entries' worth of setup and two tray icons
(penguin's and mbv's). mbv should have exactly one launcher and exactly one tray, with mbv's icon.
When mbv runs pinned, that tray gains one extra item that drives the penguin panel. Depends on
`add-penguin` (penguin in this repo, its control socket and `--no-tray`).

## What Changes

- **One launcher**: `contrib/mbv.desktop` stays the only desktop entry. When penguin is installed,
  it starts mbv pinned via `penguin --no-tray mbv`. Exact shape (`Exec=mbv --desktop` branch vs.
  direct `Exec`) and the no-penguin fallback (`xdg-terminal-exec`) are settled in design.
- **Pinned Client announces itself**: a Client started with `PENGUIN_SOCKET` in its environment
  tells the Owner process over ctrl that it is pinned and passes the socket path. This is an
  additive ctrl capability, not a protocol version bump. The Owner never reads `PENGUIN_SOCKET`
  from its own environment, because the Owner may predate the panel or outlive it.
- **One tray, mbv's icon**: the Owner's tray is shown while stay-alive is enabled **or** while a
  pinned Client is attached. The tray menu gains nothing stay-alive-specific. This makes the
  tray's presence dynamic: it appears when a pinned Client attaches and goes away when the last
  one detaches, unless stay-alive keeps it.
- **Pin item**: while a pinned Client is attached, the tray shows `Pin options...`, which sends
  `options` to that Client's penguin socket. penguin's own tray is never shown for mbv
  (`--no-tray`).

## Capabilities

### New Capabilities
- `pinned-launch`: the single mbv desktop launcher, its penguin and no-penguin paths, and how a
  Client detects that it is pinned.

### Modified Capabilities
- `local-daemon-tray`: "The tray belongs to a stay-alive local daemon" changes. The tray is shown
  when stay-alive is enabled or a pinned Client is attached, its presence can change at runtime,
  and it gains the `Pin options...` item while pinned.
- `ctrl-protocol`: additive capability for a Client to declare itself pinned with its penguin
  socket path.

## Open questions

- Does pinning force the tray icon even when `show_systray_icon = false`? (The proposal currently
  assumes "with pin the icon always shows"; confirm before writing the spec delta.)

## Impact

- `contrib/mbv.desktop`, `src/main.rs` (launch branch if chosen), Client attach path, ctrl
  vocabulary (`crates/mbv-ctrl`), Owner tray lifecycle (`crates/mbv-daemon/src/run.rs`
  `start_tray`, `src/local_daemon.rs` tray hook), tray menu (`crates/mbv-desktop/src/tray.rs`).
- `mbv` package: `optdepends` on `penguin`, plus `xdg-terminal-exec` if the fallback is kept.
  No GTK dependency.
- `mbvd` unaffected (still no tray).

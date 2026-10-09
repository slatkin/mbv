# Proposal

## Why

Moving between the pinned panel and a normal terminal today means quitting one TUI and starting
another by hand, and with Stay-alive off, quitting also stops playback. The Tray is always at hand
while the Tray is enabled, so it is the natural place for a one-click Pin/Unpin.

## What Changes

- The Tray menu gains one item: **Pin** when no pinned Client is attached, otherwise **Unpin**.
- A Pin swap replaces one Client with a new Client in the other mode. The Owner process keeps the
  Player, so playback continues without a break. The old Client saves its TUI launch state first,
  and the new Client restores it, so the user lands on the same tab and item.
- **Pin** targets the most recently connected unpinned Client and starts `mbv --pin`. With no
  Client attached, Pin only starts `mbv --pin`.
- **Unpin** targets the pinned Client and starts a terminal that runs `mbv`. The terminal command
  is the new `[panel] terminal` config key when set, otherwise `$TERMINAL -e`. There is no
  built-in default terminal. With neither set, Unpin sends a desktop notification and changes
  nothing.
- The old Client exits only after the new Client has attached. If the new Client fails to start or
  attach within a bounded time, the old Client stays and a desktop notification names the reason.
- With Stay-alive off, the Owner process admits the Client it spawned for the swap despite
  exclusive admission. The old Client's swap exit does not request coordinated shutdown.
- New ctrl capabilities: a Client advertises that it can be swapped and whether it is pinned. Two
  new Owner-to-Client events and one Client-to-Owner command carry the swap. The protocol version
  does not change.

## Capabilities

### New Capabilities

- `pin-swap`: a Pin swap from the Tray: choosing the target Client, starting the replacement
  Client, the handoff order, the terminal command for Unpin, and failure handling.

### Modified Capabilities

- `daemon-lifecycle`: Stay-alive-off exclusive admission admits the Client spawned for a Pin swap.
  A Client leaving through a Pin swap does not request coordinated shutdown.
- `tui-launch-state`: a Client being swapped out saves its launch state before the replacement
  starts, not at exit.

## Impact

- `crates/mbv-ctrl`: new capabilities, a `swap_token` Hello field, `SwapPrepare`/`SwapQuit`
  events, a `SwapPrepared` command.
- `crates/mbv-daemon`: Client registry records surface and swap support; swap state machine and
  timeout; admission exception; child process spawn with display env.
- `crates/mbv-desktop`: Tray menu item and its label source.
- `src/local_daemon.rs`: tray hook wiring for the new item.
- `src/app` (TUI shell): handle the swap events, save launch state, exit without the shutdown
  request.
- `crates/mbv-config`: `[panel] terminal` key.
- `CONTEXT.md`: the *Pin swap* term.
- No pinwin change.

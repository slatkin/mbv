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
- `mbv --swap-panel` runs the same Pin swap from any shell or compositor key. It asks the running
  Owner process to run the swap, then exits: 0 when the Owner accepted it, 1 with a message when
  no Owner process runs or the Owner refused it. It never starts an Owner process and does not wait
  for the swap to finish. Later failures arrive as the same desktop notification as a Tray click.
- The swap is the first *Owner action*: a closed set of user actions that the Owner process runs,
  each with exactly one CLI flag. The Tray and the CLI name the same Owner action and the Owner runs
  it in one place. The Tray sends it in-process; it does not start `mbv`. Later Tray items move onto
  Owner actions without new ctrl vocabulary. This change adds only the swap.

## Capabilities

### New Capabilities

- `pin-swap`: a Pin swap from the Tray or `mbv --swap-panel`: choosing the target Client, starting
  the replacement Client, the handoff order, the terminal command for Unpin, and failure handling.
- `owner-actions`: user actions the Owner process runs on request from the Tray or a CLI flag: one
  flag per action, how the CLI reaches the Owner, and its exit status.

### Modified Capabilities

- `daemon-lifecycle`: Stay-alive-off exclusive admission admits the Client spawned for a Pin swap
  and an Owner action connection. A Client leaving through a Pin swap does not request coordinated
  shutdown.
- `tui-launch-state`: a Client being swapped out saves its launch state before the replacement
  starts, not at exit.

## Impact

- `crates/mbv-ctrl`: new capabilities, a `swap_token` Hello field, `SwapPrepare`/`SwapQuit`
  events, a `SwapPrepared` command; the `OwnerAction` vocabulary (`RunOwnerAction` command,
  accepted/refused events, `TransportCommand::OwnerAction`).
- `crates/mbv-daemon`: Client registry records surface and swap support; one connection-role enum
  in place of the service-setup admin bools; swap state machine and timeout; admission exceptions;
  one Owner action handler; child process spawn with display env.
- `crates/mbv-remote-player`: a one-shot local Owner action request, modelled on the service-setup
  signal.
- `src/main.rs`: the Owner action flag dispatch and usage rows.
- `crates/mbv-desktop`: Tray menu item and its label source.
- `src/local_daemon.rs`: tray hook wiring for the new item.
- `src/app` (TUI shell): handle the swap events, save launch state, exit without the shutdown
  request.
- `crates/mbv-config`: `[panel] terminal` key.
- `CONTEXT.md`: the *Pin swap* and *Owner action* terms.
- No pinwin change.

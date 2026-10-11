# Proposal

## Why

When Stay-alive keeps the Owner process running with no TUI attached, the Tray is the only way
back into mbv. Today it offers only **Pin**, and **Pin** fails with "panel swap could not start
…/mbv (deleted)" after the Owner's binary is removed or replaced. The Owner log shows this on
2026-10-10 06:24 and 2026-10-11 06:32 (`daemon.pin_swap.spawn_failed`, no Client attached). Both Owners were started
from a worktree build that was then removed. The
user also has no Tray item that opens a normal terminal TUI.

## What Changes

- Fix: when the file the Owner started from is replaced or removed, the Owner starts the binary it
  is itself running (`/proc/<pid>/exe`), which is the same build that Clients must match. The Pin
  swap, Unpin, and the new Open item use this lookup. Today `std::env::current_exe()` returns
  `<path> (deleted)`, and the spawn fails.
- New Tray item **Open**, shown only while no local TUI Client is attached. It starts a terminal
  that runs `mbv`, with the same terminal command that Unpin uses.
- New Owner action `OpenTerminal` with the CLI flag `mbv --open`. The Tray item and the flag use
  the same Owner path. The Owner does not refuse Open while a Client is attached.
- Open uses the Pin-swap launch machine: one launch at a time, the 10 s attach deadline, and a
  desktop notification on failure. Open replaces no Client.
- The Tray reads one three-state value (no Client, unpinned only, pinned) in place of the
  "pinned Client attached" flag.
- The `mbv` Owner-action CLI reports "restart mbv" when an older Owner cannot run the flag. Today
  it waits 6 s and prints a vague message.
- `CONTEXT.md` describes the Tray and the Owner actions with the new item.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pin-swap`: adds the Tray **Open** item and its launch behavior, and adds a requirement that
  swap and Open launches start the Owner's own build after its binary file is removed or replaced.
- `owner-actions`: adds the `--open` flag to the Owner action flag list, and makes the CLI report
  an Owner that cannot run the requested action.

## Impact

- `crates/mbv-ctrl/src/owner_action.rs`: new `OwnerAction::OpenTerminal` variant and flag. New
  shared attached-Clients state type.
- `crates/mbv-daemon/src/ctrl.rs`, `core.rs`, `run.rs`: the registry publishes the three-state
  value in place of `pinned_client_attached`.
- `crates/mbv-daemon/src/event_loop/pin_swap.rs`, `owner_action.rs`: Open launch with no target.
- `crates/mbv-desktop/src/tray.rs`: **Open** item, menu order, watcher reads the new state.
- `src/local_daemon.rs`: binary lookup for the swap command hook.
- `crates/mbv-remote-player/src/connect/owner_actions.rs`: the reply wait maps `CommandRejected`
  to the restart error.
- `CONTEXT.md`: *Tray*, *Owner action*, and *Pin swap* entries.
- `mbvd` is not affected. It has no Tray and no swap command hook.

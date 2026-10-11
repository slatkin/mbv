# Proposal

## Why

When Stay-alive keeps the Owner process running with no TUI attached, the Tray is the only way
back into mbv. Today it offers only **Pin**, and **Pin** fails with "panel swap could not start
…/mbv (deleted)" after the Owner's binary is rebuilt or upgraded. The Owner log shows this on
2026-10-10 06:24 and 2026-10-11 06:32 (`daemon.pin_swap.spawn_failed`, no Client attached). The
user also has no Tray item that opens a normal terminal TUI.

## What Changes

- Fix: the Owner finds the mbv binary to start even after the file it was started from is
  replaced or removed. The Pin swap, Unpin, and the new Open item use this lookup. Today
  `std::env::current_exe()` returns `<path> (deleted)`, and the spawn fails.
- New Tray item **Open**, shown only while no local TUI Client is attached. It starts a terminal
  that runs `mbv`, with the same terminal command that Unpin uses.
- New Owner action `OpenTerminal` with the CLI flag `mbv --open`. The Tray item and the flag use
  the same Owner path. The Owner does not refuse Open while a Client is attached.
- Open uses the Pin-swap launch machine: one launch at a time, the 10 s attach deadline, and a
  desktop notification on failure. Open replaces no Client.
- The Tray reads one three-state value (no Client, unpinned only, pinned) in place of the
  "pinned Client attached" flag.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pin-swap`: adds the Tray **Open** item and its launch behavior, and adds a requirement that
  swap and Open launches start the current mbv binary after the Owner's binary changes on disk.
- `owner-actions`: adds the `--open` flag to the Owner action flag list.

## Impact

- `crates/mbv-ctrl/src/owner_action.rs`: new `OwnerAction::OpenTerminal` variant and flag. New
  shared attached-Clients state type.
- `crates/mbv-daemon/src/ctrl.rs`, `core.rs`, `run.rs`: the registry publishes the three-state
  value in place of `pinned_client_attached`.
- `crates/mbv-daemon/src/event_loop/pin_swap.rs`, `owner_action.rs`: Open launch with no target.
- `crates/mbv-desktop/src/tray.rs`: **Open** item, menu order, watcher reads the new state.
- `src/local_daemon.rs`: binary lookup for the swap command hook.
- `mbvd` is not affected. It has no Tray and no swap command hook.

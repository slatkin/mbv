# Proposal

## Why

The pinned panel covers the desktop while it is shown, and the Tray icon is the one control the
user already looks at. A left click on that icon does nothing today, so hiding the panel needs a
compositor key bound to `mbv --toggle` or a terminal. The click should do what `--toggle` does.

## What Changes

- A left click on the Tray icon (the status-icon `Activate` action) asks the Owner process to
  toggle the pinned panel: hide it when it is shown, show it when it is hidden. The request is the
  same one `mbv --toggle` sends, so the effect is the same.
- The request runs only while a pinned Client is attached. With no pinned Client there is no
  panel, and the click does nothing.
- The Owner process runs the request. The Tray starts no process, and the request runs off the
  Owner's event loop.
- `mbv --toggle` keeps its own path: the flag still sends the request from the CLI process, so a
  compositor key works with no Owner process running.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `local-daemon-tray`: adds the requirement that the Tray icon click toggles the pinned panel.
- `pinned-launch`: extends the "Panel toggle" requirement with the Tray as a second trigger for
  the same request.

## Impact

- `crates/mbv-ctrl/src/commands.rs`: the new `TransportCommand::TogglePanel` variant, plus the
  no-op arms it needs in the two transports that match every variant.
- `crates/mbv-desktop/src/tray.rs`: the `activate` handler of the Tray.
- `crates/mbv-daemon/src/core.rs`, `event_loop.rs`, `run.rs`: the `toggle_panel` runtime hook and
  the transport arm that runs it.
- `src/local_daemon.rs`, `src/pin.rs`: the runtime hook that sends the pinwin request.
- `crates/mbvd/src/main.rs`: the hook that does nothing; mbvd has no Tray and no pinned panel.
- `CONTEXT.md`: the Tray entry names the icon click.

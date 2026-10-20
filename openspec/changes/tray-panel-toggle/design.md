# Design

## Context

The Tray lives in the Local Owner process (`crates/mbv-desktop`, started by the daemon's
`on_tray_ready` hook). Its menu items reach the Owner over the transport channel
(`TransportCommand`), and the Owner resolves them in-process. The pinned panel is a pinwin surface
on the user's Wayland display; `mbv --toggle` already asks pinwin to hide or show it by running
`pinwin toggle mbv` from the CLI process.

Constraints that shape the approach:

- The Tray starts no process for a request (spec owner-actions "The Tray and the CLI run the same
  Owner action"). The Owner process runs it.
- The daemon library never names pinwin: the runtime supplies the hooks that touch the desktop
  (`swap_command`, `notify`). `mbvd` supplies refusing ones.
- The Owner process is what started the pinned panel (through `mbv --pin`), so its environment
  names the same Wayland display and the same pinwin socket the panel uses. The pinned Client
  scrubs `PINWIN_PANEL` and `PINWIN_SOCKET` from its environment, so the Owner process cannot
  inherit the socket path and resolves it from `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY` the way
  pinwin does.

## Goals / Non-Goals

**Goals:**

- A left click on the Tray icon hides the shown panel and shows the hidden one, with the same
  effect `mbv --toggle` has.
- The Owner process decides whether the request applies; the Tray only asks.
- The request never blocks the Owner's event loop.

**Non-Goals:**

- A menu item for the toggle. The click is the trigger the user asked for.
- Changing `mbv --toggle`. The flag keeps sending the request from the CLI process.
- Hiding the panel without a pinned Client, or opening a panel from the click.

## Decisions

### D1: The click asks the Owner, and the Owner decides

The `activate` handler of the Tray sends one transport command. The Owner's event loop reads the
ctrl registry and runs the request only while a pinned Client is attached. The registry is the one
source of that state: it also drives the attached-Clients value the Tray's Pin/Unpin label reads
and the target the Pin swap picks.

*Alternative:* the Tray reads the shared attached-Clients cell and sends the command only in
pinned mode. Rejected: the state would then be read in two places, and a stale cell would send a
request for a panel that no longer exists.

### D2: The request is a transport command, not an Owner action

`OwnerAction` is a user action with exactly one CLI flag, and the Owner action path is the ctrl
socket. The panel toggle already has a CLI flag (`--toggle`) that does not use the Owner process,
and the Owner-action path would make that flag fail when the Owner process is older than the CLI
or absent. The toggle is therefore a transport command of its own, resolved in-process like the
transport's other commands.

*Alternative:* make the toggle an `OwnerAction` and route `mbv --toggle` through the Owner too.
Rejected: a compositor key bound to `mbv --toggle` would then fail against an older Owner
process, where it works today.

### D3: The Owner runs the request through a runtime hook

The daemon library gains a `toggle_panel` hook beside `swap_command` and `notify`. The Local
runtime sends the request; `mbvd` supplies a refusing one, because it has no desktop session and
no Tray. The hook starts the request and returns: the Owner process must not wait for a child
process on its event loop.

*Alternative:* the daemon library spawns `pinwin` itself. Rejected: the daemon crate would name
an external program and the panel name, which the TUI binary owns.

### D4: The request is the one `mbv --toggle` sends

The runtime sends `pinwin toggle mbv`, the same argv the CLI flag sends, and reports a failure as
a desktop notification. The CLI flag keeps its own path: it sends the request from the CLI
process, so it works with no Owner process running.

*Alternative:* the Owner asks the pinned Client to send the request, so the request carries the
Client's own socket path. Rejected: the Client scrubs that path from its environment at start-up
(it keeps it only for its own panel requests), so the Owner would have to carry the socket path
over the ctrl protocol for a case the Owner's own display already covers.

## Risks / Trade-offs

- [The Owner process runs on a different Wayland display than the panel] → the request fails and
  the user gets a notification naming pinwin's reason. The Owner starts the panel itself, so the
  displays match unless the user started `mbv --pin` by hand against a different Owner process.
- [The request is sent while the panel is closing] → pinwin reports that no such panel runs, and
  the user gets a notification.
- [The pinned Client runs in a panel with another name] → the request names the `mbv` panel, as
  `mbv --toggle` does, so pinwin reports that no such panel runs. The pinned Client does not
  advertise its panel name, and adding it to the ctrl protocol is out of scope for this change.

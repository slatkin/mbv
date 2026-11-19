# Proposal

## Why

The GUI Client shows toasts the TUI way: as coloured text inside a status
line (the Queue footer line, the Library Status bar, or the Settings status
bar). The toast takes space from the armed `PREFIX` badge. Also, a new toast
erases the toast before it at once, often before the user can read it. A GUI
window must show toasts in the usual GUI way: as cards that float over the
window.

## What Changes

- The GUI Client shows each toast as a card that floats at the bottom-right of
  the window, over every panel, the Settings screen, and modal overlays. The
  card shows the toast's severity.
- Several toasts show at the same time, one above the other, the newest at the
  bottom. Each toast expires on its own severity-derived duration (2 s for
  Neutral and Success, 5 s for Warning and Error, as today).
- A maximum of 5 toasts show. When a sixth arrives, the oldest goes.
- A new toast with the same text and severity as a live toast replaces that
  toast. It becomes the newest toast and its duration starts again. Thus a
  repeated message does not fill the column with copies.
- The toast card has no interaction. It takes no pointer input: a press on a
  card goes to what is under it. The card does not change the layout.
- The Queue footer line, the Library Status bar, and the Settings status bar no
  longer show toasts. They keep the armed `PREFIX` state and their own content.
  The `PREFIX` badge no longer disappears while a toast shows.
- The system notifications setting keeps its current effect: a Success,
  Warning, or Error toast whose desktop notification is delivered does not
  show in the window.
- The Client core keeps a list of live toasts instead of one toast slot. The
  TUI is parked: it still shows only the newest toast, with its current
  behavior.

## Capabilities

### New Capabilities

- `gui-toasts`: How the GUI Client presents toasts: the floating cards, their
  order, their maximum count, and their place over the window.

### Modified Capabilities

- `toast-notification-semantics`: Toasts accumulate. Several toasts are live
  at the same time, each with its own expiry, a maximum of 5, and a repeated
  toast replaces its live copy.
- `gui-client`: The footer line and the Library Status bar no longer show
  toasts.
- `gui-settings-screen`: The Settings screen's status bar no longer shows
  toasts; the floating cards show over the screen.

## Impact

- Client core (`src/app/`): the toast slot fields (`status`, `status_expires`,
  `status_severity`) become one list of live toasts; `flash`, the expiry
  pass, the direct clear sites, and `toast_presentation` change. A new
  presentation gives the GUI the full list.
- `crates/mbv-ui-model`: the toast presentation type is shared by both
  Clients and stays the item type of the list.
- `crates/mbv-gui`: a new section carries the toast list; a new floating
  toast component paints it; `queue_footer`, `library_status_bar`, and
  `settings_status_bar` drop their toast paint.
- No new dependencies.
- Docs: `docs/architecture/gui-client.md` and `CONTEXT.md` change to match.

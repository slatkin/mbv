# Proposal

## Why

In the GUI Client the Queue column is at the left window edge and the Wide Library list is at the
right window edge, with the Hero pane between them. The user wants the opposite order: the Library
list at the left window edge and the Queue column at the right window edge. The Hero pane stays in
the middle.

## What Changes

- The GUI root row puts the Library panel first and the Queue column last. The Queue column is at
  the right window edge. The Queue divider is between them, on the Queue column's left edge. A drag
  to the left makes the Queue column wider.
- In the Wide arrangement the Library panel puts the list first and the Hero pane after it. The list
  is at the left window edge. The list divider is on the list's right edge. A drag to the right
  makes the list wider.
- The collapsed rail is at the right window edge. Its expand control points into the window (to the
  left).
- The Settings screen does not change: its sidebar stays at the left window edge and its section
  content takes the rest of the width. The spec names the left edge and no longer says "in the
  Queue column's place".
- Mini (only the Queue panel), Narrow (list with the Hero overlay over it), the arrangement
  thresholds, and the saved column widths do not change. A saved width keeps its meaning, so no
  saved state is migrated.
- The TUI Client does not change.

Decisions that the user made:

- Scope is the GUI Client only. The TUI frame and ADR 0027 stay as they are.
- The Settings sidebar stays on the left.

Recorded assumption: the design-canvas artboard copied into
`openspec/changes/archive/2026-11-07-align-gui-with-design-canvas/canvas/` shows the Queue panel
alone and fixes no side for the two columns, so this change does not depart from the canvas.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the requirement "The GUI presents the Library panel beside the Queue panel" fixes
  the side of each column (Library panel left, Queue column right). The requirement "The window
  arrangement follows the Library column width" fixes the Wide order (list left, Hero pane right).
  The requirement "The panel widths persist across launches" states the drag direction of each
  divider.
- `gui-settings-screen`: the requirement "The Settings screen replaces the two panels while it is
  open" puts the sidebar at the left window edge instead of "in the Queue column's place".

## Impact

- `crates/mbv-gui/src/window/app.rs`: the order of the root row children and the Queue divider's
  side.
- `crates/mbv-gui/src/window/components/library_panel.rs`: the order of the Wide list and Hero
  pane, the list divider's side, and the list's border edge.
- `crates/mbv-gui/src/window/components/collapsed_rail.rs`: the expand control's icon.
- `crates/mbv-gui/src/window/components/panel_divider.rs`: doc comments only, if they name a side.
- `docs/architecture/gui-client.md`: the sentences that place the columns.
- No dependency, protocol, config, or Client core change. No change to `mbv-render` or the TUI.
- The in-flight change `gui-named-queues` rebuilds the queue header, rows, and footer. It does not
  move the columns. The only shared file is `app.rs`, so a merge conflict is small.

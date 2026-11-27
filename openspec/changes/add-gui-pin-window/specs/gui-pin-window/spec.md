## Purpose

Lets the user dock the running GUI window at a side edge of the output as a pinned window beside the tiled windows, move it between its Queue view and its Full view with the mouse, and put it back as a normal window, all without a new process.

## ADDED Requirements

### Requirement: The header offers Pin and Unpin

The global header SHALL show one pin control beside the Settings control whenever pinning is available. While the window is not pinned, a press SHALL pin it at the `[panel] side` edge. While the window is pinned, the control SHALL show as Unpin, and a press SHALL put the window back as a normal compositor window. The header SHALL show the control in every arrangement.

#### Scenario: Pin from a tiled window

- **GIVEN** the GUI window is a normal tiled window and `[panel] side` is `right`
- **WHEN** the user presses the pin control
- **THEN** the window docks at the right output edge in the Queue view
- **AND** the control shows as Unpin

#### Scenario: Unpin

- **GIVEN** the GUI window is pinned
- **WHEN** the user presses Unpin
- **THEN** the window opens as a normal compositor window and the control shows as Pin

### Requirement: Pinning is available only with a layer-shell host

Pinning SHALL be available only when the GUI runs on a Wayland session whose compositor offers layer-shell and the GUI's panel host started. Otherwise the header SHALL show no pin control, and the GUI SHALL work as a normal window. A failed host start SHALL be logged once and SHALL show no toast.

#### Scenario: X11 session

- **WHEN** the GUI starts on an X11 session
- **THEN** the header shows no pin control and the window works as before

#### Scenario: No layer-shell

- **WHEN** the GUI starts on a Wayland compositor without layer-shell
- **THEN** the header shows no pin control and one warning is logged

### Requirement: Pin and Unpin keep the window state

Pin and Unpin SHALL keep the window's state: the active destination, the picked item, the scroll positions, the open overlays, the Queue panel's toggles, and the Library interaction state. Playback SHALL continue without a pause. The GUI SHALL NOT start a new process or attach to the Owner again.

#### Scenario: Pin while playing

- **GIVEN** an item plays and Music shows a picked album
- **WHEN** the user pins the window and then opens the Full view
- **THEN** playback continues without a pause and Music shows the same picked album

#### Scenario: Unpin keeps the Owner link

- **GIVEN** the GUI window is pinned and attached to the Owner process
- **WHEN** the user presses Unpin
- **THEN** the window stays attached to the same Owner connection and no new Client attaches

### Requirement: A refused pin leaves the window as it was

When the panel host refuses a pin, for example because the layout leaves the output no width, the window SHALL stay a normal window and a warning toast SHALL name the reason.

#### Scenario: Gutters leave no width

- **GIVEN** the `[panel]` gutters leave the output no width for a panel
- **WHEN** the user presses the pin control
- **THEN** the window stays a normal window and a warning toast names the reason

### Requirement: A pinned window has a Queue view and a Full view

A pinned window SHALL show one of two views. The Queue view SHALL show the Queue panel alone, and the window width SHALL be the Queue column width: the rail width while the panel is collapsed, otherwise the Queue column's dragged or default width. The Full view SHALL show the Library and the Queue panel. A pin SHALL start in the Queue view. The views SHALL take their widths from these rules and from no `[panel]` width key.

#### Scenario: Queue view width follows the Queue column

- **GIVEN** the user dragged the Queue column to 520 px in an earlier session
- **WHEN** the user pins the window
- **THEN** the window is 520 px wide and shows the Queue panel alone

#### Scenario: Pinned width keys have no effect

- **GIVEN** `[panel] width = 300` and `width_expanded = 900`
- **WHEN** the user pins the window and opens the Full view
- **THEN** neither view takes its width from those keys

### Requirement: The Full view width is learned

The Full view SHALL first open at the Queue column width plus the Wide arrangement's minimum Library column width. A drag on the window's inner edge in the Full view SHALL set the Full view width, and the GUI SHALL save it and use it for each later Full view, also after a relaunch. Reset UI State SHALL drop the saved Full view width.

#### Scenario: First Full view is Wide

- **GIVEN** no Full view width is saved and the output is tall enough for Wide
- **WHEN** the user opens the Full view
- **THEN** the Library shows the Hero pane beside the list

#### Scenario: A dragged Full view width is kept

- **GIVEN** the user dragged the Full view to 1500 px
- **WHEN** the user returns to the Queue view and opens the Full view again, in this or a later session
- **THEN** the Full view is 1500 px wide

### Requirement: The header moves between the views

In a pinned window, a press on a destination tab, the Home icon, or the Playlists icon in the Queue view SHALL open the Full view with that destination active. In the Full view, a press on the active destination's own control SHALL return to the Queue view. A press on another destination in the Full view SHALL make it active and keep the Full view.

#### Scenario: A tab opens the Full view

- **GIVEN** the pinned window shows the Queue view
- **WHEN** the user presses the Shows tab
- **THEN** the window grows to the Full view with Shows active

#### Scenario: The active tab returns to the Queue view

- **GIVEN** the pinned window shows the Full view with Shows active
- **WHEN** the user presses the Shows tab
- **THEN** the window returns to the Queue view

#### Scenario: Another tab keeps the Full view

- **GIVEN** the pinned window shows the Full view with Shows active
- **WHEN** the user presses the Music tab
- **THEN** Music becomes active and the Full view stays

### Requirement: A play from the Full view returns to the Queue view

When a user action in the pinned Full view starts playback now, the window SHALL return to the Queue view after the Owner accepts the queue replacement, at most one time per action and for every output. Enqueue, a refused or cancelled play, and a remote-control Play SHALL NOT change the view.

#### Scenario: Play returns to the Queue view

- **GIVEN** the pinned window shows the Full view
- **WHEN** the user plays an album from the Library
- **THEN** playback starts and the window returns to the Queue view

#### Scenario: Enqueue keeps the Full view

- **GIVEN** the pinned window shows the Full view
- **WHEN** the user enqueues an item
- **THEN** the item appends and the Full view stays

#### Scenario: A remote Play keeps the Full view

- **GIVEN** the pinned window shows the Full view
- **WHEN** an Emby remote-control session sends a Play command that plays now
- **THEN** playback starts and the Full view stays

### Requirement: The Full view closes when the keyboard leaves the window

While the pinned window shows the Full view, a loss of the window's keyboard focus SHALL return it to the Queue view. A focus loss in the Queue view or in the rail SHALL change nothing. A focus change that the GUI's own re-dock causes SHALL change no view.

#### Scenario: A click on a tiled window

- **GIVEN** the pinned window shows the Full view over a tiled window
- **WHEN** the user clicks the part of the tiled window that shows beside the panel
- **THEN** the pinned window returns to the Queue view

#### Scenario: A focus loss in the Queue view

- **GIVEN** the pinned window shows the Queue view
- **WHEN** the user clicks a tiled window
- **THEN** the pinned window keeps the Queue view

### Requirement: A pinned Queue view collapses to the rail

In a pinned window, the collapse toggle SHALL shrink the window to the rail width, and an expand SHALL restore the Queue column width that the window had before the collapse. The Full view SHALL return to the Queue view before it collapses. The global header SHALL NOT show while the pinned window is at the rail width.

#### Scenario: Collapse shrinks the window

- **GIVEN** the pinned window shows the Queue view at 400 px
- **WHEN** the user presses the footer's collapse control
- **THEN** the window shrinks to the rail width and shows the rail with no header

#### Scenario: Expand restores the width

- **GIVEN** the pinned window shows the rail and was 520 px wide before the collapse
- **WHEN** the user presses the rail's expand control
- **THEN** the window grows to 520 px and shows the Queue view

### Requirement: The inner edge resizes the pinned window

The pinned window's edge away from the docked output edge SHALL be a resize handle in the Queue view and the Full view. A drag SHALL change the window width live. In the Queue view it SHALL set the Queue column width within that column's limits and save it as a divider drag does. In the Full view it SHALL set the Full view width. The rail SHALL have no resize handle.

#### Scenario: Drag the Queue view wider

- **GIVEN** the pinned window is docked at the left edge in the Queue view at 400 px
- **WHEN** the user drags the window's right edge 100 px to the right
- **THEN** the window and the Queue column are 500 px wide, and the width is saved

#### Scenario: The rail has no handle

- **GIVEN** the pinned window shows the rail
- **WHEN** the user presses and drags the rail's inner edge
- **THEN** the window width does not change

### Requirement: The covering follows the view

The pinned Queue view SHALL reserve a strip beside it so that the tiled windows move aside while `[panel] reserve` is on, and SHALL draw over the tiled windows while it is off. The Full view SHALL always draw over the tiled windows and hold the strip that the Queue view reserved, so a change between the views moves no tiled window.

#### Scenario: Open the Full view

- **GIVEN** `[panel] reserve` is on and the pinned window shows the Queue view
- **WHEN** the user opens the Full view
- **THEN** the window covers the tiled windows and no tiled window moves

#### Scenario: Reserve off

- **GIVEN** `[panel] reserve` is off
- **WHEN** the user pins the window
- **THEN** the window draws over the tiled windows and no tiled window moves

### Requirement: The pinned window docks at the configured side

The pinned window SHALL dock at the `[panel] side` edge, left or right, with the `[panel]` gutters. The GUI's column order SHALL NOT change with the side. A view change and a collapse SHALL animate when the window docks at the right edge and SHALL apply in one step when it docks at the left edge.

#### Scenario: Left dock keeps the column order

- **GIVEN** `[panel] side` is `left`
- **WHEN** the pinned window opens the Full view
- **THEN** the Library is at the output's left edge and the Queue panel is to its right

#### Scenario: Left dock changes in one step

- **GIVEN** `[panel] side` is `left` and the pinned window shows the Queue view
- **WHEN** the user opens the Full view
- **THEN** the window takes the Full view width in one step with no animation

### Requirement: The pinned state persists across launches

At an orderly exit the GUI SHALL save whether its window was pinned. At start, when the saved state is pinned and pinning is available, the first window SHALL open pinned in the Queue view. When pinning is not available, the window SHALL open as a normal window and the saved state SHALL stay as it was.

#### Scenario: Start pinned

- **GIVEN** the GUI exited while pinned
- **WHEN** the user starts the GUI again on the same session
- **THEN** the window opens pinned in the Queue view

#### Scenario: Start on X11 after a pinned exit

- **GIVEN** the GUI exited while pinned on Wayland
- **WHEN** the user starts the GUI on an X11 session
- **THEN** the window opens as a normal window

### Requirement: Panel settings apply live while pinned

While the window is pinned, a change to Side, Reserve space, or a gutter SHALL apply to the running window and SHALL be saved only when the panel host accepts it. A refused change SHALL show a warning toast with the reason and save nothing. A Reserve space change in the Full view SHALL be saved and apply at the next Queue view. An accent change SHALL be saved and apply at the next pin, with a neutral toast that says so.

#### Scenario: Side change while pinned

- **GIVEN** the pinned window is docked at the right edge
- **WHEN** the user changes Side to left in the Settings screen
- **THEN** the window moves to the left edge and the value is saved

#### Scenario: Refused gutter

- **GIVEN** the window is pinned
- **WHEN** the user steps a gutter to a value that leaves the output no width
- **THEN** a warning toast names the reason, the window does not change, and the value is not saved

#### Scenario: Accent change while pinned

- **GIVEN** the window is pinned
- **WHEN** the user turns the accent off
- **THEN** the value is saved, the window does not change, and a neutral toast says it applies at the next pin

### Requirement: The pinned GUI is separate from the TUI pinned panel

The pinned GUI window SHALL NOT count as a pinned Client for the Owner process. The Tray's Pin swap item, a Tray left click, and `mbv-tui --toggle` SHALL NOT act on it. The GUI SHALL need no `pinwin` program at run time.

#### Scenario: Tray with only a pinned GUI attached

- **GIVEN** only a pinned GUI window is attached to the Owner process
- **WHEN** the user opens the Tray menu
- **THEN** the menu shows Pin, and choosing it starts `mbv-tui --pin` and ends no Client

#### Scenario: No pinwin program installed

- **GIVEN** the `pinwin` program is not installed
- **WHEN** the user pins the GUI window on a compositor with layer-shell
- **THEN** the window docks

## Purpose

Defines context-menu placement, presentation, and input ownership so the menu
behaves as one consistent modal surface across every view that supports it.

## ADDED Requirements

### Requirement: Keyboard-triggered menu anchors to the selected item
When the context menu is opened via the keyboard shortcut from Home, an Emby
browse view, or the queue, its position SHALL be derived from the fresh-frame
screen rectangle of the currently selected row or grid cell in the focused
panel.

The menu's right edge SHALL align with the selected item's right edge whenever
the rendered menu fits horizontally at that position. The menu SHALL open with
its top edge aligned to the selected item's top edge when its full rendered
height fits below that edge in the containing panel. Otherwise it SHALL open
upward with its bottom edge aligned to the selected item's bottom edge.

After choosing the preferred position, the menu SHALL be clamped inside the
containing panel when exact alignment would place it outside that panel. If the
menu itself exceeds the available panel dimension, the panel edge SHALL win and
the terminal renderer MAY clip the unavoidable overflow.

#### Scenario: Selected item near the top of the panel
- **WHEN** the shortcut is pressed and the menu fits from the selected item's
  top edge to the panel's bottom edge
- **THEN** the menu opens downward with its top-right corner aligned to the
  selected item's top-right corner

#### Scenario: Selected item near the bottom of the panel
- **WHEN** the shortcut is pressed and the menu does not fit below but does fit
  above the selected item
- **THEN** the menu opens upward with its bottom-right corner aligned to the
  selected item's bottom-right corner

#### Scenario: Preferred alignment exceeds a panel edge
- **WHEN** the preferred placement would extend outside the containing panel
- **THEN** the menu position is clamped to that panel's nearest edge

#### Scenario: Selected item is a grid cell
- **WHEN** a supported Emby view presents selectable items in multiple columns
  and the shortcut is pressed
- **THEN** the menu anchors to the selected cell's actual on-screen rectangle,
  not the full row or panel width

#### Scenario: Nested detail content is present
- **WHEN** the selected row or cell expands or renders nested hero/detail
  content
- **THEN** the outer selectable row or cell remains the menu anchor and nested
  content does not replace it

#### Scenario: Unsupported destination is active
- **WHEN** Audiobookshelf or Feeds is selected and the shortcut is pressed
- **THEN** no context menu opens

### Requirement: Mouse-triggered menu retains pointer anchoring
When a context menu is opened by mouse, its anchor SHALL be the click position
and SHALL NOT depend on selected-item geometry. Placement SHALL be recalculated
while rendering and clamped to remain visible using the same bounds policy as a
keyboard-triggered menu.

#### Scenario: Mouse-triggered menu opens
- **WHEN** a supported item is right-clicked
- **THEN** the menu opens from the click position rather than the selected
  item's keyboard anchor

### Requirement: Dim backdrop while the menu is open
While the context menu is open, content behind it SHALL use the application's
existing dim-backdrop treatment. Image rendering SHALL use the same half-block
modal path selected by other dimmed modal surfaces.

#### Scenario: Menu opens over visible content
- **WHEN** the context menu opens by keyboard or mouse
- **THEN** background text and images are dimmed for as long as the menu remains
  open, while the menu itself remains undimmed

### Requirement: Open menu exclusively owns keyboard input
While the context menu is open, it SHALL claim every keyboard event before all
other key handlers. Up and Down SHALL move the highlight to the previous or
next selectable entry, skipping separators and wrapping at the ends. Enter
SHALL execute the highlighted action after closing the menu. Esc SHALL close
the menu without acting. Every other key SHALL be swallowed.

#### Scenario: Move the highlighted entry
- **WHEN** Up or Down is pressed while the menu is open
- **THEN** the highlight moves in that direction, skips non-selectable entries,
  and wraps among selectable entries

#### Scenario: Execute the highlighted entry
- **WHEN** Enter is pressed while the menu is open
- **THEN** the menu closes and the highlighted entry's action executes exactly
  once

#### Scenario: Dismiss without acting
- **WHEN** Esc is pressed while the menu is open
- **THEN** the menu closes and no entry's action executes

#### Scenario: Another shortcut is pressed
- **WHEN** any other key is pressed, including F1-F4, Ctrl+/, Tab, BackTab,
  numeric tab selection, refresh, playback, or view-action keys
- **THEN** the key is swallowed and neither the underlying view nor another
  overlay changes

### Requirement: Only one modal surface is active
The context menu SHALL NOT coexist with another modal or sidebar surface. A
context-menu open request while another such surface is active SHALL do
nothing. A mandatory modal activated asynchronously while the context menu is
open SHALL close and replace the context menu before rendering or handling
input.

#### Scenario: Shortcut is pressed over an existing overlay
- **WHEN** another modal or sidebar surface is active and the context-menu
  shortcut is pressed
- **THEN** no context menu opens and the existing surface remains active

#### Scenario: Mandatory modal activates asynchronously
- **WHEN** a mandatory modal becomes active while a context menu is open
- **THEN** the context menu closes and only the mandatory modal remains active

### Requirement: Open menu owns mouse interaction
While the context menu is open, its existing menu-click behavior SHALL remain:
clicking an actionable entry closes the menu and executes that action, while
clicking outside closes it without acting. Other mouse events SHALL NOT mutate
or navigate the obscured view.

#### Scenario: Mouse wheel moves while menu is open
- **WHEN** a wheel event occurs while the context menu is open
- **THEN** the underlying selection and scroll state do not change

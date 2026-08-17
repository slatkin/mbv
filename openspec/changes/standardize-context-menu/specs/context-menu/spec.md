## Purpose

Defines where the context menu appears when opened, how it is visually set
apart from the content behind it, and how the keyboard interacts with it
once open, so behavior is consistent across every view instead of being
re-derived per view.

## ADDED Requirements

### Requirement: Keyboard-triggered menu anchors to the selected item
When the context menu is opened via the keyboard shortcut, its position
SHALL be derived from the screen rectangle of the currently selected item
(row or grid cell) in the focused panel, not from a value independently
tracked per view.

The menu's right edge SHALL align with the selected item's right edge.

If the menu's full height fits within the visible area below the selected
item's top edge, the menu SHALL open downward with its top edge aligned to
the selected item's top edge. Otherwise, the menu SHALL open upward with
its bottom edge aligned to the selected item's bottom edge.

#### Scenario: Selected item near the top of the panel
- **WHEN** the context menu shortcut is pressed and the menu's height fits
  between the selected item's top edge and the bottom of the visible area
- **THEN** the menu opens below the selected item, with its top-right
  corner at the selected item's top-right corner

#### Scenario: Selected item near the bottom of the panel
- **WHEN** the context menu shortcut is pressed and the menu's height does
  not fit between the selected item's top edge and the bottom of the
  visible area
- **THEN** the menu opens above the selected item, with its bottom-right
  corner at the selected item's bottom-right corner

#### Scenario: Selected item is a grid cell, not a full-width row
- **WHEN** the focused panel presents selectable items in a multi-column
  layout (e.g. a two-column track list) and the context menu shortcut is
  pressed
- **THEN** the menu anchors to the selected cell's own rectangle (its
  actual on-screen column position and width), not the full panel width

#### Scenario: Mouse-triggered menu is unaffected
- **WHEN** the context menu is opened by a mouse click (e.g. right-click)
- **THEN** the menu opens at the click position, unaffected by the
  selected-item anchoring rule above

### Requirement: Dim backdrop while the menu is open
While the context menu is open, the screen content behind it SHALL be
dimmed, consistent with every other modal overlay in the application.

#### Scenario: Menu opens over visible content
- **WHEN** the context menu opens, by keyboard or mouse
- **THEN** the previously rendered screen content outside the menu is
  dimmed for as long as the menu remains open

### Requirement: Keyboard navigation of an open menu
While the context menu is open, the keyboard SHALL control it directly:
moving the highlighted entry, executing the highlighted entry, and
dismissing the menu without executing anything.

#### Scenario: Move the highlighted entry
- **WHEN** the context menu is open and the user presses the down (or up)
  navigation key
- **THEN** the highlighted entry moves to the next (or previous)
  selectable entry, skipping non-selectable entries such as separators,
  and wrapping is consistent with other selectable lists in the app

#### Scenario: Execute the highlighted entry
- **WHEN** the context menu is open and the user presses the confirm key
- **THEN** the highlighted entry's action executes and the menu closes

#### Scenario: Dismiss without acting
- **WHEN** the context menu is open and the user presses the cancel key
- **THEN** the menu closes and no entry's action executes

#### Scenario: No other key handler acts while the menu is open
- **WHEN** the context menu is open
- **THEN** keys not bound to menu navigation, execution, or dismissal do
  not affect the view underneath the menu

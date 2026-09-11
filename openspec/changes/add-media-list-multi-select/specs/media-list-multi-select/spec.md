## Purpose

Lets users select several rows in any canonical media list with Explorer-style
mouse gestures or a vim-style visual mode, and apply one context-menu action to
the whole selection.

## ADDED Requirements

### Requirement: Every canonical list opens row menus through one path
Every destination that shows a canonical media list SHALL open its row context
menu from right-click on a row and from the keyboard menu key through the same
list-level entry path, with unchanged menu entries and anchoring for existing
menus. Feeds lists SHALL offer a row context menu.

#### Scenario: Right-click on a TV row
- **WHEN** the user right-clicks a row in the TV library list
- **THEN** the context menu opens anchored at the pointer with the same entries as before this change

#### Scenario: Feeds row menu
- **WHEN** the user presses the menu key on a Feeds entry row
- **THEN** a context menu opens for that entry

### Requirement: Selection is uniform across canonical lists
Multi-row selection SHALL behave identically in every canonical media list and
at every breakpoint variant (Wide, Inline, Grid). Selection SHALL be scoped to
one list and identified by stable item identity, not row position.

#### Scenario: Same gestures everywhere
- **WHEN** the user Ctrl+Clicks two rows in Home, then in a Music album list
- **THEN** both lists show two selected rows with the same visual treatment

#### Scenario: Refresh keeps surviving selection
- **WHEN** a list refreshes while three rows are selected and one of those items disappears
- **THEN** the two remaining items stay selected

#### Scenario: Breakpoint change keeps selection
- **WHEN** the terminal resizes across a breakpoint while rows are selected
- **THEN** the same items remain selected

### Requirement: Mouse gestures build a selection
Ctrl+Click SHALL toggle the clicked row in the selection; the first Ctrl+Click
with no selection SHALL select both the previously focused row and the clicked
row. Shift+Click SHALL select the contiguous range from the anchor row to the
clicked row. A plain Click SHALL clear the selection and select only the
clicked row. While mouse capture is enabled, mbv SHALL request that the
terminal deliver Shift+Click to the application, and SHALL release that
request when mouse capture is disabled.

#### Scenario: Ctrl+Click toggles
- **WHEN** row 2 is focused and the user Ctrl+Clicks row 5, then Ctrl+Clicks row 5 again
- **THEN** rows 2 and 5 are selected, then only row 2 is selected

#### Scenario: Shift+Click range
- **WHEN** the anchor is row 2 and the user Shift+Clicks row 6
- **THEN** rows 2 through 6 are selected

#### Scenario: Plain click clears
- **WHEN** rows are selected and the user clicks a row without modifiers
- **THEN** the selection is cleared and only the clicked row is focused

### Requirement: Keyboard visual mode builds a selection
`V` SHALL start a selection anchored at the focused row. While a selection is
active, cursor movement SHALL extend the contiguous range from the anchor,
`Space` SHALL toggle the focused row instead of play/pause, and `Esc` SHALL
clear the selection. Outside an active selection these keys SHALL keep their
existing meaning.

#### Scenario: Visual range by keyboard
- **WHEN** the user presses `V` on row 3 and then moves down twice
- **THEN** rows 3 through 5 are selected

#### Scenario: Space toggles only while selecting
- **WHEN** a selection is active and the user presses `Space`
- **THEN** the focused row toggles in the selection and playback is not paused

#### Scenario: Esc clears
- **WHEN** a selection is active and the user presses `Esc`
- **THEN** the selection is cleared and `Space` again toggles play/pause

### Requirement: Visual mode is indicated and dismissible
While a list has a selection of one or more rows entered through a
multi-select gesture, the status row SHALL show a visual-mode indicator with
the selected count and a clickable control that clears the selection.

#### Scenario: Indicator count
- **WHEN** four rows are selected
- **THEN** the status row shows a visual-mode indicator reading 4

#### Scenario: Clear by mouse
- **WHEN** the user clicks the indicator's clear control
- **THEN** the selection is cleared and the indicator disappears

### Requirement: Context menu acts on the whole selection
Opening the context menu on a selected row SHALL offer only actions valid for
every selected item, drawn from: Play, Shuffle, Add to Queue, Remove, Mark
Played, Mark Unplayed. Opening it on a row outside the selection SHALL clear
the selection and show that row's single-item menu. Running any selection
action SHALL clear the selection.

#### Scenario: Right-click inside selection
- **WHEN** three rows are selected and the user right-clicks one of them
- **THEN** the menu offers actions applying to all three items

#### Scenario: Right-click outside selection
- **WHEN** rows 2-4 are selected and the user right-clicks row 8
- **THEN** the selection is cleared and row 8's normal menu opens

### Requirement: Play, Shuffle and Add to Queue use list order
Play SHALL replace the queue with the selected items in their list order and
start the first; Shuffle SHALL do the same in random order; Add to Queue SHALL
append them in list order. Items the Player owner does not admit SHALL be
reported through existing enqueue feedback while admitted items proceed.

#### Scenario: Add to Queue order
- **WHEN** the user Ctrl+Clicks rows 5, then 2, then 9 and chooses Add to Queue
- **THEN** the queue gains rows 2, 5, 9 in that order

### Requirement: Remove is offered only for removable lists
Remove SHALL appear only when every selected item can be removed from the list
it is shown in (for example Continue Watching, or the Queue), and SHALL remove
all selected items. Library lists SHALL NOT offer Remove.

#### Scenario: Continue Watching bulk remove
- **WHEN** three Continue Watching rows are selected and the user chooses Remove
- **THEN** all three are removed from Continue Watching

#### Scenario: Library has no Remove
- **WHEN** rows in a Movies library list are selected
- **THEN** the menu does not offer Remove

### Requirement: Played state is set in bulk
Mark Played SHALL set every selected item to played and Mark Unplayed SHALL
set every selected item to unplayed, regardless of each item's prior state.
There SHALL be no per-item toggle action for a selection.

#### Scenario: Mixed selection marked played
- **WHEN** a selection contains two played and three unplayed items and the user chooses Mark Played
- **THEN** all five items are played

#### Scenario: Queue omits self-referential actions
- **WHEN** rows in the Queue are selected
- **THEN** the menu offers Remove, Mark Played and Mark Unplayed but not Play, Shuffle or Add to Queue

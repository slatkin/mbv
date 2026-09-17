## ADDED Requirements

### Requirement: The window raise keeps a group's label visible

When the shared media-list owner raises the visible window to bring the selection back into view,
it SHALL continue raising over the contiguous non-selectable rows (`Heading`/`Spacer`) directly
above the selection, so the `Heading` that labels the group containing the selection stays painted.
The raise SHALL NOT continue past the first selectable row above that label run. No other window
behaviour SHALL change: the wheel, `PgUp`/`PgDn`, cursor chords, restore, and clamping keep their
current meaning, and the window has no writer besides the existing selection-following rule.

#### Scenario: Scrolling back up re-shows the group heading

- **WHEN** a grouped list is scrolled down and the cursor is moved back up to a group's first
  selectable row, entering the window from above
- **THEN** the window's first row is that group's `Heading`
- **AND** the selection is painted directly below it

#### Scenario: The raise shows only the selection's own group label

- **WHEN** the raise walks the rows above the selection
- **THEN** it stops at the first selectable row above that label run
- **AND** the previous group's rows stay outside the window

#### Scenario: A raise with no label above is unchanged

- **WHEN** the selection enters the window from above and the row above it is selectable
- **THEN** the window top is the selection's display row, exactly as before this change

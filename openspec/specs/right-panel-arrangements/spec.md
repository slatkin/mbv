# right-panel-arrangements Specification

## Purpose

Defines the right panel's two responsive arrangements, which screens use each, and where the
responsive decision is made, so that the wide and narrow presentations can be changed independently
of one another and a new screen inherits a known arrangement without being individually designed.

## Requirements

### Requirement: The right panel has exactly two arrangements

The right panel SHALL assign each screen one of two wide arrangements. Hero-on-top places the hero
above the list. Hero-on-left places the hero beside the list, with the list in a single column.
Below the shared breakpoint, every library browse screen SHALL use the standard narrow one-column
presentation: the selected item's hero SHALL be rendered inline in the scrolling list at its active
row. Narrow library presentation SHALL NOT pin the hero in a separate area above the list, and SHALL
NOT be a per-library arrangement exception. Non-library screens retain their existing narrow
presentation.

#### Scenario: A library enters the narrow presentation

- **WHEN** a library browse screen's available width falls below the shared breakpoint
- **THEN** it renders one list column
- **AND** the selected item's hero renders inline in the list at the active row
- **AND** the hero does not reserve a separate area above the list

#### Scenario: A wide hero-on-top screen crosses the breakpoint

- **WHEN** a hero-on-top library screen's available width crosses below the breakpoint
- **THEN** its wide arrangement assignment remains hero-on-top
- **AND** its narrow presentation is the shared inline-hero one-column presentation

#### Scenario: A hero-on-top screen crosses the breakpoint

- **WHEN** a hero-on-top screen's available width crosses the breakpoint
- **THEN** its wide arrangement does not change
- **AND** its narrow library presentation uses one inline-hero column when it is a library browse
  screen

#### Scenario: A wide hero-on-left screen falls below the breakpoint

- **WHEN** a hero-on-left library screen's available width falls below the breakpoint
- **THEN** its wide arrangement assignment remains hero-on-left
- **AND** it renders the shared inline-hero one-column presentation

#### Scenario: A hero-on-left screen falls below the breakpoint

- **WHEN** a hero-on-left screen's available width falls below the breakpoint
- **THEN** a library browse screen renders the shared inline-hero presentation with a single list
  column

#### Scenario: Panel mode changes

- **WHEN** the user cycles Panel mode
- **THEN** the presentation is recomputed from the width the right panel is left with
- **AND** the selected wide arrangement or standard narrow presentation is otherwise unaffected

### Requirement: Each screen is assigned one wide arrangement

Every right-panel screen SHALL be assigned exactly one wide arrangement. TV shows and the dedicated
Movies library SHALL use hero-on-left. Podcasts, feeds, and home videos SHALL use hero-on-top.
Home, music, and audiobooks SHALL use hero-on-left. No right-panel screen SHALL be without an
assignment.

#### Scenario: Wide TV shows has an interactive left hero

- **WHEN** the TV shows library is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-left arrangement
- **AND** the selected Series detail, season pills, and persistent episode preview are on the left
- **AND** TV letter-range pills and the one-column Series list are in the right rail

#### Scenario: Movies is displayed at a wide width

- **WHEN** the dedicated Movies library is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-left arrangement
- **AND** the selected-media hero is on the left
- **AND** the letter-range pills and one-column Movies list are in the right rail

#### Scenario: Wide Movies has its selected-media hero

- **WHEN** the dedicated Movies library is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-left arrangement
- **AND** the selected-media hero is on the left
- **AND** the letter-range pills and one-column Movies list are in the right rail

#### Scenario: TV shows falls below the breakpoint

- **WHEN** the TV shows library's available width falls below the shared breakpoint
- **THEN** it falls back to hero-on-top with a single list column

#### Scenario: Movies falls below the shared breakpoint

- **WHEN** the Movies library's available width falls below the shared breakpoint
- **THEN** it falls back to hero-on-top with a single list column

#### Scenario: Feeds is displayed at a wide width

- **WHEN** the feeds screen is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-top arrangement with a two-column list

#### Scenario: Home videos is displayed at a wide width

- **WHEN** the home videos screen is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-top arrangement with a two-column list

#### Scenario: Audiobooks is displayed at a wide width

- **WHEN** an Audiobookshelf book library is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-left arrangement, matching music at the same dimensions

### Requirement: Screens do not determine their own arrangement

The responsive breakpoint SHALL be evaluated in one place, and its value SHALL be defined in one
place. An individual screen SHALL NOT test the available width to select an arrangement, a column
count, or any presentation that differs between arrangements. Code outside a screen SHALL NOT paint
part of that screen's arrangement.

#### Scenario: The breakpoint value is changed

- **WHEN** the breakpoint value is changed in its single definition
- **THEN** every right-panel screen changes arrangement at the new width
- **AND** no screen requires an individual edit

#### Scenario: One arrangement's presentation is changed

- **WHEN** the presentation of one arrangement is changed
- **THEN** the other arrangement is unaffected

### Requirement: Hero-on-left presents up to two focusable panes

The hero-on-left arrangement SHALL present up to two panes, of which at most one is focused, and
only while the right panel itself is focused. A screen with a read-only hero pane, such as Home or
the wide Movies library, SHALL expose only its right-hand list as focusable content. The wide TV
shows library SHALL expose the right-hand Series list and the left-hand episode workspace as
focusable content. While Series browsing is active, the left pane SHALL remain a projection of the
selected Series; when episode selection is active, the left pane SHALL receive focus.

#### Scenario: Wide Movies has Library focus

- **WHEN** the wide Movies library is displayed and the Library panel has focus
- **THEN** the right-hand Movies list is the focused pane
- **AND** the left selected-media hero remains read-only and does not become a second focus target

#### Scenario: Wide TV shows has Series-list focus

- **WHEN** the wide TV shows library is displayed and episode selection is inactive
- **THEN** the right-hand Series list is the focused pane
- **AND** the left Series and episode workspace renders as an unfocused preview

#### Scenario: Wide TV shows has episode focus

- **WHEN** episode selection is active in the wide TV shows library
- **THEN** the left-hand episode workspace is the focused pane
- **AND** the right-hand Series list renders its unfocused treatment

#### Scenario: Focus moves between panes

- **WHEN** the user moves focus within a hero-on-left screen that has focusable hero content
- **THEN** exactly one pane is focused and the other renders its unfocused appearance

#### Scenario: The right panel is unfocused

- **WHEN** the right panel is not focused
- **THEN** neither pane of a hero-on-left screen renders as focused

### Requirement: Per-screen presentation differences are declared in one place

Where a screen differs from the shared presentation, those differences SHALL be declared together in
a single place associated with that screen, rather than expressed at the points where the screen
renders. A screen that declares no differences SHALL receive the shared defaults. Declarations MAY
cover the source of the hero image, the hero image's shape, which metadata lines are shown and in
what order, the colour variant, and which elements are present. Declarations SHALL NOT cover
geometry, the breakpoint, or focus behaviour.

#### Scenario: A screen shows different metadata

- **WHEN** a screen's library provides metadata that differs from the default set
- **THEN** that difference is declared in the screen's single declaration
- **AND** the screen's rendering path contains no other expression of it

#### Scenario: A screen declares nothing

- **WHEN** a screen declares no differences
- **THEN** it renders with the shared defaults for its assigned arrangement

#### Scenario: A shared default changes

- **WHEN** a shared default changes
- **THEN** every screen that has not declared a difference for it renders the change

### Requirement: Mouse targets are produced by the arrangement

The arrangement SHALL produce the mouse hit targets for the content it draws, in one common form
shared by all right-panel screens. Hit-testing SHALL NOT require knowing which screen produced the
targets, and adding a screen SHALL NOT require adding a target representation or a hit-testing
branch.

#### Scenario: The user clicks an item row

- **WHEN** the user clicks an item row on any right-panel screen, in either arrangement
- **THEN** that item is resolved through the common hit targets

#### Scenario: A new screen is added

- **WHEN** a new right-panel screen is added using an existing arrangement
- **THEN** its rows and panes are clickable without new hit-testing code
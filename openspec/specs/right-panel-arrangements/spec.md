# right-panel-arrangements Specification

## Purpose

Defines the right panel's two responsive arrangements, which screens use each, and where the
responsive decision is made, so that the wide and narrow presentations can be changed independently
of one another and a new screen inherits a known arrangement without being individually designed.

## Requirements

### Requirement: The right panel has exactly two arrangements

The right panel SHALL present its content in one of two arrangements. Hero-on-top places the hero
above the list. Hero-on-left places the hero beside the list, with the list in a single column.
Narrow presentation SHALL be hero-on-top with a single list column; it SHALL NOT be a third
arrangement. These arrangements are a property of the right panel and are independent of Panel mode.

#### Scenario: A hero-on-top screen crosses the breakpoint

- **WHEN** a hero-on-top screen's available width crosses the breakpoint
- **THEN** its arrangement does not change
- **AND** only its list column count changes between one and two

#### Scenario: A hero-on-left screen falls below the breakpoint

- **WHEN** a hero-on-left screen's available width falls below the breakpoint
- **THEN** it renders as hero-on-top with a single list column

#### Scenario: Panel mode changes

- **WHEN** the user cycles Panel mode
- **THEN** the arrangement is recomputed from the width the right panel is left with, and is
  otherwise unaffected by which Panel mode is active

### Requirement: Each screen is assigned one wide arrangement

Every right-panel screen SHALL be assigned exactly one wide arrangement. Movies, TV shows, podcasts,
feeds, and home videos SHALL use hero-on-top. Home, music, and audiobooks SHALL use hero-on-left. No
right-panel screen SHALL be without an assignment.

#### Scenario: Feeds is displayed at a wide width

- **WHEN** the feeds screen is displayed at or above the breakpoint
- **THEN** it renders the hero-on-top arrangement with a two-column list

#### Scenario: Home videos is displayed at a wide width

- **WHEN** the home videos screen is displayed at or above the breakpoint
- **THEN** it renders the hero-on-top arrangement with a two-column list

#### Scenario: Audiobooks is displayed at a wide width

- **WHEN** the audiobooks screen is displayed at or above the breakpoint
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
only while the right panel itself is focused. A screen with no focusable content in its hero pane
(such as Home, whose hero is a non-focusable preview) SHALL be treated as having that content
unimplemented rather than as a different arrangement.

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
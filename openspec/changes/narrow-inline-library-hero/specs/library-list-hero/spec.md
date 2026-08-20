## MODIFIED Requirements

### Requirement: Hero area pinned above the list

The selected item's hero SHALL be positioned by the right-panel presentation rather than by a
library screen's ad hoc renderer. Wide hero-on-top SHALL place the hero above the list and reserve
the remaining area for the list. Wide hero-on-left SHALL place the hero beside a single-column list,
with the list's pills and rows in the right rail. Below the shared breakpoint, library screens SHALL
render the selected hero inline at the active row in the one-column scrolling list. The narrow inline
hero SHALL remain part of list flow, so its variable height SHALL not move content outside the active
row. The selected hero's content SHALL remain visible while the list scrolls far enough to include
the active row.

For the wide Movies library, the left hero SHALL use the exact selected-media card already used by
Home's wide hero-on-left Movies Latest presentation. The card SHALL use its existing image shape,
metadata order, watch-state indicator, overview treatment, and artwork behavior. The right rail
SHALL contain the Movies letter-range pills when eligible and the one-column Movies list.

Below the shared breakpoint, Movies SHALL use the shared narrow inline-hero presentation. A hero SHALL
be suppressed when the active presentation cannot fit at least one usable list row and the minimum
content required for the active row.

#### Scenario: Wide Movies renders the Home selected-media card

- **WHEN** a Movie is selected in the wide Movies list
- **THEN** the left pane renders the same selected-media card that Home renders for that Movie
- **AND** the right rail renders the letter-range pills when eligible
- **AND** the right rail renders the Movies list as one column

#### Scenario: Narrow library renders an inline hero

- **WHEN** a library browse screen is below the shared breakpoint and has a selected item
- **THEN** the selected item's hero renders inline at that item's active row
- **AND** the list remains a single scrolling column
- **AND** the hero uses the same content, artwork, metadata, and loading behavior declared for that
  library's hero

#### Scenario: Hero renders above the list

- **WHEN** a wide hero-on-top library view has a selected item
- **THEN** the hero banner renders in a fixed-height area above the list
- **AND** the list renders below it

#### Scenario: Narrow selection changes

- **WHEN** the cursor moves to another item in a narrow library
- **THEN** the inline hero moves to the newly active row
- **AND** the previous row returns to its ordinary non-hero presentation

#### Scenario: Narrow list has insufficient space

- **WHEN** a narrow library cannot fit the minimum active row and its inline hero content
- **THEN** the hero is suppressed
- **AND** the list retains the available content area

#### Scenario: Wide Movies falls back below the breakpoint

- **WHEN** the Movies library is below the shared breakpoint
- **THEN** the selected Movie uses the shared narrow inline-hero presentation
- **AND** the list renders as one column

#### Scenario: Narrow grouped Music uses the pinned hero

- **WHEN** grouped Music is below the shared wide-layout breakpoint
- **THEN** its selected album hero renders inline at its active row in the one-column album list

#### Scenario: Wide grouped Music uses its side hero

- **WHEN** grouped Music reaches the shared wide-layout breakpoint
- **THEN** its selected album hero renders to the left of its one-column album browser

#### Scenario: Wide Movies pills sit in the right rail

- **WHEN** the wide Movies view is eligible for letter-range pills
- **THEN** the pill row renders at the top of the right-hand list rail
- **AND** the list renders below that pill row rather than below the left hero

#### Scenario: Letter pills sit between hero and list

- **WHEN** a wide hero-on-top library view shows a letter-pill row
- **THEN** the pill row renders directly below the hero
- **AND** the list renders below the pill row

### Requirement: Hero tracks the current selection independent of scroll position

The hero SHALL always reflect the currently selected item. In a wide arrangement, its screen position
SHALL remain fixed while the list cursor moves. In the narrow inline presentation, its position SHALL
be the active row's position in the scrolling list, and list scrolling SHALL keep the active row and
its inline hero addressable together. For wide Movies, the right-hand list cursor SHALL remain the
sole source of the selected item projected into the left hero.

#### Scenario: Selection scrolled out of view

- **WHEN** the Movies cursor moves to an item whose row is scrolled off screen in the right rail
- **THEN** the left hero still updates to show that item
- **AND** the hero remains in the same left-pane position

#### Scenario: Narrow selection is scrolled

- **WHEN** the cursor moves through a narrow library and the active row crosses the visible list area
- **THEN** scrolling keeps the active row and its inline hero in the navigable list flow
- **AND** the hero follows the active row rather than remaining pinned to a screen edge

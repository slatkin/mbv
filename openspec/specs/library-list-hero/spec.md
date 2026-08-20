# library-list-hero Specification

## Purpose

Gives the library list a persistent, always-visible detail view of the
selected item — a compact banner pinned above the list — instead of
splicing detail rows inline with the scrolling list content.
## Requirements
### Requirement: Hero area pinned above the list

The selected item's hero SHALL be positioned by the right-panel arrangement rather than being
intrinsically pinned above the list. Hero-on-top SHALL place the hero above the list and reserve the
remaining area for the list. Hero-on-left SHALL place the hero beside a single-column list, with the
list's pills and rows in the right rail. The selected hero SHALL remain visible while the list
scrolls.

For the wide Movies library, the left hero SHALL use the exact selected-media card already used by
Home's wide hero-on-left Movies Latest presentation. The card SHALL use its existing image shape,
metadata order, watch-state indicator, overview treatment, and artwork behavior. The right rail
SHALL contain the Movies letter-range pills when eligible and the one-column Movies list.

Below the shared breakpoint, Movies SHALL use the hero-on-top single-column fallback and its existing
narrow arrangement. A hero SHALL be suppressed when the active arrangement cannot fit a valid hero
and usable list area.

#### Scenario: Wide Movies renders the Home selected-media card

- **WHEN** a Movie is selected in the wide Movies list
- **THEN** the left pane renders the same selected-media card that Home renders for that Movie
- **AND** the right rail renders the letter-range pills when eligible
- **AND** the right rail renders the Movies list as one column

#### Scenario: Hero renders above the list

- **WHEN** a hero-on-top library view has a selected item
- **THEN** the hero banner for that item renders in a fixed-height area at the top of the content
  area
- **AND** the list renders below it

#### Scenario: Movies falls back below the breakpoint

- **WHEN** the Movies library is below the shared breakpoint
- **THEN** the selected hero renders above the list using the existing hero-on-top fallback
- **AND** the list renders as one column

#### Scenario: Narrow grouped Music uses the pinned hero

- **WHEN** grouped Music is below the shared wide-layout breakpoint
- **THEN** its selected album hero renders above its one-column album list

#### Scenario: Wide grouped Music uses its side hero

- **WHEN** grouped Music reaches the shared wide-layout breakpoint
- **THEN** its selected album hero moves to the left of its one-column album browser as defined by
  `music-library-hero`

#### Scenario: Hero suppressed when too little space remains

- **WHEN** the active arrangement cannot fit the hero's minimum block and at least one usable list
  row
- **THEN** the hero area collapses to zero height
- **AND** the list uses the available content area

#### Scenario: Wide Movies pills sit in the right rail

- **WHEN** the wide Movies view is eligible for letter-range pills
- **THEN** the pill row renders at the top of the right-hand list rail
- **AND** the list renders below that pill row rather than below the left hero

#### Scenario: Letter pills sit between hero and list

- **WHEN** a hero-on-top library view shows a letter-pill row
- **THEN** the pill row renders directly below the hero with no additional gap
- **AND** the list renders below the pill row

### Requirement: Selected cell indicator

The selected cell in any list SHALL be identified by the unified selection marker — a thin AQUA
block at the list's outer edge, directional in two-column mode (`▎` at the left column's left edge,
`▏` at the right column's right edge) — rather than by a `▌` left-edge mark and a `##` title prefix.
The `▌` mark and `##` prefix SHALL NOT appear on any selected cell. The cell's background SHALL use
the ordinary list background, not the media-selected background — that treatment is reserved for the
hero.

#### Scenario: Selected cell marked without a background change

- **WHEN** a cell in a list is the current selection
- **THEN** it shows the thin AQUA edge marker at its list edge, with the list's ordinary
  (non-selected) background
- **AND** it does NOT show a `▌` mark or a `##` title prefix

### Requirement: Hero tracks the current selection independent of scroll position

The hero SHALL always reflect the currently selected item, regardless of whether that item's row is
scrolled into view within the list area. The hero's own screen position SHALL NOT change when the
cursor moves; only its content changes. For wide Movies, the right-hand list cursor SHALL be the
sole source of the selected item projected into the left hero.

#### Scenario: Selection scrolled out of view

- **WHEN** the Movies cursor moves to an item whose row is scrolled off screen in the right rail
- **THEN** the left hero still updates to show that item
- **AND** the hero remains in the same left-pane position

### Requirement: Hero click focuses without activating

For hero-on-top library views, a single click inside the hero area SHALL focus the Library panel only,
and a double click SHALL retain the existing activation behavior. A read-only hero-on-left preview,
including the wide Movies hero, SHALL not receive focus or activation from a pointer gesture; the
right-hand list remains the interaction surface.

#### Scenario: Wide Movies hero remains read-only

- **WHEN** the wide Movies hero is displayed
- **THEN** it has no keyboard focus state and no activation action
- **AND** activating the selected Movie is performed from the right-hand list

#### Scenario: Single click on the hero

- **WHEN** a user single-clicks inside a hero-on-top hero area
- **THEN** the Library panel gains focus and no item is activated

#### Scenario: Double click on the hero

- **WHEN** a user double-clicks inside a hero-on-top hero area
- **THEN** the selected item is activated the same as a double-click on its list row

#### Scenario: Hero-on-top activation remains unchanged

- **WHEN** a user clicks a hero-on-top Movie or Series hero
- **THEN** the existing single-click focus and double-click activation behavior remains in effect

### Requirement: Column-count invariant preserved

The library list SHALL remain one renderer parameterized by column
count: the list area at a 1-column width and the equivalent 2-column
width SHALL render the same per-cell content, modulo cell-width
truncation and the right cell's trailing-column absorption.

#### Scenario: Same content at the 1-col/2-col boundary

- **WHEN** the list area is rendered once at a width just below the
  two-column threshold and once at a width just above it
- **THEN** each cell's content matches between the two renders, aside
  from truncation and trailing-column absorption differences

### Requirement: Independence from top-hero design

Hero presentation SHALL be defined by the selected screen and its assigned arrangement. The wide
Movies hero-on-left presentation SHALL reuse the exact Home wide selected-media card rather than
maintaining a second Movies-specific left-card implementation. The existing narrow Movies
hero-on-top fallback may retain its arrangement-specific presentation; changing the wide card SHALL
not require changing that fallback.

#### Scenario: Home and wide Movies use one selected-media card

- **WHEN** the same Movie is selected in Home's Movies Latest section and in the wide Movies library
- **THEN** the hero card uses the same image selection, layout, metadata, watch-state indicator,
  overview treatment, and image cache behavior

#### Scenario: Hero content remains consistent

- **WHEN** an item is shown in the hero-on-top hero
- **THEN** its image, metadata, and overview match the content declared for that screen and
  arrangement

#### Scenario: Placement changes

- **WHEN** the arrangement places the hero differently
- **THEN** the hero content remains the declared content for the selected screen
- **AND** only its position and arrangement-specific placement change

#### Scenario: Wide Movies card changes centrally

- **WHEN** the shared Home selected-media card presentation changes
- **THEN** the wide Movies hero renders that change without a second Movies-card edit



## MODIFIED Requirements

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

For the wide TV shows library, the left pane SHALL show the selected Series' artwork, metadata,
overview, season pills, and the current season's episodes. The right rail SHALL contain eligible TV
letter-range pills or active search and the one-column Series list. The season pills SHALL filter
only the left-pane episode list.

Below the shared breakpoint, Movies and TV shows SHALL use their existing hero-on-top single-column
fallbacks. A hero SHALL be suppressed when the active arrangement cannot fit a valid hero and usable
list area.

#### Scenario: Wide TV shows renders the selected Series workspace

- **WHEN** a Series is selected in the wide TV shows list
- **THEN** the left pane renders that Series' artwork, metadata, and overview
- **AND** the current season's episodes remain visible in the left pane
- **AND** the season pill bar appears above the episode list when season data is available
- **AND** the right rail renders eligible TV letter-range pills or active search
- **AND** the right rail renders the Series list as one column

#### Scenario: Wide Movies renders the Home selected-media card

- **WHEN** a Movie is selected in the wide Movies list
- **THEN** the left pane renders the same selected-media card that Home renders for that Movie
- **AND** the right rail renders the letter-range pills when eligible
- **AND** the right rail renders the Movies list as one column

#### Scenario: Wide TV season selection filters episodes

- **WHEN** the user selects another season for the selected Series
- **THEN** the left-pane season pill selection changes
- **AND** the left-pane episode list shows episodes from that season only
- **AND** the right-hand Series list remains a Series browser

#### Scenario: Hero renders above the list

- **WHEN** a hero-on-top library view has a selected item
- **THEN** the hero banner for that item renders in a fixed-height area at the top of the content
  area
- **AND** the list renders below it

#### Scenario: Movies falls back below the breakpoint

- **WHEN** the Movies library is below the shared breakpoint
- **THEN** the selected hero renders above the list using the existing hero-on-top fallback
- **AND** the list renders as one column

#### Scenario: TV shows falls back below the breakpoint

- **WHEN** the TV shows library is below the shared breakpoint
- **THEN** the selected Series renders using the existing hero-on-top TV presentation
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

#### Scenario: Wide TV pills sit in separate rails

- **WHEN** the wide TV shows view has both eligible library letter pills and season data for the
  selected Series
- **THEN** the letter-range pills render at the top of the right-hand Series rail
- **AND** the season pills render in the left Series workspace above its episode list

#### Scenario: Wide Movies pills sit in the right rail

- **WHEN** the wide Movies view is eligible for letter-range pills
- **THEN** the pill row renders at the top of the right-hand list rail
- **AND** the list renders below that pill row rather than below the left hero

#### Scenario: Letter pills sit between hero and list

- **WHEN** a hero-on-top library view shows a letter-pill row
- **THEN** the pill row renders directly below the hero with no additional gap
- **AND** the list renders below the pill row

### Requirement: Hero tracks the current selection independent of scroll position

The hero SHALL always reflect the currently selected item, regardless of whether that item's row is
scrolled into view within the list area. The hero's own screen position SHALL NOT change when the
cursor moves; only its content changes. For wide Movies, the right-hand list cursor SHALL be the sole
source of the selected item projected into the left hero. For wide TV shows, the right-hand Series
cursor SHALL be the sole source of the selected Series projected into the left workspace while
Series-list browsing is active.

#### Scenario: TV selection scrolled out of view

- **WHEN** the TV Series cursor moves to a Series whose row is scrolled out of the visible right rail
- **THEN** the left workspace updates to show that Series
- **AND** its season pills and episode preview update to that Series
- **AND** the left workspace remains in the same position

#### Scenario: Selection scrolled out of view

- **WHEN** the Movies cursor moves to an item whose row is scrolled off screen in the right rail
- **THEN** the left hero still updates to show that item
- **AND** the hero remains in the same left-pane position

#### Scenario: Episode selection does not change the projected Series

- **WHEN** an episode is selected in the left workspace
- **THEN** the left workspace continues to show the same Series detail
- **AND** the right Series-list cursor remains unchanged

### Requirement: Hero click focuses without activating

For hero-on-top library views, a single click inside the hero area SHALL focus the Library panel only,
and a double click SHALL retain the existing activation behavior. A read-only hero-on-left preview,
including the wide Movies hero, SHALL not receive focus or activation from a pointer gesture. The
wide TV episode workspace SHALL accept clicks on episode rows and season pills as navigation targets,
while artwork and non-interactive blank space SHALL not activate playback.

#### Scenario: Wide Movies hero remains read-only

- **WHEN** the wide Movies hero is displayed
- **THEN** it has no keyboard focus state and no activation action
- **AND** activating the selected Movie is performed from the right-hand list

#### Scenario: Wide TV episode row click

- **WHEN** a user clicks a visible episode row in the wide TV left workspace
- **THEN** that episode becomes selected
- **AND** episode selection becomes active without changing the Series-list cursor

#### Scenario: Wide TV season pill click

- **WHEN** a user clicks a season pill in the wide TV left workspace
- **THEN** that season becomes active
- **AND** the episode list refreshes to that season
- **AND** no episode is played by the pill click alone

#### Scenario: Wide TV artwork click

- **WHEN** a user clicks Series artwork or blank non-interactive space in the left workspace
- **THEN** no episode is selected or played

#### Scenario: Single click on the hero

- **WHEN** a user single-clicks inside a hero-on-top hero area
- **THEN** the Library panel gains focus and no item is activated

#### Scenario: Double click on the hero

- **WHEN** a user double-clicks inside a hero-on-top hero area
- **THEN** the selected item is activated the same as a double-click on its list row

#### Scenario: Single click on a hero-on-top hero

- **WHEN** a user single-clicks inside a hero-on-top hero area
- **THEN** the Library panel gains focus and no item is activated

#### Scenario: Double click on a hero-on-top hero

- **WHEN** a user double-clicks inside a hero-on-top hero area
- **THEN** the selected item is activated the same as a double-click on its list row

#### Scenario: Hero-on-top activation remains unchanged

- **WHEN** a user clicks a hero-on-top Movie or Series hero
- **THEN** the existing single-click focus and double-click activation behavior remains in effect

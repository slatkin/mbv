## RENAMED Requirements

- FROM: `### Requirement: The right panel has exactly two arrangements`
  TO: `### Requirement: The right panel has exactly two hero presentations`

## MODIFIED Requirements

### Requirement: The right panel has exactly two hero presentations

The right panel SHALL provide exactly two responsive hero presentations for every hero-bearing browse surface. At or above the shared breakpoint, when the existing minimum-height guard is satisfied, the surface SHALL use hero-on-left: the selected hero or detail workspace occupies the left pane and a single-column browser occupies the right rail. Otherwise the surface SHALL use selected-row replacement: the selected item's ordinary row is replaced by its variable-height detail block in the single-column scrolling browser.

A separate detail block SHALL NOT be an arrangement or fallback. A surface SHALL NOT reserve a hero in a separate full-width area above its browser. Non-hero screens retain their existing presentation.

#### Scenario: A browse surface enters the narrow presentation
- **WHEN** a hero-bearing browse surface's available width falls below the shared breakpoint
- **THEN** it renders one browser column
- **AND** the selected item's ordinary row is replaced by inline detail at the same flow position
- **AND** no separate hero area is reserved above the browser

#### Scenario: Wide geometry has insufficient height
- **WHEN** a hero-bearing browse surface meets the shared width breakpoint but fails the existing minimum-height guard
- **THEN** it uses selected-row replacement
- **AND** it restores the ordinary selected row if detail cannot fit

#### Scenario: A browse surface enters the wide presentation
- **WHEN** a hero-bearing browse surface meets the shared width and minimum-height conditions
- **THEN** it renders hero-on-left
- **AND** its browser is a single-column right rail

#### Scenario: Panel mode changes
- **WHEN** the user cycles Panel mode
- **THEN** the presentation is recomputed from the width and height available to the right panel
- **AND** the same shared breakpoint and minimum-height guard apply

#### Scenario: A library enters the narrow presentation
- **WHEN** a library browse surface does not meet the shared wide geometry conditions
- **THEN** it renders one list column with selected detail inline at the active row

#### Scenario: A formerly separate-detail surface crosses the breakpoint
- **WHEN** a formerly separate-detail surface crosses below the shared breakpoint
- **THEN** it uses selected-row replacement and retains no separate detail assignment

#### Scenario: A formerly separate-detail surface crosses the breakpoint
- **WHEN** a formerly separate-detail surface crosses the shared breakpoint in either direction
- **THEN** it switches only between hero-on-left and selected-row replacement

#### Scenario: A wide hero-on-left screen falls below the breakpoint
- **WHEN** a hero-on-left surface crosses below the shared breakpoint
- **THEN** it renders selected-row replacement with one browser column

#### Scenario: A hero-on-left screen falls below the breakpoint
- **WHEN** a hero-on-left surface no longer meets either wide geometry condition
- **THEN** it renders selected-row replacement

### Requirement: Each screen is assigned one wide arrangement

Every hero-bearing right-panel browse surface SHALL use hero-on-left for its wide presentation. This includes Home, Movies, TV shows, grouped Music, Emby podcasts, Emby home videos, Audiobookshelf podcasts, Audiobookshelf books, and Feeds. A read-only selected-item hero SHALL remain a projection of the right-hand browser selection. A surface whose left detail workspace contains episodes, tracks, or chapters MAY expose that existing interactive content without changing the shared placement rule. No hero-bearing browse surface SHALL declare a separate detail placement or a surface-specific responsive placement.

#### Scenario: Wide read-only hero surface
- **WHEN** Home, Movies, an Emby home-video library, or Feeds is displayed with wide geometry
- **THEN** the selected-item hero renders in the left pane
- **AND** the right rail remains the only focusable browser pane

#### Scenario: Wide interactive detail surface
- **WHEN** TV shows, grouped Music, an Emby podcast library, an Audiobookshelf podcast library, or an Audiobookshelf book library is displayed with wide geometry
- **THEN** the selected item's persistent detail workspace renders in the left pane
- **AND** the single-column catalog browser renders in the right rail
- **AND** existing episode, track, or chapter focus behavior remains available where that surface already provides it

#### Scenario: Movies is displayed at a wide width
- **WHEN** the dedicated Movies library meets the wide geometry conditions
- **THEN** the selected-media hero is on the left
- **AND** the letter-range pills and one-column Movies list are in the right rail

#### Scenario: TV shows is displayed at a wide width
- **WHEN** the TV shows library meets the wide geometry conditions
- **THEN** the selected Series detail, season pills, and persistent episode preview are on the left
- **AND** TV letter-range pills and the one-column Series list are in the right rail

#### Scenario: Feeds is displayed at a wide width
- **WHEN** Feeds meets the wide geometry conditions
- **THEN** the selected entry's hero is on the left
- **AND** group and watched selectors plus the one-column entry browser are in the right rail

#### Scenario: Audiobookshelf podcast library is displayed at a wide width
- **WHEN** an Audiobookshelf podcast library meets the wide geometry conditions
- **THEN** the selected show and its filtered episode workspace are on the left
- **AND** the one-column podcast-show browser is in the right rail

#### Scenario: Audiobookshelf book library is displayed at a wide width
- **WHEN** an Audiobookshelf book library meets the wide geometry conditions
- **THEN** it renders the hero-on-left arrangement matching grouped Music at the same dimensions

#### Scenario: Hero-bearing surface leaves wide geometry
- **WHEN** any hero-bearing browse surface no longer meets the shared wide geometry conditions
- **THEN** it renders its selected detail inline in a single-column browser
- **AND** no separate fallback is used

#### Scenario: Wide TV shows has an interactive left hero
- **WHEN** TV shows meets the wide geometry conditions
- **THEN** Series browsing remains on the right and the interactive episode workspace remains on the left

#### Scenario: Wide Movies has its selected-media hero
- **WHEN** Movies meets the wide geometry conditions
- **THEN** its selected-media hero renders on the left and its one-column browser on the right

#### Scenario: TV shows falls below the breakpoint
- **WHEN** TV shows does not meet the wide geometry conditions
- **THEN** selected Series detail replaces its ordinary row in its one-column browser

#### Scenario: Movies falls below the shared breakpoint
- **WHEN** Movies does not meet the wide geometry conditions
- **THEN** selected Movie detail replaces its ordinary row in its one-column browser

#### Scenario: Home videos is displayed at a wide width
- **WHEN** an Emby home-video library meets the wide geometry conditions
- **THEN** it renders hero-on-left with a one-column right-rail browser

#### Scenario: Audiobooks is displayed at a wide width
- **WHEN** an Audiobookshelf book library meets the wide geometry conditions
- **THEN** it renders hero-on-left matching grouped Music

## RENAMED Requirements

- FROM: `### Requirement: Hero area pinned above the list`
  TO: `### Requirement: Hero placement follows the responsive presentation`
- FROM: `### Requirement: Hero click focuses without activating`
  TO: `### Requirement: Hero pointer behavior follows placement`
- FROM: `### Requirement: Independence from top-hero design`
  TO: `### Requirement: Hero content is independent of placement`

## MODIFIED Requirements

### Requirement: Hero placement follows the responsive presentation

The selected item's hero or detail workspace SHALL be positioned by the shared right-panel presentation rather than a surface-specific renderer. When wide geometry is available, hero-on-left SHALL place selected detail beside a single-column browser. Otherwise the selected detail SHALL render inline immediately after the active media row in the single-column scrolling browser. No presentation SHALL pin the hero in a separate full-width area above the browser.

The inline hero SHALL remain part of list flow. Its rows SHALL move with the active row, account for variable detail height, and remain inert unless they contain an existing explicitly interactive subcomponent such as episode, track, chapter, or selector targets. The active media row remains the selection and activation target. If the minimum active row and minimum hero content cannot fit, the hero SHALL be suppressed and the browser SHALL retain the available area.

For wide Movies, the left hero SHALL continue using Home's selected-media card. For wide TV, the left workspace SHALL continue showing Series artwork, metadata, overview, season pills, and episodes. Other surfaces SHALL retain their declared content and interaction behavior while adopting the same placement rule.

#### Scenario: Wide hero-bearing browse surface
- **WHEN** a hero-bearing browse surface meets the shared wide geometry conditions and has a selected item
- **THEN** selected detail renders in the left pane
- **AND** the single-column browser renders in the right rail

#### Scenario: Narrow library renders an inline hero
- **WHEN** a hero-bearing browse surface does not meet the shared wide geometry conditions and has a selected item
- **THEN** the selected item's hero renders inline immediately after that item's active row
- **AND** the browser remains a single scrolling column
- **AND** the hero uses the content, artwork, metadata, loading behavior, and detail rows declared for that surface

#### Scenario: Narrow selection changes
- **WHEN** the cursor moves to another item in the inline presentation
- **THEN** the inline detail moves to the newly active row
- **AND** the previous row returns to its ordinary presentation

#### Scenario: Narrow list has insufficient space
- **WHEN** the inline presentation cannot fit the minimum active row and minimum selected detail
- **THEN** selected detail is suppressed
- **AND** the browser retains the available content area

#### Scenario: Narrow grouped Music
- **WHEN** grouped Music uses the inline presentation
- **THEN** selected album and track detail render at the active album row

#### Scenario: Narrow Audiobookshelf podcast
- **WHEN** an Audiobookshelf podcast library uses the inline presentation
- **THEN** selected-show metadata, filters, and downloaded episodes render at the active show row

#### Scenario: Narrow Audiobookshelf book
- **WHEN** an Audiobookshelf book library uses the inline presentation
- **THEN** selected-book metadata and chapter detail render at the active book row

#### Scenario: Narrow Feeds
- **WHEN** Feeds uses the inline presentation
- **THEN** selected-entry detail renders at the active entry row

#### Scenario: Narrow Home
- **WHEN** Home uses the inline presentation and its selected section has an item
- **THEN** selected-item detail renders in the selected section's list flow at the active row
- **AND** the section pills remain outside selected-item detail

#### Scenario: Wide TV pills sit in separate rails
- **WHEN** wide TV has both eligible library letter pills and season data for the selected Series
- **THEN** letter-range pills render at the top of the right-hand Series rail
- **AND** season pills render in the left Series workspace above its episode list

#### Scenario: Wide Movies pills sit in the right rail
- **WHEN** wide Movies is eligible for letter-range pills
- **THEN** the pill row renders at the top of the right-hand list rail
- **AND** the Movies list renders below it

#### Scenario: Inline selectors remain outside inert hero rows
- **WHEN** a surface has browser-level pills or search controls in the inline presentation
- **THEN** those controls retain their browser-level placement
- **AND** selected-item detail does not duplicate them

#### Scenario: Wide Movies renders the Home selected-media card
- **WHEN** a Movie is selected in wide Movies
- **THEN** the left pane renders the same selected-media card Home uses for that Movie

#### Scenario: Wide TV shows renders the selected Series workspace
- **WHEN** a Series is selected in wide TV
- **THEN** the left pane renders its artwork, metadata, season pills, and episodes
- **AND** the one-column Series browser remains in the right rail

#### Scenario: Wide TV season selection filters episodes
- **WHEN** the user selects another season in the wide TV workspace
- **THEN** only the left-pane episode list changes
- **AND** the right-hand Series browser remains unchanged

#### Scenario: Hero renders above the list
- **WHEN** a surface that formerly rendered selected detail above its list is displayed
- **THEN** it renders that detail on the left when wide or inline at the active row otherwise
- **AND** no separate top area is reserved

#### Scenario: Movies falls back below the breakpoint
- **WHEN** Movies does not meet the wide geometry conditions
- **THEN** selected Movie detail renders inline at the active row

#### Scenario: TV shows falls back below the breakpoint
- **WHEN** TV shows does not meet the wide geometry conditions
- **THEN** selected Series detail renders inline at the active row

#### Scenario: Narrow grouped Music uses the pinned hero
- **WHEN** grouped Music uses the narrow presentation
- **THEN** selected album detail renders inline rather than pinned

#### Scenario: Wide grouped Music uses its side hero
- **WHEN** grouped Music meets the wide geometry conditions
- **THEN** selected album and tracks render in the left pane beside the one-column album browser

#### Scenario: Hero suppressed when too little space remains
- **WHEN** the active presentation cannot fit minimum selected detail and a usable active row
- **THEN** selected detail collapses and the browser uses the available area

#### Scenario: Letter pills sit between hero and list
- **WHEN** a surface uses browser-level letter pills
- **THEN** hero-on-left places them in the right rail and inline presentation places them before browser flow
- **AND** they are never attached to a top-pinned hero

### Requirement: Hero tracks the current selection independent of scroll position

The hero SHALL always reflect the selected item. In hero-on-left, its screen position SHALL remain fixed while the browser cursor moves. In the inline presentation, its position SHALL follow the active row in scrolling list flow, and scrolling SHALL keep the active row and its selected detail addressable together. Wide read-only heroes SHALL derive selection solely from the right-hand browser. Interactive left workspaces SHALL continue deriving their parent item from the right-hand browser while their child cursor is active.

#### Scenario: Wide selection scrolls out of view
- **WHEN** the browser cursor moves to an item whose row is scrolled outside the visible right rail
- **THEN** the left hero or workspace updates to that item
- **AND** the left pane remains in the same position

#### Scenario: Child selection does not change the projected parent
- **WHEN** an episode, track, or chapter is selected in the left workspace
- **THEN** the left workspace continues showing the parent selected by the right-hand browser
- **AND** the right-hand browser cursor remains unchanged

#### Scenario: Inline selection is scrolled
- **WHEN** the active row crosses the visible inline browser area
- **THEN** scrolling keeps the active row and inline selected detail in navigable flow
- **AND** selected detail follows the active row rather than remaining pinned to a screen edge

#### Scenario: TV selection scrolled out of view
- **WHEN** the wide TV Series cursor moves outside visible right-rail rows
- **THEN** the left Series workspace updates and remains fixed in the left pane

#### Scenario: Selection scrolled out of view
- **WHEN** a wide read-only browser selection scrolls outside visible right-rail rows
- **THEN** the left hero continues projecting the selected item

#### Scenario: Episode selection does not change the projected Series
- **WHEN** an episode is selected in the left TV workspace
- **THEN** the workspace continues projecting the Series selected by the right-hand browser

#### Scenario: Narrow selection is scrolled
- **WHEN** the cursor crosses the visible inline browser area
- **THEN** scrolling keeps the active row and inline detail addressable together

### Requirement: Hero pointer behavior follows placement

A read-only hero-on-left preview SHALL not receive focus or activation from pointer gestures. Interactive child rows and selectors in a hero-on-left workspace SHALL expose their existing navigation targets, while artwork and blank hero space remain inert. In the inline presentation, hero-only rows SHALL be inert; activation remains on the active media row, while explicit episode, track, chapter, or selector targets retain their existing behavior.

#### Scenario: Wide read-only hero remains inert
- **WHEN** a user clicks artwork or blank space in a read-only left hero
- **THEN** no media item is activated
- **AND** activation remains available from the right-hand browser row

#### Scenario: Wide interactive child row
- **WHEN** a user clicks an episode, track, or chapter row in an interactive left workspace
- **THEN** that child becomes selected according to the surface's existing interaction behavior

#### Scenario: Inline hero-only row
- **WHEN** a user clicks inline hero space that is not an explicit child or selector target
- **THEN** no item is selected or activated

#### Scenario: Inline active media row
- **WHEN** a user activates the media row owning inline selected detail
- **THEN** the existing row activation behavior runs

#### Scenario: Wide Movies hero remains read-only
- **WHEN** the wide Movies hero is displayed
- **THEN** it has no keyboard focus or pointer activation action

#### Scenario: Wide TV episode row click
- **WHEN** a user clicks a visible episode row in the wide TV left workspace
- **THEN** that episode becomes selected without changing the Series browser cursor

#### Scenario: Wide TV season pill click
- **WHEN** a user clicks a season pill in the wide TV left workspace
- **THEN** the season changes without playing an episode

#### Scenario: Wide TV artwork click
- **WHEN** a user clicks Series artwork or blank space in the wide TV left workspace
- **THEN** no episode is selected or played

#### Scenario: Single click on the hero
- **WHEN** a user single-clicks non-interactive hero framing or metadata
- **THEN** no item is activated

#### Scenario: Double click on the hero
- **WHEN** a user double-clicks non-interactive hero framing or metadata
- **THEN** no item is activated and activation remains owned by media or child rows

#### Scenario: Hero-on-top activation remains unchanged
- **WHEN** a formerly top-pinned surface adopts inline or hero-on-left placement
- **THEN** media-row and explicit child-target activation remain unchanged
- **AND** the removed top hero is not retained as a duplicate target

### Requirement: Hero content is independent of placement

Hero content SHALL be independent of responsive placement. The same surface declaration SHALL supply content to hero-on-left and inline presentations, with only arrangement-specific composition changing. Wide Movies SHALL continue reusing Home's selected-media card rather than maintaining a second Movies-specific left card. No hero content implementation SHALL depend on a hero-on-top fallback.

#### Scenario: Placement changes
- **WHEN** terminal geometry switches between hero-on-left and inline presentation
- **THEN** selected detail preserves the content declared for that surface
- **AND** only placement and arrangement-specific composition change

#### Scenario: Home and wide Movies use one selected-media card
- **WHEN** the same Movie is selected in Home and in wide Movies
- **THEN** the hero card uses the same image selection, metadata, watch-state, overview treatment, and cache behavior

#### Scenario: Shared card changes centrally
- **WHEN** the shared Home selected-media card changes
- **THEN** wide Movies renders that change without a second Movies-card edit

#### Scenario: Hero content remains consistent
- **WHEN** selected detail switches between hero-on-left and inline presentation
- **THEN** its declared image, metadata, overview, loading state, and child detail remain consistent

#### Scenario: Wide Movies card changes centrally
- **WHEN** the shared Home selected-media card presentation changes
- **THEN** wide Movies renders the change without a Movies-specific card edit

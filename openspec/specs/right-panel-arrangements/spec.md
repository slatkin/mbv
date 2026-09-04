# right-panel-arrangements Specification

## Purpose

Defines the right panel's two responsive hero presentations, which screens use each, and where the
responsive decision is made, so that the wide and narrow presentations can be changed independently
of one another and a new screen inherits a known arrangement without being individually designed.

## Requirements

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

### Requirement: Screens do not determine their own arrangement

The responsive breakpoint SHALL be evaluated in one place, and its value SHALL be defined in one
place. An individual screen SHALL NOT test the available width to select an arrangement, a column
count, or any presentation that differs between arrangements. The arrangement SHALL own pane
placement, breakpoints, and rectangle splitting; components SHALL own painting; and screens SHALL
provide content and interaction state. Code outside a screen SHALL NOT paint part of that screen's
arrangement.

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

### Requirement: The right panel has exactly two hero presentations

The right panel SHALL provide exactly two responsive hero presentations for every hero-bearing browse surface. At or above the shared breakpoint, when the existing minimum-height guard is satisfied, the surface SHALL use hero-on-left: the selected hero or detail workspace occupies the left pane and a single-column browser occupies the right rail. Otherwise the surface SHALL use selected-row replacement: the selected item's ordinary row is replaced by its variable-height detail block in the single-column scrolling browser.

A separate detail block SHALL NOT be an arrangement or fallback. A surface SHALL NOT reserve a hero in a separate full-width area above its browser. Non-hero screens retain their existing presentation.

The inline hero SHALL render one content shape across all surfaces: title, optional metadata line, optional overview text, and an optional image. The image model SHALL be selected by image aspect ratio — right-aligned wrap-around (Model A) for tall images such as posters and book covers, right-half meta-column (Model B) for wide 16:9 thumbnails, and Model A's degenerate no-image form for surfaces without artwork. No surface SHALL render structured lists (seasons, episodes, tracks, chapters) inside the inline hero; those SHALL be accessed via the inline-hero selection modal.

#### Scenario: A browse surface enters the narrow presentation

- **WHEN** a hero-bearing browse surface's available width falls below the shared breakpoint
- **THEN** it renders one browser column
- **AND** the selected item's ordinary row is replaced by inline detail at the same flow position
- **AND** the inline hero shows title, metadata, overview, and image using the model selected by the image's aspect ratio
- **AND** no separate hero area is reserved above the browser
- **AND** no structured lists render inside the inline hero

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
- **AND** the inline hero shows one content shape (title, metadata, overview, image) with no structured lists

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

### Requirement: Feeds Wide arrangement is canonical
The Feeds Service/tab Wide panel SHALL use the canonical one-column `WideMediaList` and preserve the accepted `restore-feeds-service-wide-list` (umbrella task 1.3a) rail framing, surface treatment, and selected-row alignment.

#### Scenario: Wide and Narrow use approved variants
- **WHEN** the panel crosses the Wide breakpoint
- **THEN** only the named Wide variant changes placement; Narrow uses `InlineMediaBrowser` as applicable, without changing FeedEntry identity or watched/group state.

### Requirement: Shared hero-on-left arrangement owns the status-row reserve
The shared hero-on-left arrangement primitive SHALL reserve the one status-bar row when it computes the hero and list panes, so every hero-on-left destination inherits the reserve from one place. Screens and components SHALL NOT re-derive the reserve (no per-tab `saturating_sub(1)`, `bottom_pad`, or equivalent) on top of the panes the shared primitive returns.

#### Scenario: Panels leave one blank row above the status bar
- **WHEN** any hero-on-left destination (Home, Feeds, and the non-migrated media tabs that share the primitive) renders in the Wide layout
- **THEN** exactly one blank row separates the bottom of the content panels from the status bar, and that reserve is applied by the shared arrangement primitive rather than the screen.

### Requirement: Other two-column policy is unchanged
This slice SHALL NOT alter non-hero two-column arrangements outside Home and Feeds.

#### Scenario: Unrelated library layout remains stable
- **WHEN** a non-hero library is rendered outside the migrated Home or Feeds destinations
- **THEN** its existing two-column policy and geometry remain unchanged.

### Requirement: Music and Audiobookshelf adopt the TV and Movies Wide precedent

Grouped Music and Audiobookshelf Podcast and Book destinations SHALL use the same Wide right-panel contract established by TV and Movies. When the shared width and minimum-height predicate is satisfied, the provider-owned detail/workspace SHALL occupy the left pane, while parent-owned browser-level pills followed by ordinary one-column rows SHALL occupy the right rail. The arrangement SHALL reuse the shared predicate, pane framing, content spacing, and short-height fallback. No Wide presentation SHALL use an Inline hero or selected-row replacement in the right rail.

#### Scenario: Wide provider workspace and ordinary right rail
- **WHEN** grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide geometry conditions
- **THEN** its provider-owned detail/workspace renders in the left pane
- **AND** its browser-level pills and ordinary one-column rows render in the right rail
- **AND** the right rail contains no Wide Inline hero or selected-row replacement.

#### Scenario: Shared geometry fallback is retained
- **WHEN** the destination crosses the shared width or minimum-height guard
- **THEN** it uses the same shared predicate, pane framing, content spacing, and short-height fallback as TV and Movies
- **AND** it uses the shared Inline fallback, or suppresses detail when the shared minimum cannot fit
- **AND** it does not define a destination-specific breakpoint or arrangement.

### Requirement: Audiobookshelf Podcast and Book Wide surfaces route through the shared right pane

The Audiobookshelf Podcast Wide surface SHALL render through the shared hero-on-left right-pane arrangement rather than a bespoke painter. The Audiobookshelf Book Wide right rail already routes through the shared right pane; its defect is that the `render_book_browser` call reused there carries the inline selected-row replacement path, and that replacement path SHALL NOT be used in the Wide right rail. These are provider-arrangement repairs this slice owns, distinct from the canonical list control itself. The Podcast Wide right rail SHALL present the same pill row it presents at Narrow width. The Book Wide left pane SHALL use the shared provider-detail-workspace framing and content spacing used by grouped Music, and its right rail SHALL show ordinary fixed-height one-column rows with no selected-row replacement and no Inline hero. Neither surface SHALL define a destination-specific breakpoint, column-sizing rule, or fallback.

#### Scenario: Podcast Wide has pill-row parity with Narrow
- **WHEN** an Audiobookshelf Podcast library meets the shared Wide geometry conditions
- **THEN** its right rail renders the shared pill row over the one-column show browser
- **AND** it routes through the shared hero-on-left right pane, not a surface-specific painter.

#### Scenario: Book Wide uses shared workspace framing
- **WHEN** an Audiobookshelf Book library meets the shared Wide geometry conditions
- **THEN** the selected book's provider detail workspace renders in the left pane with the shared framing and spacing used by grouped Music
- **AND** the right rail renders ordinary fixed-height one-column rows with no selected-row replacement or Inline hero.

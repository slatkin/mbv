## MODIFIED Requirements

### Requirement: Hero placement follows the responsive presentation

The selected item's hero or detail workspace SHALL be positioned by the shared right-panel presentation rather than a surface-specific renderer. The arrangement SHALL own pane placement, breakpoints, and rectangle splitting; the component SHALL own painting; and the screen SHALL provide semantic content and interaction state. When wide geometry is available, hero-on-left SHALL place selected detail beside a single-column browser. Otherwise the selected ordinary browser row SHALL be replaced by the variable-height inline detail block in the single-column scrolling browser. No presentation SHALL reserve a separate full-width area above the browser.

The inline hero SHALL remain part of list flow as the selected row's replacement. Its variable height SHALL be budgeted once, its block SHALL own the selected item's geometry and parent activation target, and single click SHALL focus while double click performs normal item activation. If the replacement cannot fit, the ordinary selected row SHALL be restored with its normal selected appearance and interaction.

The inline hero SHALL render the same content shape on every surface: title, optional metadata line, optional overview text, and an optional image. The image model SHALL be selected by image aspect ratio — Model A (right-aligned, wrap-around) for tall images such as posters and book covers, Model B (right-half, meta-column) for wide 16:9 thumbnails. No surface SHALL render structured lists (seasons, episodes, tracks, chapters) inside the inline hero. Structured lists SHALL be accessed via the inline-hero selection modal (see `inline-hero-selection-modal`).

For wide Movies, the left hero SHALL continue using Home's selected-media card. For wide TV, the left workspace SHALL continue showing Series artwork, metadata, overview, season pills, and episodes. Other surfaces SHALL retain their declared content and interaction behavior while adopting the same placement rule. Wide-mode track and episode listings are outside this requirement; they are governed by the hero-on-left presentation.

#### Scenario: Wide hero-bearing browse surface

- **WHEN** a hero-bearing browse surface meets the shared wide geometry conditions and has a selected item
- **THEN** selected detail renders in the left pane
- **AND** the single-column browser renders in the right rail

#### Scenario: Narrow library renders an inline hero

- **WHEN** a hero-bearing browse surface does not meet the shared wide geometry conditions and has a selected item
- **THEN** the selected item's ordinary row is replaced by its inline hero block at the same flow position
- **AND** the browser remains a single scrolling column
- **AND** the hero shows title, metadata, overview, and image using the model selected by the image's aspect ratio

#### Scenario: Narrow selection changes

- **WHEN** the cursor moves to another item in the inline presentation
- **THEN** the previous row returns to its ordinary presentation and the new selected row is replaced by inline detail
- **AND** the previous row returns to its ordinary presentation

#### Scenario: Selected inline hero reaches the viewport bottom

- **WHEN** the selected row's full inline hero would extend below the visible browser
- **THEN** the browser scrolls upward until the complete inline hero is visible
- **AND** every surface uses the shared inline-detail flow rather than surface-specific scrolling

#### Scenario: Narrow list has insufficient space

- **WHEN** the inline presentation cannot fit the minimum active row and minimum selected detail
- **THEN** the ordinary selected row is restored
- **AND** its normal selected appearance and interaction are retained

#### Scenario: Narrow TV shows uses standard hero with selection modal

- **WHEN** a TV Series is selected in the inline presentation
- **THEN** the inline hero shows the Series title, metadata, overview, and poster image only
- **AND** season pills and episode rows do NOT render inside the inline hero
- **AND** pressing Enter opens the constituent-list modal for season and episode selection

#### Scenario: Narrow grouped Music

- **WHEN** grouped Music uses the inline presentation
- **THEN** selected album hero content (title, metadata, album art) replaces the active album row
- **AND** the track list does NOT render inline; Enter opens the selection modal

#### Scenario: Narrow Audiobookshelf podcast

- **WHEN** an Audiobookshelf podcast library uses the inline presentation
- **THEN** selected-show hero content (title, author, description, cover) replaces the active show row
- **AND** filters and downloaded episodes do NOT render inside the inline hero
- **AND** alphabetical pills render in the panel area like every other library tab
- **AND** pressing Enter opens the constituent-list modal for episode selection

#### Scenario: Narrow Audiobookshelf book

- **WHEN** an Audiobookshelf book library uses the inline presentation
- **THEN** selected-book hero content (title, author, metadata, overview, cover) replaces the active book row
- **AND** chapter detail does NOT render inside the inline hero
- **AND** the cover image uses Model A (right-aligned, wrap-around), not Model B
- **AND** exactly one author-bucket pill row renders above the browser with a parent-background spacer
- **AND** no chapter child target or chapter focus exists in the narrow presentation
- **AND** Enter or parent double-click opens the chapter selection modal

#### Scenario: Narrow Feeds

- **WHEN** Feeds uses the inline presentation
- **THEN** selected-entry detail replaces the active entry row
- **AND** the hero shows title and metadata with no image (Model A degenerate)

#### Scenario: Narrow Home

- **WHEN** Home uses the inline presentation and its selected section has an item
- **THEN** selected-item detail replaces the active row in the selected section's list flow
- **AND** the section pills remain outside selected-item detail
- **AND** Home items with wide 16:9 artwork (Emby Keep Watching, Audiobookshelf episodes) use Model B (beside-image)
- **AND** Home Feed items use Model A no-image (text-only), matching the dedicated Feeds tab

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
- **THEN** selected Series detail renders inline at the active row with title, metadata, overview, and poster only

#### Scenario: Narrow grouped Music uses selected-row replacement

- **WHEN** grouped Music uses the narrow presentation
- **THEN** selected album detail replaces the active album row with title, metadata, and album art only

#### Scenario: Wide grouped Music uses its side hero

- **WHEN** grouped Music meets the wide geometry conditions
- **THEN** selected album and tracks render in the left pane beside the one-column album browser

#### Scenario: Hero suppressed when too little space remains

- **WHEN** the active presentation cannot fit minimum selected detail and a usable active row
- **THEN** the ordinary selected row is restored and the browser uses the available area

#### Scenario: Letter pills sit between hero and list

- **WHEN** a surface uses browser-level letter pills
- **THEN** hero-on-left places them in the right rail and inline presentation places them before browser flow
- **AND** they are never attached to a separate detail block

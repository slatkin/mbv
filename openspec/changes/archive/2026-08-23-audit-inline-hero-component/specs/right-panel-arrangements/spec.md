## MODIFIED Requirements

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

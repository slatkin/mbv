## MODIFIED Requirements

### Requirement: Hero overview and media-list boxes have distinct ownership

When the shown item has overview text, every Wide hero destination SHALL render it in one recessed
overview main-content box below the Hero header, through the shared Library panel, at one
panel-owned internal padding value. When the item has no overview text, no overview box SHALL render.
The overview box carries only the Hero text description.

A destination with structured episode, track, or chapter content SHALL additionally render one
separate recessed media-list box (the Workspace box). The Library panel owns both box and viewport
rects and the Workspace box's surface, which is accent-soft while its list holds focus and the
backdrop surface otherwise; the destination component owns its embedded `WideMediaList<Target>`,
including rows, target identity, cursor, scroll, selection, intent translation, and hit geometry. A
Hero SHALL NOT carry a structured listing or mutable list state. The destination SHALL NOT define its
own box geometry or surface.

#### Scenario: TV presents overview before episodes

- **WHEN** a selected Series with overview text renders at Wide geometry
- **THEN** title and ordered metadata render first
- **AND** one blank row separates the metadata from the overview main-content box
- **AND** a separate media-list box follows the overview box
- **AND** season pills are parent chrome above the episode `WideMediaList` viewport

#### Scenario: A structured workspace renders its media list

- **WHEN** TV, Music, or Audiobookshelf renders selected structured content
- **THEN** its parent-owned `WideMediaList` renders inside the separate media-list box
- **AND** canonical row, scroll, selection, and hit geometry are preserved

#### Scenario: The overview is empty

- **WHEN** a selected item supplies no description text
- **THEN** no overview main-content box is painted
- **AND** the media-list box, if any, follows the Hero header directly

#### Scenario: Two overview payloads are compared

- **WHEN** two description payloads render in the overview main-content box at the same pane width
- **THEN** both begin at the same offset from the box's edges

#### Scenario: The media list takes focus

- **WHEN** a Workspace list holds focus on any destination
- **THEN** its media-list box renders the accent-soft surface

### Requirement: The right panel has exactly two hero presentations

The right panel SHALL provide exactly two responsive hero presentations for every hero-bearing browse surface. At or above the shared breakpoint, when the existing minimum-height guard is satisfied, the surface SHALL use Wide hero: the selected hero or detail workspace occupies the right pane and a single-column browser occupies the left rail. Otherwise the surface SHALL use selected-row replacement: the selected item's ordinary row is replaced by its variable-height detail block in the single-column scrolling browser.

A separate detail block SHALL NOT be an arrangement or fallback. A surface SHALL NOT reserve a hero in a separate full-width area above its browser. Every library is hero-bearing.

The inline hero SHALL render one content shape across all surfaces: title, optional metadata line, optional overview text, and an optional image. The image chosen by the shared artwork policy (`library-panel`) SHALL render right-aligned, sized from its aspect, with the text wrapping around it and continuing at full width below it; a surface without an image renders the same form without the image. No surface SHALL render structured lists (seasons, episodes, tracks, chapters), selector pills, or list controls inside the inline hero; structured lists SHALL be accessed via the inline-hero selection modal.

#### Scenario: A browse surface enters the narrow presentation

- **WHEN** a hero-bearing browse surface's available width falls below the shared breakpoint
- **THEN** it renders one browser column
- **AND** the selected item's ordinary row is replaced by inline detail at the same flow position
- **AND** the inline hero shows title, metadata, overview, and a right-aligned image with wrap-around text
- **AND** no separate hero area is reserved above the browser
- **AND** no structured lists render inside the inline hero

#### Scenario: Wide geometry has insufficient height

- **WHEN** a hero-bearing browse surface meets the shared width breakpoint but fails the existing minimum-height guard
- **THEN** it uses selected-row replacement
- **AND** it restores the ordinary selected row if detail cannot fit

#### Scenario: A browse surface enters the wide presentation

- **WHEN** a hero-bearing browse surface meets the shared width and minimum-height conditions
- **THEN** it renders Wide hero
- **AND** its browser is a single-column left rail

#### Scenario: Panel mode changes

- **WHEN** the user cycles Panel mode
- **THEN** the presentation is recomputed from the width and height available to the right panel
- **AND** the same shared breakpoint and minimum-height guard apply

#### Scenario: A library enters the narrow presentation

- **WHEN** a library browse surface does not meet the shared wide geometry conditions
- **THEN** it renders one list column with selected detail inline at the active row
- **AND** the inline hero shows one content shape (title, metadata, overview, image) with no structured lists

#### Scenario: A 16:9 image in the narrow presentation

- **WHEN** the selected item's image is a 16:9 thumbnail in the narrow presentation
- **THEN** it renders right-aligned with wrap-around text, the same form as a poster or cover

#### Scenario: A formerly separate-detail surface crosses the breakpoint

- **WHEN** a formerly separate-detail surface crosses below the shared breakpoint
- **THEN** it uses selected-row replacement and retains no separate detail assignment

#### Scenario: A formerly separate-detail surface crosses the breakpoint

- **WHEN** a formerly separate-detail surface crosses the shared breakpoint in either direction
- **THEN** it switches only between Wide hero and selected-row replacement

#### Scenario: A wide hero screen falls below the breakpoint

- **WHEN** a Wide hero surface crosses below the shared breakpoint
- **THEN** it renders selected-row replacement with one browser column

#### Scenario: A Wide hero screen falls below the breakpoint

- **WHEN** a Wide hero surface no longer meets either wide geometry condition
- **THEN** it renders selected-row replacement

## REMOVED Requirements

### Requirement: Per-screen presentation differences are declared in one place

**Reason**: Per-screen declarations of image shape, colour variant and element presence are the
mechanism that let destinations diverge. Differences now come only from the typed content a
destination supplies to the Library panel's slots; the Wide Hero header type follows the item's kind.

**Migration**: `library-panel` — "Every library destination renders through one Library panel
skeleton" and "Wide Hero header has three types chosen by content kind".

### Requirement: Feeds Wide arrangement is canonical

**Reason**: It preserved the Feeds-only `restore-feeds-service-wide-list` presentation (a second pill
bar and bespoke list chrome), which was never an accepted design. Feeds conforms to the Emby screens.

**Migration**: `library-panel` — "Emby screens are the reference presentation" and "The Browser pane
has one Selector row and one List controls row".

### Requirement: Other two-column policy is unchanged

**Reason**: No non-hero library exists; the two-column catalog it protected is removed.

**Migration**: `canonical-media-lists` — "Named destinations compose without changing provider
authority".

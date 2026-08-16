## MODIFIED Requirements

### Requirement: Independence from top-hero design

Hero placement is a property of the right panel's arrangement, not of the list renderer. The
hero's content — image, metadata, and overview — SHALL be identical regardless of where the
arrangement places it. Only the position changes. The two placements SHALL NOT be maintained as
parallel branches within the list renderer for comparison; the arrangement selects one.

The positional variant MUST preserve the same hero content as the top-hero design.

#### Scenario: Hero content remains consistent

- **WHEN** an item is shown in the inline hero
- **THEN** its image, metadata, and overview SHALL match the top-hero content

#### Scenario: Placement changes

- **WHEN** the arrangement places the hero differently
- **THEN** the hero's content is unchanged and only its position differs

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

## REMOVED Requirements

### Requirement: Hero position
**Reason**: The inline hero placed directly below the selected row, with the list wrapping around it, is superseded by arrangement-owned hero placement.
**Migration**: The hero renders per the screen's assigned arrangement (hero-on-top or hero-on-left); no list wraps around a hero.

### Requirement: Hero follows the cursor
**Reason**: Cursor-following hero position is superseded by arrangement-owned hero placement.
**Migration**: The hero stays in its arrangement position and only its content updates as the cursor moves.

### Requirement: Row map reflects the hero
**Reason**: The row-map `None` entries for inline-hero rows are superseded by arrangement-produced hit targets (`right-panel-arrangements`).
**Migration**: Hit-testing uses the common hit-target set; there are no inline-hero rows to exclude.

### Requirement: Auto-scroll
**Reason**: The inline hero's keep-hero-and-cursor-visible scroll behaviour is superseded by the pinned arrangement hero.
**Migration**: Auto-scroll keeps the cursor visible as it does for other list screens.

### Requirement: Hero interaction
**Reason**: Duplicate single/double-click contract superseded by "Hero click focuses without activating", which stays.
**Migration**: Hero click behaviour is unchanged and covered by the retained requirement.

### Requirement: Invariant preserved
**Reason**: The top-section/bottom-section packing invariant is superseded by arrangement-owned list rendering.
**Migration**: The column-count invariant is covered by the retained "Column-count invariant preserved" requirement.

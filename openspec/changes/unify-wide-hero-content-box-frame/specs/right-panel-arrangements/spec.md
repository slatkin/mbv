## MODIFIED Requirements

### Requirement: Hero overview and media-list boxes have distinct ownership

Every Wide hero destination SHALL paint a recessed overview main-content box through the
shared primitive, even when its description is empty. The overview box carries only the Hero
text description and has one primitive-owned internal padding value.

The shared arrangement SHALL own the box's horizontal frame. Every destination's main-content box
SHALL begin at the same offset from its Hero pane's edges — the pane's own content edge — and that
frame SHALL be derived by the shared arrangement rather than supplied by the destination. A
destination SHALL NOT be able to paint a box in a horizontal frame of its own choosing, and SHALL
NOT compensate for a frame it was given.

A destination with structured episode, track, or chapter content SHALL additionally paint a
separate recessed media-list box. The shared arrangement owns both box and viewport rects; the
destination component owns its embedded `WideMediaList<Target>`, including rows, target identity,
cursor, scroll, selection, intent translation, and hit geometry. A Hero SHALL NOT carry a
structured listing or mutable list state. The destination SHALL NOT define its own box geometry
or surface.

#### Scenario: TV presents overview before episodes

- **WHEN** a selected Series renders at Wide geometry
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
- **THEN** the overview main-content box is still painted
- **AND** its absence is never used to signal an empty payload

#### Scenario: Two overview payloads are compared

- **WHEN** two description payloads render in the overview main-content box at the same pane width
- **THEN** both begin at the same offset from the box's edges

#### Scenario: Every destination paints its box in the same frame

- **WHEN** any two Wide hero destinations render at the same Wide geometry
- **THEN** each destination's main-content box begins at the same offset from its own Hero pane's
  edges
- **AND** that offset is the pane's shared content inset, so the box spans the column the items
  above it occupy

#### Scenario: A destination paints content beside its main-content box

- **WHEN** a destination paints hero text or artwork above its main-content box
- **THEN** that content and the box share one horizontal frame
- **AND** no destination-supplied inset shifts the box relative to the content above it

#### Scenario: A destination attempts to frame its own box

- **WHEN** a destination is written to paint a main-content box
- **THEN** it cannot supply the box's horizontal frame, and no compensation for a received frame
  is required of it

## ADDED Requirements

### Requirement: Hero-on-left is one layout with a type-varying inset box

The hero-on-left arrangement SHALL be a single layout for every surface that uses
it: a static hero in the left pane, and in the right rail a pill bar above an
inset box. Only the inset box's contents SHALL vary by content type — overview
for movies, seasons with a pill selector for TV, and a track/episode list for
albums, podcasts, and audiobooks. A surface SHALL NOT introduce its own pane
geometry, its own right-rail structure, or a bespoke arrangement for its type.

#### Scenario: A surface renders hero-on-left

- **WHEN** any surface is displayed in the hero-on-left arrangement
- **THEN** it presents the static left hero and the right rail's pill-bar-over-
  inset-box structure from the shared arrangement
- **AND** the only per-type difference is what fills the inset box

#### Scenario: A new content type adopts hero-on-left

- **WHEN** a content type is added to the hero-on-left arrangement
- **THEN** it supplies inset-box contents only
- **AND** it does not define pane widths, the pill row, or the panel chrome

### Requirement: Every hero-on-left surface reserves the pill row uniformly

Every surface in the hero-on-left arrangement SHALL obtain its right-rail pill row
from the shared right-pane split, so the pill row is present and placed
identically across surfaces. A surface SHALL NOT paint its list directly onto the
right pane in a way that omits or relocates the pill row.

#### Scenario: A hero-on-left surface shows its right rail

- **WHEN** a hero-on-left surface renders its right rail at wide geometry
- **THEN** the pill row occupies the top of the rail via the shared split
- **AND** the list panel begins below it, the same as every other hero-on-left
  surface

### Requirement: Emby podcast libraries use the generic library arrangement

An Emby library whose collection type is podcasts SHALL be arranged exactly like
any other Emby library of its shape. mbv SHALL NOT route Emby podcast libraries
through a podcast-specific arrangement, hero, or detail path; the podcast
collection type SHALL NOT be a factor in choosing the arrangement.

#### Scenario: An Emby podcast library is displayed

- **WHEN** an Emby library identified as a podcast collection is displayed
- **THEN** it uses the same arrangement rules a generic Emby library would use for
  its geometry
- **AND** no podcast-specific branch alters its placement, hero, or detail

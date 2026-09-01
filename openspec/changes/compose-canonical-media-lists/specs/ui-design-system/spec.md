## ADDED Requirements

### Requirement: Named primary media browsers reuse the canonical list controls

Home, the hero-bearing generic Emby library catalog browser, Movies, TV Series browsing, grouped Music album browsing, the Emby homevideos feed view, the Emby podcast channel list, Audiobookshelf Podcast show browsing, Audiobookshelf Book browsing, Feeds, and Queue's fixed-row list SHALL compose the applicable canonical control for shared cursor, scroll, viewport, movement, fixed-row painting, selection, truncation, scrollbar, and hit-geometry behavior.

A destination SHALL NOT copy those mechanics into its own painter merely because its content comes from another provider or has different metadata. Queue composes fixed-row behavior only; non-hero two-column browsers remain governed by their existing column policy.

#### Scenario: A new provider hero browser is added

- **WHEN** a provider destination displays selectable hero-bearing media rows
- **THEN** it maps its content into the canonical row vocabulary
- **AND** it composes the canonical control appropriate to its responsive presentation
- **AND** provider identity alone is not accepted as a reason for bespoke list rendering

#### Scenario: A shared list behavior changes

- **WHEN** canonical row placement, truncation, selection, scrolling, or hit geometry changes
- **THEN** every composing destination receives the change through the shared control
- **AND** no destination-local copy requires the same edit

#### Scenario: A destination has distinct content

- **WHEN** Queue needs bounded active progress, Music needs artist headings, Feeds needs date headings, or Home needs section identity
- **THEN** it expresses the difference through prepared semantic item state, heading/spacer rows, or opaque targets
- **AND** it does not replace the canonical list mechanics

### Requirement: Canonical-list exceptions are explicit

A named primary media browser that cannot use the canonical controls SHALL be registered as a named bespoke surface. Its record SHALL identify the structural requirement that the closed row and presentation vocabulary cannot express, the canonical behavior it still reuses, and focused verification for the exception. Temporary migration state and implementation convenience SHALL NOT qualify as structural reasons.

#### Scenario: A bespoke list is proposed

- **WHEN** a destination claims the canonical row vocabulary cannot represent its presentation
- **THEN** review compares the requirement with item, heading, spacer, bounded semantic state, and opaque targets first
- **AND** the bespoke surface is accepted only when that vocabulary cannot express the required behavior

#### Scenario: A bespoke surface duplicates canonical mechanics

- **WHEN** an exception independently implements cursor visibility, fixed-row placement, truncation, selection, scrollbar, or hit geometry that the canonical control already provides
- **THEN** the exception is non-conforming
- **AND** those mechanics are moved back to the canonical control or reused from it

### Requirement: Each implementation slice proves composition before deleting loops

Every implementation slice SHALL identify the exact destinations it migrates, preserve or improve relevant existing characterization, add focused structural checks for realistic uncovered drift, and record manual end-to-end evidence for that slice's destinations before deleting their old loops. Existing characterization alone SHALL NOT be treated as sufficient when its fixture omits the metadata, grouping, active state, breakpoint transition, or image behavior being migrated.

#### Scenario: A destination slice migrates

- **WHEN** a slice replaces a destination's bespoke fixed-row loop
- **THEN** focused automated evidence confirms the destination composes the correct canonical control and preserves its structural behavior
- **AND** manual evidence covers the destination's affected Wide and Narrow presentations, focus, movement, and prepared image behavior
- **AND** the old loop is deleted only after both forms of evidence exist

#### Scenario: An existing baseline is vacuous

- **WHEN** an existing fixture lacks the metadata or interaction state needed to exercise the path being migrated
- **THEN** that fixture is improved or supplemented with the smallest representative case before deletion
- **AND** a passing metadata-free or state-free buffer is not cited as evidence for the missing behavior

#### Scenario: Known Wide drift is corrected

- **WHEN** the Home/Feeds and Music/Audiobookshelf slices migrate Feeds and Audiobookshelf Books
- **THEN** focused checks protect Feeds' single-column Wide rail and Books' absence of Wide selected-row replacement
- **AND** unrelated cell-by-cell visual details are not duplicated in new tests when stronger existing characterization already covers them

### Requirement: Slice boundaries remain review and rollback boundaries

Each destination-family slice SHALL be delivered as its own PR against the migration feature branch. A squash MAY combine commits within one slice but SHALL NOT combine multiple slices. File splits required by the 800-line cap SHALL land before or with the slice wiring that requires them.

#### Scenario: A slice is reviewed

- **WHEN** a destination-family implementation is ready
- **THEN** its PR contains only that slice and its directly required shared changes
- **AND** another family can be reverted or delayed without reverting the completed slice

#### Scenario: A near-limit component receives new wiring

- **WHEN** a slice would push a source file over 800 lines
- **THEN** that slice includes the ownership-preserving split before or with the new wiring
- **AND** final campaign verification is not the first point where the over-limit file is detected

### Requirement: Canonical controls are the sole list painter

Each migrated primary media-list surface SHALL have exactly one canonical list painter for its body at each layout breakpoint and no destination-specific duplicate row geometry or second hit-coordinate path. A slice SHALL NOT treat a surface as migrated while a legacy list painter still runs for that surface body in the same frame.

#### Scenario: Painter ownership is reviewable

- **WHEN** a reviewer traces a migrated surface's render path at a given breakpoint
- **THEN** the path reaches the embedded canonical control exactly once
- **AND** no legacy list painter and no second hit-coordinate calculation runs for that surface body

### Requirement: Visual verification precedes UI tests

For every slice that changes a rendered media-list surface, the controlling order SHALL be: characterize current behavior by source reading and manual observation only; perform the production visual correction at the affected Wide and Narrow widths; obtain explicit user live visual approval of the running result; and only then add or modify any UI fixture, characterization buffer, or rendered/geometry test for that surface. Characterization performed before that approval SHALL be read-only — source trace, unchanged existing evidence, and manual observation — and SHALL NOT add or edit a test or fixture or drive appearance from a test. Non-visual tests such as delivery, arbitration, selectable-index, and gesture recognition in isolation MAY precede approval.

#### Scenario: Tests follow approval

- **WHEN** a slice has corrected a media-list surface's visuals and the user has explicitly approved the live result
- **THEN** focused rendered and geometry tests may be added or updated using metadata-, group-, state-, image-, and breakpoint-bearing fixtures

#### Scenario: Characterization stays read-only before approval

- **WHEN** a slice characterizes a surface's current behavior before visual approval
- **THEN** it uses only source reading, unchanged existing evidence, and manual observation
- **AND** it does not add or modify a UI test or fixture and does not drive appearance from a test

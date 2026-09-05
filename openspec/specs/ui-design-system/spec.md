# ui-design-system Specification

## Purpose
The UI design-system capability keeps mbv's terminal surfaces visually consistent
while allowing screens to provide their own content and explicitly approved
semantic variations.

These requirements bind all new UI code and any surface a change touches, from the
PR that lands the module split forward. Surfaces not yet migrated are listed in the
change's `ledger.md`; that list may only shrink.

## Requirements

### Requirement: Screens use canonical UI ownership boundaries

The UI SHALL separate screen content, arrangement geometry, and component painting.
Screens SHALL provide semantic content and approved variants; arrangements SHALL own
shared geometry; components SHALL own their painting and styling. Screen modules
SHALL NOT call Ratatui or construct layout rectangles.

For every mouse path, hit-target ownership belongs to the interactive component
that paints the region, as defined by the `interactive-component-framework` and
`mouse-input` capabilities. Screens SHALL NOT compute or own hit geometry, and no
screen-local or arrangement-local hit map is introduced. The former global
completed-frame mouse hit map and router are removed by those capabilities;
render-only layout state MAY remain, but this capability no longer treats it as hit
resolution authority.

Classification is by signature: a function taking app state and returning a typed
content model is screen code; a function taking a typed content model, a `Rect`, and
a buffer is a component; a function placing components within a `Rect` and owning
breakpoints is an arrangement.

#### Scenario: A screen adds content without duplicating a painter
- **WHEN** a screen needs different titles, metadata, rows, or images
- **THEN** it supplies a screen model to an existing arrangement or component
- **AND** it does not copy the arrangement's geometry or painter

#### Scenario: Existing hit-target resolution is preserved
- **WHEN** a mouse event is resolved for any surface
- **THEN** the click resolves to a target computed by the interactive component
  that painted the region, from the same geometry it painted with
- **AND** it is not resolved by a global completed-frame hit map or by any
  screen-local or arrangement-local hit map

#### Scenario: Deferred mouse support is restored later
- **WHEN** mouse interaction is restored for a surface that had it deferred
- **THEN** its interactive component computes targets from the geometry it painted
- **AND** the implementation does not restore a global mouse router, global hit map,
  or duplicated coordinate path

#### Scenario: A surface is migrated
- **WHEN** a surface listed in the change ledger is brought inside the boundary
- **THEN** a characterization buffer test capturing its current default, focused,
  narrow-width, and selected output lands first, in its own commit
- **AND** the migration commit leaves that test unchanged and passing
- **AND** the ledger row is ticked in the same PR

#### Scenario: A hero uses an approved additional-content style
- **WHEN** a hero supplies the Movie overview/detail block, the TV season/pill and
  episode workspace, the Music track-list workspace, or another centrally mapped
  provider-specific style, including its preview versus focusable child state
- **THEN** the screen supplies the typed content and interaction state to the shared
  arrangement or component
- **AND** the arrangement owns pane placement, sizing, spacing, and responsive layout
- **AND** the screen does not invent another additional-content style or supply
  screen-local rectangles, row arithmetic, breakpoints, or renderer callbacks
- **AND** any approved customisation is implemented by the central owning component
  or arrangement, not by the surface

#### Scenario: An Inline hero displays an image
- **WHEN** an Inline hero has an image
- **THEN** the shared hero component places it against the top and right edges
- **AND** text reserves a one-column gutter to its left and a one-row gutter below it
- **AND** the surface may supply image dimensions but not placement or gutter geometry

### Requirement: Structural variation uses an approved vocabulary

Structural or visual differences SHALL be represented by centrally defined named
variants or policies. A screen SHALL NOT introduce arbitrary component geometry,
colours, styles, borders, or renderer callbacks as a local override. Any approved
override SHALL live in the central component, arrangement, theme, or named bespoke
component that owns it; surface code may only select the named option and provide
semantic data.

#### Scenario: An existing component has a legitimate structural difference
- **WHEN** a screen requires a difference in layout, spacing, image placement, or
  decoration
- **THEN** the difference is selected through an existing approved policy or added as
  a centrally defined variant
- **AND** the component continues to own the resulting geometry and painting

#### Scenario: A requested difference is content-only
- **WHEN** a screen differs only in displayed semantic content
- **THEN** the difference is represented in the screen model
- **AND** no new visual variant is created

### Requirement: Palette primitives are not a public API

Raw `Color` primitives SHALL be private to the theme module. Semantic roles SHALL be
the only public styling API. Components SHALL consume roles or component style
policies; screens SHALL NOT pass arbitrary `Color` or `Style` values into shared
components.

#### Scenario: A component renders focused and unfocused states
- **WHEN** the component receives its focus state
- **THEN** it resolves the appropriate semantic surface and text styles through a role
- **AND** the screen does not select independent foreground and background colours

#### Scenario: No existing role fits a call site
- **WHEN** a call site cannot be expressed with an existing role
- **THEN** a named role carrying the visual meaning is added to the theme vocabulary
- **AND** the primitive is not re-exported and no screen-local colour alias is created

### Requirement: Bespoke rendering is explicit

Rendering that cannot use an existing component, arrangement, or approved variant
SHALL be isolated as a named bespoke component with a documented reason and its own
buffer coverage. A bespoke component is exempt from reuse only; it remains subject to
the ownership, semantic styling, and verification requirements.

#### Scenario: A surface cannot use the design system
- **WHEN** implementation requires a bespoke painter
- **THEN** the surface is placed in an explicitly designated bespoke component
- **AND** its reason, visual contract, and test coverage are reviewable

### Requirement: UI development guidance is discoverable

The repository SHALL provide mandatory UI rules in `AGENTS.md` and a committed
`mbv-frontend` skill covering the component ownership model, the controlled-override
vocabulary, the reuse workflow, and verification expectations.

#### Scenario: An agent starts a TUI change
- **WHEN** an agent begins modifying a terminal UI screen
- **THEN** the repository guidance directs it to the `mbv-frontend` workflow
- **AND** the workflow requires checking for an existing component or arrangement
  before adding rendering code

#### Scenario: An agent completes a UI change
- **WHEN** an agent reports a UI change as complete
- **THEN** the guidance requires checking component ownership, narrow-width behaviour,
  interaction targets, and buffer tests where applicable

### Requirement: Hero content is provider-neutral at the UI layer

The UI SHALL render hero content through one provider-neutral abstraction. Every provider that
can supply a hero — Emby items and Audiobookshelf entries alike — SHALL satisfy that one
abstraction, and the hero renderer SHALL NOT branch on which provider supplied the item, nor
select geometry, extent, or layout from the provider's identity. Provider differences SHALL be
expressed as content the abstraction carries. The abstraction SHALL carry title, ordered metadata,
optional description, and semantic artwork only; it SHALL NOT carry a structured listing, a layout
rect, or interactive list state. A shared arrangement SHALL provide named artwork, overview, and
optional media-list viewport slots. A destination component SHALL own and render its existing
embedded `WideMediaList` into a media-list slot.

A hero that requests landscape artwork SHALL express that semantic aspect rather than a
provider-specific field name. The provider adapter SHALL resolve its verified image candidates
and fallback order; the layout SHALL own the landscape ratio.

While image rendering is enabled, a hero SHALL always present an image region. When the selected
item has no artwork, the region SHALL be filled by a single shared placeholder owned by the theme
and its central component, not by a per-provider or per-surface substitute. A hero SHALL NOT
render an empty image region.

While image rendering is disabled, the image region SHALL be absent entirely rather than
placeheld, and the hero's text and metadata SHALL occupy the space it would have taken. That
collapse SHALL be resolved in one place and SHALL be identical on every hero-bearing surface; no
surface SHALL retain a reserved or placeheld image region when images are off.

#### Scenario: Two providers render the same hero shape

- **WHEN** an Emby item and an Audiobookshelf entry are each rendered as the hero of the same
  presentation
- **THEN** both are rendered by the same code path through the shared hero abstraction
- **AND** the renderer contains no branch on the provider that supplied the item

#### Scenario: A structured Hero preserves list ownership

- **WHEN** a Hero-bearing destination presents episodes, tracks, or chapters
- **THEN** Hero text renders in its overview slot
- **AND** the destination's existing `WideMediaList` renders the structured rows in the separate
  media-list slot
- **AND** the Hero abstraction contains no list target, cursor, scroll, or hit state

#### Scenario: A Hero requests landscape artwork

- **WHEN** a Hero presentation requires landscape artwork
- **THEN** its layout uses the shared landscape geometry
- **AND** its provider adapter resolves artwork through its verified candidate chain rather than
  exposing a provider-specific image field to the layout

#### Scenario: A provider's hero cannot resize its container

- **WHEN** a hero's content is shorter or taller than the container it is placed in
- **THEN** the container's geometry is unchanged
- **AND** the hero renderer does not adjust a container rect it was given

#### Scenario: An item has no artwork

- **WHEN** a hero renders an item for which no artwork exists and image rendering is enabled
- **THEN** the shared placeholder fills the image region
- **AND** the region's geometry matches what real artwork would have occupied

#### Scenario: Image rendering is disabled

- **WHEN** any hero-bearing surface renders with image rendering disabled
- **THEN** no image region is reserved, painted, or placeheld
- **AND** the hero's text and metadata occupy the full content width

#### Scenario: Two surfaces render with images disabled

- **WHEN** two different hero-bearing surfaces render with image rendering disabled
- **THEN** both collapse the image region the same way

### Requirement: Common bypasses are mechanically visible

The repository SHALL include path-scoped source checks that flag direct Ratatui
painting, layout-rect construction, and buffer access inside screen modules.

The repository SHALL additionally include a source check that rejects supplying a bare colour
value where a style is expected, because that silently sets the foreground and leaves the
intended background unpainted. A call site SHALL state the styling role it is setting
explicitly.

These checks SHALL run unscoped over the whole repository in continuous
integration, and the tree SHALL be clean of their findings. A build SHALL NOT
narrow the scanned path in order to pass, and a standing violation count SHALL
NOT be treated as an accepted baseline: a new bypass fails the build rather than
being absorbed into existing findings. Code that a check flags is either moved
to its owning component or arrangement, or moved out of the checked path because
it was never screen code — never left in place behind a narrowed scan.

These checks catch the common bypass only. Duplicated arrangement geometry and hit
targets that have drifted from their painting are not statically detectable and
SHALL be named in the review checklist as review's responsibility. Buffer tests
verify component behaviour and preserved output; they do not by themselves establish
conformance.

#### Scenario: A screen bypasses a canonical painter
- **WHEN** a change adds direct rendering or rect construction in a screen module
- **THEN** the source check identifies the bypass
- **AND** the change cannot be treated as conforming without moving the code to its
  owning component or arrangement

#### Scenario: A bare colour is supplied where a style is expected
- **WHEN** a change paints a block or widget by supplying a bare colour value in place of a style
- **THEN** the source check identifies it
- **AND** the change is not conforming until the call site names the foreground or background
  role it intends to set

#### Scenario: The checks are enforced over the whole tree
- **WHEN** continuous integration runs the architecture-boundary job
- **THEN** it runs the source checks across the whole repository rather than a
  subset of paths
- **AND** the job fails if any check reports a finding anywhere in the tree

#### Scenario: A bypass cannot be absorbed into a standing baseline
- **WHEN** a change would add a finding to a check that already reports findings
  elsewhere
- **THEN** the build fails on the new finding
- **AND** narrowing the scanned path, suppressing the rule, or raising an accepted
  violation count is not a conforming resolution

#### Scenario: Flagged code is not screen code
- **WHEN** a check flags code that owns geometry or painting but sits in a screen
  module for historical reasons
- **THEN** the code is rehomed to the arrangement, component, or shell module that
  its signature identifies as its owner
- **AND** the observable painted output is unchanged

### Requirement: Named primary media browsers reuse the canonical list controls

Home, the hero-bearing generic Emby library catalog browser, Movies, TV Series browsing, grouped Music album browsing, the Emby homevideos feed view, the Emby podcast channel list, Audiobookshelf Podcast show browsing, Audiobookshelf Book browsing, Feeds, and Queue's fixed-row list SHALL compose the applicable canonical control for shared cursor, scroll, viewport, movement, fixed-row painting, selection, truncation, and scrollbar behavior.

A destination SHALL NOT copy those mechanics into its own painter merely because its content comes from another provider or has different metadata. Queue composes fixed-row behavior only; non-hero two-column browsers remain governed by their existing column policy.

#### Scenario: A new provider hero browser is added

- **WHEN** a provider destination displays selectable hero-bearing media rows
- **THEN** it maps its content into the canonical row vocabulary
- **AND** it composes the canonical control appropriate to its responsive presentation
- **AND** provider identity alone is not accepted as a reason for bespoke list rendering

#### Scenario: A shared list behavior changes

- **WHEN** canonical row placement, truncation, selection, or scrolling changes
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

- **WHEN** an exception independently implements cursor visibility, fixed-row placement, truncation, selection, or scrollbar behavior that the canonical control already provides
- **THEN** the exception is non-conforming
- **AND** those mechanics are moved back to the canonical control or reused from it

### Requirement: Each implementation slice proves composition before deleting loops

Every implementation slice SHALL identify the exact destinations it migrates, preserve or improve relevant existing characterization, add focused structural checks for realistic uncovered drift, and record manual end-to-end evidence for that slice's destinations before acceptance. Old loops SHALL be removed during the implementation so tests and source-level one-painter review exercise the actual replacement. Existing characterization alone SHALL NOT be treated as sufficient when its fixture omits the metadata, grouping, active state, breakpoint transition, or image behavior being migrated.

#### Scenario: A destination slice migrates

- **WHEN** a slice replaces a destination's bespoke fixed-row loop
- **THEN** focused automated evidence confirms the destination composes the correct canonical control and preserves its structural behavior
- **AND** manual evidence covers the destination's affected Wide and Narrow presentations, focus, movement, and prepared image behavior
- **AND** the old loop is absent from the implementation exercised by both forms of evidence

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

Each migrated primary media-list surface SHALL have exactly one canonical list painter for its body at each layout breakpoint and no destination-specific duplicate row geometry. A slice SHALL NOT treat a surface as migrated while a legacy list painter still runs for that surface body in the same frame.

#### Scenario: Painter ownership is reviewable

- **WHEN** a reviewer traces a migrated surface's render path at a given breakpoint
- **THEN** the path reaches the embedded canonical control exactly once
- **AND** no legacy list painter runs for that surface body

### Requirement: Implementation and acceptance form one verified slice

For every slice that changes a rendered media-list surface, implementation, representative stateful and rendered tests, automated gates, review, and acceptance SHALL form one uninterrupted slice. There SHALL be no pre-test visual-approval checkpoint. Live Wide/Narrow review remains required before acceptance; a visual defect found during review or acceptance SHALL be treated as a bug, fixed, and followed by rerunning the affected tests and gates.

#### Scenario: Tests and gates precede acceptance

- **WHEN** a slice changes a media-list surface
- **THEN** focused stateful, rendered, and geometry evidence uses metadata-, group-, state-, image-, and breakpoint-bearing fixtures where applicable
- **AND** automated gates run before review and acceptance

#### Scenario: Live review finds a defect

- **WHEN** Wide/Narrow live review reveals incorrect output or interaction after tests pass
- **THEN** the defect is fixed as part of the same slice
- **AND** the affected tests and gates are rerun before acceptance

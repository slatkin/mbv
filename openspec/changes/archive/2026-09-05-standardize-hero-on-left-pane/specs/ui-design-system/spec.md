## ADDED Requirements

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

## MODIFIED Requirements

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

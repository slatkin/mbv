## Purpose

The UI design-system capability keeps mbv's terminal surfaces visually consistent
while allowing screens to provide their own content and explicitly approved
semantic variations.

These requirements bind all new UI code and any surface a change touches, from the
PR that lands the module split forward. Surfaces not yet migrated are listed in the
change's `ledger.md`; that list may only shrink.

## ADDED Requirements

### Requirement: Screens use canonical UI ownership boundaries

The UI SHALL separate screen content, arrangement geometry, and component painting.
Screens SHALL provide semantic content and approved variants; arrangements SHALL own
shared geometry; components SHALL own their painting and styling. Screen modules
SHALL NOT call Ratatui or construct layout rectangles.

Hit-target ownership is outside this capability's scope. Existing app-layout and input
resolution remains authoritative; this change SHALL NOT introduce a partial migration
to arrangement- or component-published hit maps.

Classification is by signature: a function taking app state and returning a typed
content model is screen code; a function taking a typed content model, a `Rect`, and
a buffer is a component; a function placing components within a `Rect` and owning
breakpoints is an arrangement.

#### Scenario: A screen adds content without duplicating a painter
- **WHEN** a screen needs different titles, metadata, rows, or images
- **THEN** it supplies a screen model to an existing arrangement or component
- **AND** it does not copy the arrangement's geometry or painter

#### Scenario: Existing hit-target resolution is preserved
- **WHEN** a surface is migrated under this capability
- **THEN** its existing app-layout and input hit-target resolution remains in use
- **AND** no partial arrangement or component hit-map migration is introduced

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

### Requirement: Common bypasses are mechanically visible

The repository SHALL include path-scoped source checks that flag direct Ratatui
painting, layout-rect construction, and buffer access inside screen modules.

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

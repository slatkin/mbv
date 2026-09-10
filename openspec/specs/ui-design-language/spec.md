# ui-design-language Specification

## Purpose

Defines one shared source of truth for the TUI's colour roles, so that a visual decision is made
once and applies everywhere, and so a screen cannot invent its own answer to a question the design
language already answers.

## Requirements

### Requirement: Colour roles have one definition

The TUI SHALL define its colours as named roles, and every surface, text run, rule, and indicator
SHALL derive its colour from a role rather than from a literal colour value. Changing a role's
definition SHALL change every place that role is used, with no per-screen implementation of the same
role.

#### Scenario: A role definition changes

- **WHEN** the definition of a colour role is changed
- **THEN** every screen using that role renders the changed colour
- **AND** no screen continues to render the previous colour

#### Scenario: Two screens present the same concept

- **WHEN** two screens display the same concept, such as a selected row or a resting panel
- **THEN** both derive that colour from the same role and render identically

### Requirement: Raw colour primitives are private

Raw colour primitives, including literal `Color` values and hue-named constants, SHALL be private to
the theme module. Modules outside the theme SHALL consume semantic roles or component style policies;
the theme SHALL NOT re-export raw primitives as a styling API.

#### Scenario: A component needs a colour

- **WHEN** a component requires a visual colour
- **THEN** it obtains that colour from a semantic role or named component style policy
- **AND** it does not import a raw colour primitive

#### Scenario: A raw primitive changes

- **WHEN** a raw colour primitive changes inside the theme
- **THEN** only the semantic roles that reference it expose the change
- **AND** modules outside the theme do not gain direct access to the primitive

### Requirement: Focus state colouring is centrally controlled

The focused and unfocused appearance of every panel, sub-panel, list, and component SHALL be
determined in one place from a focus state supplied by the caller. A screen SHALL supply the
panel's focus state — the existing `PanelFocus` plus, for Wide hero screens, a pane bit — and
SHALL NOT name the colour used for any state. This SHALL apply to the left panel's card and queue
as well as to right-panel content.

#### Scenario: The focused appearance is changed

- **WHEN** the definition of the focused appearance is changed in one place
- **THEN** every panel and sub-panel in the application renders the changed appearance, including
  the queue and card
- **AND** no screen requires an individual edit

#### Scenario: A screen reports its focus state

- **WHEN** a screen renders with a given panel and pane focus state
- **THEN** its appearance is chosen by the shared definition from that focus state alone

### Requirement: One surface table maps a surface to its colour

The theme SHALL expose one closed set of rendered-surface identities and one function resolving a
surface plus the focus state to its fill and border. Every production paint site SHALL obtain a
surface's colour from that function by naming the surface. A screen SHALL NOT name a colour role, call
a colour resolver directly, or branch on a focus bit to choose between colours. Each surface entry
SHALL state its nesting level, and a role SHALL NOT be reachable from a screen by any other path.

#### Scenario: A level's appearance changes

- **WHEN** the definition of one nesting level's appearance is changed in the theme
- **THEN** every surface of that level renders the changed appearance
- **AND** surfaces of other levels do not change
- **AND** no screen requires an individual edit

#### Scenario: A screen tries to choose a colour

- **WHEN** a screen would name a colour role, call a colour resolver, or pick a colour from a focus
  bit
- **THEN** the build fails
- **AND** the screen instead names its surface identity and passes the focus state it was given

#### Scenario: A surface has no painter

- **WHEN** a surface identity exists in the table but no painted rect matches it in a layout under
  test
- **THEN** the conformance test fails
- **AND** the surface is either painted or removed from the table

#### Scenario: The selection moves inside a focused pane

- **WHEN** a pane holds focus and the selection moves between sub-panels inside it, for example from
  a series list to its episode list or from a music browser to its track list
- **THEN** no surface's fill changes
- **AND** the sub-panel that gained the cursor shows its selected row and the other does not
- **AND** the focus state supplied to the screen is the only input that changed the appearance

#### Scenario: One focus state drives every surface

- **WHEN** a frame renders with a given focus state
- **THEN** every surface's appearance is derived from that one value
- **AND** no screen derives its own focus state for colour

### Requirement: Per-screen colour exceptions are named variants

A screen MAY deviate from a default colour role only by opting into a variant that is itself defined once alongside the roles. A screen SHALL NOT supply a literal colour value at the point of use. A second screen needing the same deviation SHALL reuse the existing variant rather than introduce another.

The inline hero SHALL render one content shape on every surface: title, optional metadata, optional overview, and an optional image. The image model SHALL be selected by image aspect ratio, not by surface identity. No surface SHALL add a bespoke content path, extension block, in-hero pill bar, or hand-painted metadata block that bypasses the shared hero content component. The closed structural vocabulary for the inline hero is the shared hero content model (Model A: right-aligned, wrap-around) and the shared beside-image model (Model B: right-half, meta-column). No surface SHALL introduce a third content model or a third image placement.

#### Scenario: A screen needs a colour that differs from the default

- **WHEN** a screen requires an appearance the default role does not provide
- **THEN** it opts into a named variant defined with the roles
- **AND** the variant is available to any other screen by the same name

#### Scenario: A variant definition changes

- **WHEN** a variant's definition is changed
- **THEN** every screen opted into that variant renders the change

#### Scenario: A surface attempts a bespoke inline hero content path

- **WHEN** a surface would render inline hero content through a path other than the shared hero content component (Model A) or the shared beside-image component (Model B)
- **THEN** the surface SHALL route through the shared component instead
- **AND** no bespoke content path, extension block, or hand-painted metadata block SHALL exist

#### Scenario: A surface attempts an in-hero pill bar

- **WHEN** a surface would render filter or navigation pills inside the inline hero content
- **THEN** the pills SHALL move to the panel area
- **AND** no pills SHALL render inside the inline hero

#### Scenario: A panel pill bar reserves a spacer row

- **WHEN** a panel reserves vertical space between a pill bar and its browser content
- **THEN** the pill bar SHALL paint exactly one row
- **AND** the spacer row SHALL inherit the parent panel background
- **AND** surfaces SHALL NOT extend the pill-row background into the spacer
- **AND** pill hit targets SHALL be exactly one row high
- **AND** each designated pill area SHALL have exactly one render owner

#### Scenario: A surface with a tall image uses the wrong model

- **WHEN** a surface with a tall image (poster, book cover) would use the beside-image model (Model B)
- **THEN** it SHALL use the right-aligned wrap-around model (Model A) instead
- **AND** the image SHALL be right-aligned with text wrapping around it row by row

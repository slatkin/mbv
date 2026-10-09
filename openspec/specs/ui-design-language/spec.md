# ui-design-language Specification

## Purpose

Defines one shared source of truth for the TUI's colour roles, so that a visual decision is made
once and applies everywhere, and so a screen cannot invent its own answer to a question the design
language already answers.

## Requirements

### Requirement: Colour roles have one definition

The TUI SHALL define its colours as named roles and surface rows, and every surface, text run,
rule, and indicator SHALL derive its colour from a role or a surface row rather than from a literal
colour value. Changing a role's or surface row's definition SHALL change every place it is used,
with no per-screen implementation of the same role or row.

#### Scenario: A role definition changes

- **WHEN** the definition of a colour role or surface row is changed
- **THEN** every screen using that role or row renders the changed colour
- **AND** no screen continues to render the previous colour

#### Scenario: Two screens present the same concept

- **WHEN** two screens display the same concept, such as a selected row or a resting panel
- **THEN** both derive that colour from the same role or surface row and render identically

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

### Requirement: One surface table maps a surface to its fill

The theme SHALL expose one closed set of rendered-surface identities and one function resolving a
surface plus a focus boolean to its fill. Every production paint site SHALL obtain a background fill
from that function by naming the surface. Each surface entry SHALL state exactly two slots, resting
and focused; a surface that does not react to focus states the same slot twice. Roles SHALL serve
only as foregrounds, apart from the permitted inversions.

#### Scenario: A surface row changes

- **WHEN** one surface's resting or focused slot is changed in the theme
- **THEN** every site painting that surface renders the changed appearance
- **AND** surfaces with their own rows do not change
- **AND** no screen requires an individual edit

#### Scenario: A background is painted from a role

- **WHEN** a paint site would fill a background, stripe, bar, or chip from a role
- **AND** the fill is not an inversion permitted by "Inverted spans swap a role and a fill"
- **THEN** review rejects it
- **AND** the site names a surface row instead, adding one if no row names the concept

### Requirement: Inverted spans swap a role and a fill

A reverse-video span MAY paint a foreground role as its fill when its text is the on-accent role;
the indicator Chips and Powerline treatments paint the same status roles that the other treatments
paint as text. An edge glyph that draws a surface's outline MAY paint that surface's fill as its
foreground. No other site SHALL fill from a role or take a foreground from a surface.

#### Scenario: An indicator chip is painted in reverse video

- **WHEN** the Chips or Powerline indicator treatment paints a status indicator
- **THEN** the chip fill is the same role the text treatments paint that indicator with
- **AND** the chip text is the on-accent role
- **AND** no surface row duplicates the indicator's role

#### Scenario: A pill edge glyph is painted

- **WHEN** a pill shell paints its slanted edge glyphs
- **THEN** each glyph's foreground is the pill's surface fill

### Requirement: A surface follows its paint site's own focus condition

The focus boolean passed to the surface table SHALL be the call site's own focus input, exactly the
condition the site used before the table existed. The table SHALL NOT change whether a surface
reacts to focus, nor which signal drives the reaction.

#### Scenario: A site migrates onto the table

- **WHEN** a production paint site is moved onto the table
- **THEN** the site passes the same focus condition it used before, unchanged
- **AND** with that condition true the surface renders the focused appearance it rendered before
- **AND** with that condition false the surface renders the resting appearance it rendered before

#### Scenario: A cursor-driven site keeps its predicate

- **WHEN** a site's colour followed a cursor predicate rather than Panel focus, for example a
  detail pane's episode box that lights only while an episode cursor is active
- **THEN** the site keeps that predicate as its focus input
- **AND** the table does not substitute Panel focus or any other signal for it

### Requirement: One concept takes one colour identity

A concept painted on more than one screen SHALL use one role or one surface row everywhere it
appears. A second identity SHALL NOT be created for an existing concept only so that its colour can
be edited separately; a deliberate visual difference SHALL be a named variant that states the
difference.

#### Scenario: Two lists paint the same stripe

- **WHEN** two lists paint the same secondary zebra fill
- **THEN** both name the same surface row

#### Scenario: Two paint sites paint the same selection bar

- **WHEN** a list's selected row and a popup's selected row paint the same selection bar
- **THEN** both name the same surface row

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

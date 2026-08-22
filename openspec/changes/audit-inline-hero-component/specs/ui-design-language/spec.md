## MODIFIED Requirements

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

#### Scenario: A surface with a tall image uses the wrong model

- **WHEN** a surface with a tall image (poster, book cover) would use the beside-image model (Model B)
- **THEN** it SHALL use the right-aligned wrap-around model (Model A) instead
- **AND** the image SHALL be right-aligned with text wrapping around it row by row

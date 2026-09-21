## Purpose

Covers the palette side of the 2026-09-20 7.4 paint work (user-approved as necessary): the palette is one closed, meaning-free colour vocabulary defined once in the theme, with `docs/palette.json` as a generated mirror kept honest by test. No existing main-spec clause contradicts the landed behaviour, so this is coverage, not reconciliation.

## ADDED Requirements

### Requirement: The palette is one closed meaning-free colour vocabulary

The theme SHALL own one closed palette enum: one variant per distinct colour, each variant carrying
exactly one `Rgb` literal, and no meaning of its own. Variants SHALL be named neutrally (colour
names, not roles); all visual meaning SHALL live in semantic roles and surface rows, which are
compiler-visible assignments over the enum. The palette SHALL be additive only: production code
consumes it through `color()` from roles and surface rows, never through an indexable table, and no
`Color::Rgb` literal for a palette colour SHALL appear outside the enum. Ratatui's own raw `Color::`
specials (Black/White/Reset) SHALL stay outside the palette as mechanics. `docs/palette.json` SHALL
be the generated mirror of the enum, kept honest by a drift test, and SHALL NOT be edited as an
independent source. Adding, restoring, or re-valuing a variant or re-assigning a role SHALL happen in
the theme alone, with the mirror regenerated to match.

#### Scenario: A variant is added or re-valued

- **WHEN** a distinct colour enters the UI, or an existing variant's value is restored or changed
- **THEN** the change lands as one enum variant with its single `Rgb` literal in the theme
- **AND** `docs/palette.json` is regenerated to match and the drift test passes
- **AND** no screen or component gains a second copy of the value

#### Scenario: A colour is consumed

- **WHEN** a surface or text run needs a palette colour
- **THEN** it names a semantic role or surface row that assigns the variant
- **AND** it never names the variant or a hex value directly outside the theme

#### Scenario: The mirror drifts from the enum

- **WHEN** `docs/palette.json` disagrees with the theme's palette enum
- **THEN** the drift test fails
- **AND** the fix is regenerating the mirror from the enum, never hand-editing one side to silence it

## MODIFIED Requirements

### Requirement: Independence from top-hero design

Hero placement is a property of the right panel's arrangement, not of the list renderer. The
hero's content — image, metadata, and overview — SHALL be identical regardless of where the
arrangement places it. Only the position changes. The two placements SHALL NOT be maintained as
parallel branches within the list renderer for comparison; the arrangement selects one.

The positional variant MUST preserve the same hero content as the top-hero design.

#### Scenario: Hero content remains consistent

- **WHEN** an item is shown in the inline hero
- **THEN** its image, metadata, and overview SHALL match the top-hero content

#### Scenario: Placement changes

- **WHEN** the arrangement places the hero differently
- **THEN** the hero's content is unchanged and only its position differs

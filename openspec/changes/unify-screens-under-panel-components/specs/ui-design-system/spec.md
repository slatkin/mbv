## MODIFIED Requirements

### Requirement: Structural variation uses an approved vocabulary

Structural or visual differences between screens SHALL come only from the shape of the typed content a
screen supplies to a panel's slots (for example, an item's artwork kind selecting its Wide Hero header
type, or a slot left empty). A screen SHALL NOT select a variant, policy or surface that changes the
skeleton, geometry, colours, styles or borders of a panel or slot, and SHALL NOT introduce arbitrary
component geometry, colours, styles, borders, or renderer callbacks as a local override. A named
variant or policy is admissible only when it is derived from content or state that every screen can
supply; a caller-selected variant arm whose only user is one screen is a defect to remove, not
approved vocabulary.

#### Scenario: An existing component has a legitimate structural difference
- **WHEN** a screen requires a difference in layout, spacing, image placement, or decoration
- **THEN** the difference is either expressed as content every screen can supply to the same slot, or
  applied to every screen through the owning panel
- **AND** the panel continues to own the resulting geometry and painting

#### Scenario: A requested difference is content-only
- **WHEN** a screen differs only in displayed semantic content
- **THEN** the difference is represented in the screen's content
- **AND** no new visual variant is created

#### Scenario: A variant arm has one user
- **WHEN** a presentation variant or policy value is selected by exactly one screen
- **THEN** it is non-conforming and is removed by conforming that screen to the shared presentation

## MODIFIED Requirements

### Requirement: Canonical controls are the sole list painter
Migrated Home and Feeds destinations SHALL provide one-painter evidence: each applicable surface has one canonical list painter and no destination-specific duplicate row geometry.

#### Scenario: Painter ownership is reviewable
- **WHEN** a reviewer traces a Home or Feeds render path
- **THEN** the path reaches the embedded canonical control once, with no legacy list painter or second hit-coordinate calculation.

### Requirement: Visual verification precedes UI tests
Implementation SHALL characterize current behavior first, perform visual correction at Wide and Narrow widths, and obtain explicit user live verification before adding or updating UI tests.

#### Scenario: Tests follow approval
- **WHEN** visual behavior is corrected and explicitly approved
- **THEN** focused rendered tests may be added using metadata-, group-, state-, image-, and breakpoint-bearing fixtures.

## ADDED Requirements

### Requirement: Responsive grouped-view continuity

The narrow hero-above-list composition and wide side-hero composition SHALL consume the same settled grouped snapshot and album selection. Changing composition SHALL NOT restart artist metadata resolution, publish a different grouping for the same snapshot, or replace the selected album when it remains available.

#### Scenario: Grouped Music crosses the responsive breakpoint
- **WHEN** terminal resizing switches grouped Music between its narrow and wide compositions
- **THEN** the same settled grouping and selected album remain in use and the active album viewport is clamped around that selection

#### Scenario: Responsive composition redraws
- **WHEN** either responsive composition redraws without a changed album snapshot
- **THEN** it reuses the existing settled grouping without starting artist metadata resolution work

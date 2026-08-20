## MODIFIED Requirements

### Requirement: Grouped Music uses responsive compositions

The grouped Music album view SHALL use hero-on-left when it meets the shared wide geometry conditions. Its left pane SHALL contain album detail and tracks, and its right rail SHALL contain a single-column album browser. Otherwise the selected album detail and tracks SHALL replace the active album row in a single-column browser. Grouped Music SHALL NOT evaluate the breakpoint or minimum-height guard itself and SHALL NOT use a separate fallback.

#### Scenario: Grouped Music below the breakpoint
- **WHEN** grouped Music does not meet the shared wide geometry conditions
- **THEN** group pills span the content width
- **AND** albums render one per row
- **AND** the selected album's hero and track detail replace its active row

#### Scenario: Grouped Music at the breakpoint
- **WHEN** grouped Music meets the shared wide geometry conditions
- **THEN** it renders hero-on-left

#### Scenario: Grouped Music lacks sufficient height
- **WHEN** grouped Music meets the width breakpoint but fails the existing minimum-height guard
- **THEN** it renders the inline presentation
- **AND** it does not pin album detail above the browser

#### Scenario: Non-Music library at wide width
- **WHEN** another hero-bearing library meets the shared wide geometry conditions
- **THEN** it also renders hero-on-left with a one-column right-rail browser

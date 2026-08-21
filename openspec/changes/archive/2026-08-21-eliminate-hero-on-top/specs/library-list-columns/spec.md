## MODIFIED Requirements

### Requirement: Column count derives from the list pane width

The library list SHALL choose its column count from the width available to the list pane itself, not from terminal width, and SHALL use at most two columns. The shared responsive breakpoint SHALL NOT be derived from minimum cell width. A hero-bearing browse surface SHALL render a single-column browser in both presentations: in the right rail for hero-on-left and in the full-width list for inline hero. A non-hero list MAY use two columns at or above the shared breakpoint and SHALL use one column below it.

#### Scenario: Wide hero-bearing browser
- **WHEN** a hero-bearing browse surface meets the wide geometry conditions
- **THEN** its right-rail browser SHALL render one column

#### Scenario: Inline hero browser
- **WHEN** a hero-bearing browse surface uses the inline presentation
- **THEN** its browser SHALL render one column
- **AND** the selected hero SHALL replace the active item row as one full-width flow segment

#### Scenario: Wide non-hero list pane
- **WHEN** a non-hero library list pane reaches the shared breakpoint
- **THEN** it MAY render two columns of items

#### Scenario: Narrow list pane
- **WHEN** any library list pane is below the shared breakpoint
- **THEN** the list SHALL render a single column

#### Scenario: Queue column resized or collapsed
- **WHEN** the queue column is widened, narrowed, or collapsed, changing the width available to the list pane
- **THEN** the active presentation and permitted column count SHALL be recomputed on the next frame

#### Scenario: The shared breakpoint is changed
- **WHEN** the shared breakpoint value is changed
- **THEN** every right-panel browse surface switches presentation at the new width without a surface-specific edit

#### Scenario: Wide list pane
- **WHEN** a non-hero list pane reaches the shared breakpoint
- **THEN** it MAY render two columns while hero-bearing browsers remain one column

#### Scenario: Hero-on-left list stays single-column
- **WHEN** a hero-on-left browser is at or above the shared breakpoint
- **THEN** its right rail SHALL render a single column

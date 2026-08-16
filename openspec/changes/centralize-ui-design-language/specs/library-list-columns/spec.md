## MODIFIED Requirements

### Requirement: Column count derives from the list pane width

The Power View library list SHALL choose its column count from the width available to the list pane
itself, not from the terminal width, and SHALL use at most two columns. The width at which it
changes between one and two columns SHALL be the right panel's single shared breakpoint. That
breakpoint SHALL NOT be derived from the list's minimum cell width; instead the cell width SHALL be
whatever the breakpoint leaves. When the available width falls below the breakpoint the list SHALL
render a single column.

#### Scenario: Wide list pane

- **WHEN** the library list pane reaches the shared breakpoint
- **THEN** the list SHALL render two columns of items

#### Scenario: Narrow list pane

- **WHEN** the library list pane is below the shared breakpoint
- **THEN** the list SHALL render a single column with the same appearance and behaviour it has today

#### Scenario: Queue column resized or collapsed

- **WHEN** the queue column is widened, narrowed, or collapsed, changing the width left over for the
  list pane
- **THEN** the column count SHALL be recomputed from the new list pane width on the next frame

#### Scenario: The shared breakpoint is changed

- **WHEN** the shared breakpoint value is changed
- **THEN** the list changes column count at the new width without any other change to the list

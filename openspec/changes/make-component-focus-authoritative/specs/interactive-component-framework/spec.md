## ADDED Requirements

### Requirement: Mounted-component focus has one authority

Whether a mounted Interactive Component is focused SHALL be determined by the application's component-focus lifecycle. A component whose input handling or presentation depends on focus SHALL observe that lifecycle directly. The shell SHALL NOT also carry the same focused state in content projections, refresh content solely to change focused presentation, or maintain a second component-focus mirror.

Component-private pane focus, cursor, scroll, and selection SHALL remain owned by the component while mounted. Losing component focus SHALL suppress focused presentation without erasing those local values; regaining component focus SHALL immediately restore keyboard delivery and derive the focused pane from the component's retained local state.

A plain embedded Component SHALL share the mounted parent's focus boundary rather than becoming an independently focused application surface.

#### Scenario: Panel focus moves from Music to Queue and back

- **WHEN** the Music destination is mounted and Panel focus moves from Library to Queue and then back to Library
- **THEN** Music loses focused presentation while Queue holds focus
- **AND** Music receives keyboard navigation immediately when Library regains focus, without requiring a click or content refresh
- **AND** its component-private cursor, scroll, and pane focus remain as they were before the round trip

#### Scenario: Wide TV loses focused row treatment

- **WHEN** Wide TV is mounted with a selected series row and Panel focus moves from Library to Queue
- **THEN** the TV library rail loses its focused background, marker, and selected-row treatment on the next frame
- **AND** its selected series identity remains available for when Library focus returns

#### Scenario: Content refresh cannot overwrite focus

- **WHEN** shell-owned content is pushed to a mounted component while another component holds focus
- **THEN** the content refresh does not make the receiving component appear or behave focused
- **AND** a later focus transition does not require that content to be pushed again

#### Scenario: Overlay focus restores the underlying component

- **WHEN** a blocking overlay takes focus from a mounted destination and is then dismissed
- **THEN** the destination loses focused presentation while the overlay is active
- **AND** the application's focus restoration makes the destination focused again without a focus-only content projection

#### Scenario: Embedded controls share parent focus

- **WHEN** a mounted destination composes a plain embedded list, browser, or text-entry control
- **THEN** the embedded control is treated as focused only when its mounted parent and the applicable component-private pane are focused
- **AND** the embedded control is not mounted or focused independently

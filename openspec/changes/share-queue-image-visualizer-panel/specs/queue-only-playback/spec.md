## MODIFIED Requirements

### Requirement: Narrow layout stacks image and playback panel vertically

When the terminal width is less than 100 columns, the queue visual slot SHALL appear as a full-width row above the playback panel and queue list. The slot SHALL use the same rectangle for artwork or the visualizer.

#### Scenario: Narrow terminal stacks vertically

- **WHEN** the layout is in queue-only state and terminal width is less than 100 columns
- **THEN** the queue visual slot SHALL render at full column width, the playback panel SHALL render directly below it at full column width, and the queue list SHALL render below the playback panel

### Requirement: Wide layout places image and playback panel side by side

When the terminal width is 100 columns or more, the queue visual slot and playback panel SHALL render as two columns in the same row. The visual slot SHALL occupy the left column and be left-aligned. The playback panel SHALL occupy the right column. A 2-cell horizontal gap SHALL separate the two columns.

#### Scenario: Wide terminal renders two columns

- **WHEN** the layout is in queue-only state and terminal width is 100 columns or more
- **THEN** the queue visual slot SHALL render left-aligned in the left column, the playback panel SHALL render in the right column, and a 2-cell gap SHALL separate them

#### Scenario: Playback panel width uses remaining space

- **WHEN** the wide two-column layout is active
- **THEN** the playback panel width SHALL equal the total column width minus the rendered visual-slot width minus the 2-cell gap

### Requirement: Wide layout playback panel height matches image height

In the wide two-column layout, the playback panel area SHALL have the same height as the queue visual slot. Playback content SHALL be top-aligned within that area, and any remaining vertical space below the content SHALL use `DARK_BG`.

#### Scenario: Panel height matches image

- **WHEN** the wide two-column layout is active and the queue visual slot renders at N rows
- **THEN** the playback panel area SHALL also be N rows tall

#### Scenario: Content top-aligned with dark fill

- **WHEN** the playback panel area is taller than the playback content
- **THEN** the playback content SHALL start at the top of the area and `DARK_BG` SHALL fill the rows below it

### Requirement: Hero image left-aligned in wide layout

In the wide two-column layout, the queue visual slot SHALL be left-aligned within its column whether it contains artwork or the visualizer.

#### Scenario: Image left-aligned

- **WHEN** the wide two-column layout is active
- **THEN** the queue visual slot SHALL be positioned at the left edge of its column area

## REMOVED Requirements

### Requirement: Wide layout leftover space shows the visualizer when enabled

**Reason**: The visualizer now shares the queue artwork rectangle, so rendering another visualizer in playback-panel leftovers would duplicate it and retain the obsolete separate placement.

**Migration**: Fill playback-panel leftovers with `DARK_BG`; render the selected visualizer only in the queue visual slot.

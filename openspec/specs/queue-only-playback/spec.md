# queue-only-playback Specification

## Purpose

Renders the playback panel (seekbar, title, controls) inside the queue-only view, using a narrow stacked layout or a wide side-by-side layout depending on terminal width.

## Requirements

### Requirement: Playback panel renders in queue-only only when playback is active

In queue-only mode the playback panel (seekbar, title row, controls row) SHALL be rendered with `DARK_BG` background when playback is active. The panel SHALL use the same content as the standard playback panel on the right column. When playback is idle in queue-only mode, the playback panel SHALL NOT be rendered and its rows SHALL belong to the queue panel instead. A connected transport that is not currently playing is not idle for this purpose: the panel continues to render, so that its title row can still name what the session or cast target is holding.

#### Scenario: Playback panel visible in queue-only

- **WHEN** the layout is in queue-only state and playback is active or a remote session is connected
- **THEN** the playback panel SHALL be rendered with seekbar, title, and controls, using `DARK_BG` background

#### Scenario: Playback panel hidden when idle

- **WHEN** the layout is in queue-only state and no playback is active on any transport
- **THEN** the playback panel SHALL NOT be rendered and SHALL reserve zero rows, and the queue panel SHALL occupy those rows

#### Scenario: Connected transport that is not playing keeps the panel

- **WHEN** the layout is in queue-only state, a remote session or cast target is connected, and nothing is playing on it
- **THEN** the playback panel SHALL render as today and the queue visual slot SHALL NOT render

### Requirement: Narrow layout stacks image and playback panel vertically

When playback is active and the terminal width is less than 100 columns, the queue visual slot SHALL appear as a full-width row above the playback panel and queue list. The slot SHALL use the same rectangle for artwork or the visualizer. When playback is idle, neither the visual slot nor the playback panel is rendered (see the idle-collapse requirement below).

#### Scenario: Narrow terminal stacks vertically

- **WHEN** the layout is in queue-only state and terminal width is less than 100 columns and playback is active
- **THEN** the queue visual slot SHALL render at full column width, the playback panel SHALL render directly below it at full column width, and the queue list SHALL render below the playback panel

### Requirement: Wide layout places image and playback panel side by side

When playback is active and the terminal width is 100 columns or more, the queue visual slot and playback panel SHALL render as two columns in the same row. The visual slot SHALL occupy the left column and be left-aligned. The playback panel SHALL occupy the right column. A 2-cell horizontal gap SHALL separate the two columns. When playback is idle, neither column is rendered (see the idle-collapse requirement below).

#### Scenario: Wide terminal renders two columns

- **WHEN** the layout is in queue-only state and terminal width is 100 columns or more and playback is active
- **THEN** the queue visual slot SHALL render left-aligned in the left column, the playback panel SHALL render in the right column, and a 2-cell gap SHALL separate them

#### Scenario: Playback panel width uses remaining space

- **WHEN** the wide two-column layout is active
- **THEN** the playback panel width SHALL equal the total column width minus the rendered visual-slot width minus the 2-cell gap

### Requirement: Idle queue-only collapses card and panel into the queue list

When the layout is in queue-only state and no playback is active on any transport, the queue visual slot (artwork, placeholder, loading reservation, empty visualizer box) SHALL NOT be rendered and SHALL reserve zero rows, the playback panel SHALL NOT be rendered and SHALL reserve zero rows, and the queue panel SHALL occupy the rows both would have taken. The queue panel SHALL keep the single separator row it already places above itself; this requirement does not move the queue panel to the first row of the left column. Paused playback counts as active and SHALL keep both visuals. The artwork/visualizer selection SHALL persist while idle and take effect on the next playback; pressing `v` while idle SHALL NOT create a card rectangle.

#### Scenario: Idle queue-only reclaims the card and panel rows

- **WHEN** the layout is in queue-only state with a non-empty queue and no playback is active
- **THEN** no card rectangle and no playback panel SHALL be rendered above the queue, and the queue panel SHALL begin one row below the left column's content area, at the position the card occupied

#### Scenario: Idle collapse holds at both widths

- **WHEN** the layout is in queue-only state with no playback active, at a terminal width below 100 columns and at a width of 100 columns or more
- **THEN** neither the stacked narrow panel nor the wide side-by-side panel SHALL render, at both the stored-mode route and the narrow mini-view route

#### Scenario: Playback start restores card and panel

- **WHEN** playback starts from the idle queue-only state
- **THEN** the queue visual slot and playback panel SHALL render again in the active narrow or wide arrangement and the queue list SHALL move below them

#### Scenario: Paused playback keeps visuals

- **WHEN** the layout is in queue-only state and playback is paused
- **THEN** the queue visual slot and playback panel SHALL remain rendered

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
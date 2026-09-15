# queue-playback-panel Specification

## Purpose

The Queue playback panel at the top of the queue column: the header row that always states playback
status and target, the visual slot, the transport inside the queue column, how they collapse when
playback is idle, and where the Library playback panel (the right-column strip) renders instead.

## Requirements

### Requirement: Queue and Library playback panels are distinct, and exactly one transport renders

The Queue playback panel (header row, visual slot and transport in the queue column) and the Library
playback panel (the right-column strip) SHALL be two distinct panels, each with its own presentation of
the same playback content. In every frame at most one of them SHALL render a transport: the Queue
playback panel's in queue-visible layouts, the Library playback panel's when the queue column is hidden.

The Queue playback panel SHALL be present in every queue-visible layout, including while idle, because
it owns the always-painted header row. While idle it renders only the header row; its visual slot and
transport reserve zero rows. Throughout this capability and the `panel-mode` and `idle-feed-rotation`
requirements that reference it, "the playback panel" (or "the panel") in a queue-visible layout means
the Queue playback panel's transport (seekbar, title row, controls), not its header row.

#### Scenario: Idle queue-visible layout keeps only the header
- **WHEN** the queue column is visible and no transport is active
- **THEN** the Queue playback panel paints its header row and nothing else
- **AND** the Queue panel begins directly below the header row, its recessed
  top inset being the single space row between them

#### Scenario: Two-panel layout with active playback
- **WHEN** both columns are visible and playback is active
- **THEN** the transport renders in the Queue playback panel and the Library playback panel does not
  render

#### Scenario: Library-only layout with active playback
- **WHEN** the layout is library-only and playback is active
- **THEN** the Library playback panel renders the transport and no Queue playback panel renders

### Requirement: Queue-visible layouts paint a now-playing header row

In every layout that renders the queue column, the left column SHALL paint one header row at the top
of its content, above the visual slot, on the chrome surface. The header SHALL sit in the queue
column's recessed inset: one row of column surface above it and two columns of column surface on
each side, with no padding below it, so it is not flush with the column's top, left, or right edge
and its text aligns with the slot and transport below. The header SHALL always be painted, including
while playback is idle. Its left side SHALL state the playback status and its right side SHALL state
the playback target as `on <host>`, with the `on ` prefix muted and the hostname green when local
and aqua when remote. While playing, the throbber and percent SHALL paint one space right of the
status word; they SHALL NOT show while idle or paused.

The status word SHALL be `PLAYING` while playback is active and not paused, `PAUSED` while it is
active and paused, and `IDLE` while it is inactive.

The target SHALL be the same value the queue title row already resolves: this machine's device name
when playback is local, and the connected session's device name (then its host) or the direct-remote
label when it is remote. The header
SHALL follow the effective playback target — cast, then connected session, then local — and SHALL NOT
follow the queue scope being viewed.

#### Scenario: Header is recessed in the queue column

- **WHEN** a queue-visible layout is painted
- **THEN** the header SHALL sit one row below the column's top edge, inset two columns from each side
  edge, with no blank row between it and the slot/transport band below

#### Scenario: Active local playback

- **WHEN** a queue-visible layout is painted while local playback is active and not paused
- **THEN** the header row SHALL read `PLAYING` on the left and `on <device name>` on the right

#### Scenario: Paused playback

- **WHEN** a queue-visible layout is painted while playback is paused
- **THEN** the header row SHALL read `PAUSED` on the left

#### Scenario: Idle

- **WHEN** a queue-visible layout is painted while no transport is active
- **THEN** the header row SHALL read `IDLE` on the left and the resolved target on the right

#### Scenario: Active watched remote playback

- **WHEN** a queue-visible layout is painted while attached to a Session that reports a
  now-playing item the viewed queue does not hold (another device's own selection)
- **THEN** the header row SHALL read `PLAYING` and the target SHALL name that Session
- **AND** no queue row SHALL be painted as the playhead

#### Scenario: Header follows the playback target, not the viewed queue

- **WHEN** the layout is queue-visible while attached to a remote session and the local queue scope is
  selected for viewing
- **THEN** the header SHALL name the remote target, not the local device

#### Scenario: No header when the queue column is hidden

- **WHEN** the layout is library-only
- **THEN** no header row SHALL be painted

### Requirement: The queue panel carries a title band above its rows

The recessed queue panel inside the queue column SHALL paint one title band: one blank top-inset
row, the one-row `Queue` title in the foam metadata role, the `▁` block separator line in the
sage-green separator role, and one blank row above the first queue row. The title and the separator
SHALL be indented two columns from each side of the recessed box, matching the queue rows' own text
indent, so the separator starts and ends under the title rather than at the box's edges. The band
SHALL be reserved only when the box fits the band plus one queue row and the box's bottom padding;
a box too short for it SHALL keep the title-less inset.

#### Scenario: Title band separates the title from the rows

- **WHEN** a queue-visible layout with a roomy queue panel is painted
- **THEN** the panel reads `Queue` above a `▁` separator line indented two columns on each side
- **AND** one blank row sits between the separator and the first queue row

### Requirement: The playback panel renders in the queue column

In every queue-visible layout the playback panel (seekbar, title row, controls) SHALL render inside
the queue column, using the same content as the Library playback panel. The panel SHALL NOT
render in the right column of a queue-visible layout. The queue column splits the title band
across two rows: the upper row keeps the transport controls and the status pills, while the
title and the `pos / dur` time render one row below — the title left with one
space of indent, the time right with one space of indent, with the title's marquee window kept.

When the terminal is narrower than 100 columns the visual slot and the playback panel SHALL stack
vertically: the visual slot at full column width, the panel directly below it, and the queue list
below the panel. When the terminal is 100 columns or more the visual slot and the panel SHALL render
side by side in the same row, the visual slot left-aligned in the left column and the panel in the
right, separated by the existing 2-cell gap; the panel's width SHALL be the remaining column width
and its height SHALL equal the visual slot's height, with its content top-aligned and the panel
background filling the rows below it.

#### Scenario: Panel renders in the queue column in the two-panel layout

- **WHEN** the layout shows both columns and playback is active
- **THEN** the playback panel SHALL render inside the queue column and the Library playback panel
  SHALL NOT render

#### Scenario: Narrow terminal stacks vertically

- **WHEN** the layout is queue-only at a width below 100 columns and playback is active
- **THEN** the visual slot SHALL render at full column width, the panel directly below it, and the
  queue list below the panel

#### Scenario: Wide terminal renders two columns

- **WHEN** the layout is queue-only at a width of 100 columns or more and playback is active
- **THEN** the visual slot SHALL render left-aligned in the left column and the panel in the right
  column, separated by a 2-cell gap

#### Scenario: Panel height and width follow the visual slot

- **WHEN** the wide two-column layout is active
- **THEN** the panel's height SHALL equal the visual slot's rendered height and its width SHALL be the
  total column width minus the visual slot's width minus the 2-cell gap

### Requirement: Idle collapse in queue-visible layouts

When the queue column is visible and no transport is active, the visual slot (artwork, placeholder,
loading reservation, or empty visualizer box) and the playback panel SHALL NOT be rendered and SHALL
reserve zero rows; the queue panel SHALL occupy those rows and begin directly below the header row,
its recessed top inset being the single space row between the header and the panel. The separator
row between the slot/transport band and the panel exists only while that band renders. Paused
playback counts as active and SHALL keep both. The artwork/visualizer
selection SHALL persist while idle and take effect on the next playback; pressing the artwork key
while idle SHALL NOT create a visual slot rectangle. A connected transport that is not playing SHALL
NOT keep the panel: the header row's `IDLE` wording and target are the only idle-state indicator.

#### Scenario: Idle queue-visible layout reclaims the slot and panel rows

- **WHEN** the queue column is visible and no transport is active
- **THEN** no visual slot and no playback panel SHALL be rendered, and the queue panel SHALL begin at
  the position the header row leaves open

#### Scenario: Idle collapse holds at every supported width

- **WHEN** the queue column is visible with no transport active, at a width below 100 columns and at a
  width of 100 columns or more
- **THEN** neither the visual slot nor the panel SHALL render in the two-panel route, the stored
  queue-only route, or the narrow mini-view queue route

#### Scenario: Playback start restores both

- **WHEN** playback starts from an idle queue-visible layout
- **THEN** the visual slot and the panel SHALL render again and the queue panel SHALL move below them

#### Scenario: Paused playback keeps both

- **WHEN** the queue column is visible and playback is paused
- **THEN** the visual slot and the panel SHALL remain rendered

#### Scenario: Connected transport that is playing keeps the panel

- **WHEN** the queue column is visible and the connected remote Session reports a now-playing
  item, whether or not the local queue holds it
- **THEN** the visual slot and the panel SHALL render, the slot SHALL show the artwork of the
  item the Session names, the panel's title and `pos / dur` SHALL describe the Session's observed
  playback, and its transport SHALL dispatch the Session's supported remote commands

#### Scenario: Connected but idle does not keep the panel

- **WHEN** the queue column is visible, a remote session or cast target is connected, and nothing is
  playing on it
- **THEN** the visual slot and the panel SHALL NOT render, and the header row SHALL read `IDLE` with
  the connected target

### Requirement: The Library playback panel renders exactly when the queue column is hidden

The Library playback panel SHALL render only in layouts where the queue column is hidden
(library-only, wide or mini view). In every layout where the queue column is visible the strip SHALL
NOT render, and the right column's content area SHALL NOT reserve the strip's rows, so the library
takes them.

#### Scenario: Library-only paints the strip

- **WHEN** the layout is library-only at a width of 80 columns or more, or the library panel is shown
  in narrow mini view
- **THEN** the Library playback panel SHALL render below the Tab panel and the library content SHALL start below
  it

#### Scenario: Two-panel layout reclaims the strip rows

- **WHEN** the layout shows both columns
- **THEN** the Library playback panel SHALL NOT render and the library content SHALL occupy its rows
  would have used

#### Scenario: Queue-only has no right column

- **WHEN** the layout is queue-only
- **THEN** no strip SHALL render, and any transport SHALL be reachable through the queue column panel

### Requirement: Playback controls respond to pointer input wherever the panel is painted

The playback panel's transport glyphs and seekbar SHALL resolve pointer input in every layout where
the panel renders, using the geometry of the panel that was actually painted in the current frame:
the queue column panel in queue-visible layouts and the right-column strip otherwise. A panel that is
not painted SHALL resolve nothing.

#### Scenario: Sidebar panel accepts clicks in queue-only

- **WHEN** the layout is queue-only and the playback panel is painted
- **THEN** a click on its play/pause glyph SHALL toggle playback, and a click on its seekbar SHALL
  seek to the corresponding fraction

#### Scenario: Sidebar panel accepts clicks in the two-panel layout

- **WHEN** the layout shows both columns and the playback panel is painted in the queue column
- **THEN** a click on its transport glyphs SHALL emit the corresponding transport intent

#### Scenario: Strip accepts clicks in library-only

- **WHEN** the layout is library-only
- **THEN** clicks on the strip's glyphs and seekbar SHALL resolve against the strip's painted geometry

#### Scenario: Collapsed panel resolves nothing

- **WHEN** the panel is not painted because playback is idle
- **THEN** a click in the rows it would have occupied SHALL NOT emit a playback intent

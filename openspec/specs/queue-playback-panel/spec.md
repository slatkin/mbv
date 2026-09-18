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
and aqua when remote. The header SHALL state the status and the target only: no throbber, no
progress percent, and no time SHALL paint in it.

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
- **AND** no throbber, progress percent, or time SHALL paint in the header row

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

### Requirement: The Library playback panel's title row shows elapsed time and the status pill

The Library playback panel's title row SHALL paint the transport controls, the title, the elapsed
position, and the status indicator pill, in that order left to right, and no progress cluster: no
throbber glyph and no progress percent SHALL appear in the strip. The position SHALL show the
elapsed time alone, never a `pos / dur` total. The status pill SHALL carry one cell of padding on
each side, and the strip SHALL NOT paint a second pad beside it. The transport controls SHALL
appear only when the glyph, the controls, the elapsed time, the pill and the title all fit.

#### Scenario: Active playback in the strip

- **WHEN** the Library playback panel paints an active item with a known runtime
- **THEN** the title row SHALL show the transport controls, the title, the elapsed position, and the
  padded status pill
- **AND** no total duration, no percent, and no throbber glyph SHALL appear in the row

#### Scenario: The strip's pill is padded once per side

- **WHEN** the Library playback panel paints a status pill
- **THEN** the pill SHALL open and close with exactly one padding cell each side

#### Scenario: Narrow strip drops the transport controls

- **WHEN** the strip is too narrow for the transport controls, the elapsed time, the pill and the
  title together
- **THEN** the strip SHALL drop the controls and keep the title, the elapsed time and the pill

### Requirement: The queue panel opens directly with its rows

The recessed queue panel inside the queue column SHALL NOT paint a title band: no `Queue` title, no
block separator line, and no spacer row above the first queue row. The panel's content SHALL keep
only the recessed box's own one-row top inset and its bottom padding row, so the first queue row
starts one inset row below the box's top edge.

#### Scenario: The panel has no title band

- **WHEN** a queue-visible layout paints the recessed queue panel
- **THEN** no `Queue` title and no block separator line are painted above the rows
- **AND** the first queue row starts one inset row below the box's top edge

### Requirement: The playback panel renders in the queue column

In every queue-visible layout the playback panel (seekbar, title row, controls) SHALL render inside
the queue column, using the same content as the Library playback panel. The panel SHALL NOT
render in the right column of a queue-visible layout. The queue column splits the title band
across two rows: the upper row keeps the transport controls and the status pills, while the
title and the `pos / dur` time render one row below — the title left with one
space of indent, the time right with one space of indent, with the title's marquee window kept.
When the now-playing title carries a context part (e.g. a show beside an episode title), the
band SHALL expand onto a third row: the show and the `pos / dur` time render on the middle row
(the show left with one space of indent, the time right with one space of indent), and the title
alone renders on the row below with the marquee window kept; the transport's footprint SHALL
grow by that one row for as long as a context part plays. The show clips to its row without
scrolling; the marquee belongs to the title row.

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

### Requirement: The playback panel title row names the media type

The transport title row of the Queue playback panel and of the Library playback panel — the same
content both panels render — SHALL name what is playing, not just the item's own title. The row
SHALL be composed of up to two parts: the item's **title part**, and the **context part** naming
the container the item came from, when the item has one.

| Media | Title part | Context part |
|---|---|---|
| Emby movie | item name | — |
| Emby episode | episode name | series name |
| Emby audio track | track name | artist |
| Emby home video | item name | — |
| Audiobookshelf podcast episode | episode title | show (podcast) title |
| Audiobookshelf book | book title | — |
| Feed entry | entry title | feed subscription name |

The row SHALL paint the context part first, then the title part: an episode renders as the series
name followed by the episode name, an audio track as the artist followed by the track name, a feed
entry as the subscription name followed by the entry title.

The context part SHALL be omitted — leaving a single-part title — when the media type has no
container, and when the container name is absent from the item or cannot be resolved. In
particular an Audiobookshelf podcast episode queued outside its show, and a feed entry whose
subscription no longer resolves, SHALL render the title part alone.

While the panel is showing a connected remote Session or cast target whose now-playing item the
viewed queue does not hold, the row SHALL continue to render that target's own title.

#### Scenario: Emby episode shows series and episode

- **WHEN** the panel renders while an Emby episode is playing
- **THEN** the title row SHALL show the series name first and the episode name after it
- **AND** neither part SHALL be dropped

#### Scenario: Emby movie shows the name alone

- **WHEN** the panel renders while an Emby movie is playing
- **THEN** the title row SHALL show the movie name and no context part

#### Scenario: Emby audio track shows artist and track

- **WHEN** the panel renders while an Emby audio track is playing
- **THEN** the title row SHALL show the artist and the track name

#### Scenario: Emby home video shows the name alone

- **WHEN** the panel renders while an Emby home video is playing
- **THEN** the title row SHALL show the item name and no context part

#### Scenario: Audiobookshelf podcast episode shows the podcast

- **WHEN** the panel renders while an Audiobookshelf podcast episode is playing and its show
  title is known
- **THEN** the title row SHALL show the episode title and the podcast title

#### Scenario: Audiobookshelf podcast episode without a known show

- **WHEN** the panel renders while an Audiobookshelf podcast episode is playing and no show title
  is carried
- **THEN** the title row SHALL show the episode title alone

#### Scenario: Audiobookshelf book shows the book title alone

- **WHEN** the panel renders while an Audiobookshelf book is playing
- **THEN** the title row SHALL show the book title and no context part

#### Scenario: Feed entry shows the subscription

- **WHEN** the panel renders while a feed entry is playing and its subscription resolves
- **THEN** the title row SHALL show the entry title and the subscription's display name

#### Scenario: Feed entry whose subscription no longer resolves

- **WHEN** the panel renders while a feed entry is playing and no subscription matches the entry
- **THEN** the title row SHALL show the entry title alone

#### Scenario: The same content renders in whichever panel is painted

- **WHEN** the layout changes between a queue-visible layout and a library-only layout while the
  same item plays
- **THEN** the two panels SHALL render the same title parts for that item, including the
  queue column's split title band

### Requirement: Title and context parts are delineated by colour

When a title row carries both parts, the parts SHALL be separated by exactly one space and SHALL
NOT be joined by a separator glyph — no hyphen, dash, pipe, bullet or any other delimiter. The
distinction between the two parts SHALL be carried by colour alone: the title part SHALL paint in
the playback panel's aqua title role and the context part in its yellow context role. A
single-part title SHALL paint wholly in the title role.

These roles SHALL be the playback panel's own semantic roles, not the shared focus-accent or
brand roles, so that changing the focus accent or a brand colour cannot move the now-playing
title. The roles SHALL apply wherever the row paints, including while the title is marqueed for
overflow and in the queue column's split title band.

Because the parts are distinguished by colour rather than by a delimiter, the row's plain-text
forms used outside the panel — the strings carried by toasts, the media-progress interface and
log lines — SHALL NOT change, and SHALL keep their existing separator form.

#### Scenario: Both parts paint in their own roles

- **WHEN** the title row renders an item with both a title and a context part
- **THEN** the title part paints in the aqua title role and the context part in the yellow context
  role

#### Scenario: No delimiter glyph between the parts

- **WHEN** the title row renders an item with both parts
- **THEN** the characters between the two parts SHALL be a single space and nothing else

#### Scenario: A single-part title paints in the title role

- **WHEN** the title row renders an item with no context part
- **THEN** the whole title paints in the aqua title role

#### Scenario: The roles survive the overflow marquee

- **WHEN** a two-part title is wider than its slot and the row marquees it
- **THEN** the visible window of the title part stays in the aqua title role and of the context
  part in the yellow context role

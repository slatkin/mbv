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
the Queue playback panel's transport (controls, seekbar and their times), not its header row.

#### Scenario: Idle queue-visible layout keeps only the header
- **WHEN** the queue column is visible and no transport is active
- **THEN** the Queue playback panel paints its header row and nothing else
- **AND** the Queue panel begins directly below the header row, with no blank row between them

#### Scenario: Two-panel layout with active playback
- **WHEN** both columns are visible and playback is active
- **THEN** the transport renders in the Queue playback panel and the Library playback panel does not
  render

#### Scenario: Library-only layout with active playback
- **WHEN** the layout is library-only and playback is active
- **THEN** the Library playback panel renders the transport and no Queue playback panel renders

### Requirement: Queue-visible layouts paint a now-playing header row

In every layout that renders the queue column, the left column SHALL paint one header row at the top
of its content, above the visual slot. The header SHALL always be painted, including
while playback is idle.

The header SHALL follow the effective playback target — cast, then the owner's Emby Session
output, then local — and SHALL NOT follow the queue scope being viewed.

#### Scenario: Emby Session output playback of content the Owner did not load paints no playhead

- **WHEN** a queue-visible layout is painted while the owner's output is an Emby Session whose
  now-playing item the viewed queue does not hold (another device's own selection)
- **THEN** no queue row SHALL be painted as the playhead

#### Scenario: Header follows the playback target, not the viewed queue

- **WHEN** the layout is queue-visible while the owner's output is an Emby Session or Direct
  remote control targets a remote owner, and the local queue scope is selected for viewing
- **THEN** the header SHALL describe the playback target's playback, not the local queue's

#### Scenario: No header when the queue column is hidden

- **WHEN** the layout is library-only
- **THEN** no header row SHALL be painted

### Requirement: The playback panel renders in the queue column

In every queue-visible layout the playback panel (controls, seekbar and their times) SHALL render
inside the queue column, using the same content as the Library playback panel. The panel SHALL NOT
render in the right column of a queue-visible layout.

When the terminal is narrower than 100 columns the visual slot and the playback panel SHALL stack
vertically: the visual slot at full column width, the panel directly below it, and the queue list
directly below the panel. When the terminal is 100 columns or more the visual slot and the panel SHALL render
side by side in the same row, the visual slot left-aligned in the left column and the panel in the
right, separated by the existing 2-cell gap; the panel's width SHALL be the remaining column width
and its height SHALL equal the visual slot's height (never fewer than the band's four rows), with its
content top-aligned and the panel background filling the rows below it.

#### Scenario: Panel renders in the queue column in the two-panel layout

- **WHEN** the layout shows both columns and playback is active
- **THEN** the playback panel SHALL render inside the queue column and the Library playback panel
  SHALL NOT render

#### Scenario: Narrow terminal stacks vertically

- **WHEN** the layout is queue-only at a width below 100 columns and playback is active
- **THEN** the visual slot SHALL render at full column width, the panel directly below it, and the
  queue list directly below the panel

#### Scenario: Wide terminal renders two columns

- **WHEN** the layout is queue-only at a width of 100 columns or more and playback is active
- **THEN** the visual slot SHALL render left-aligned in the left column and the panel in the right
  column, separated by a 2-cell gap

#### Scenario: Panel height and width follow the visual slot

- **WHEN** the wide two-column layout is active
- **THEN** the panel's height SHALL equal the visual slot's rendered height (at least four rows) and
  its width SHALL be the total column width minus the visual slot's width minus the 2-cell gap

### Requirement: Idle collapse in queue-visible layouts

When the queue column is visible and no transport is active, the visual slot (artwork, placeholder,
loading reservation, or empty visualizer box) and the playback panel SHALL NOT be rendered and SHALL
reserve zero rows; the queue panel SHALL occupy those rows and begin below the header row.
Paused playback counts as active and SHALL keep both. The artwork/visualizer
selection SHALL persist while idle and take effect on the next playback; pressing the artwork key
while idle SHALL NOT create a visual slot rectangle. A connected transport that is not playing SHALL
NOT keep the panel.

#### Scenario: Idle queue-visible layout reclaims the slot and panel rows

- **WHEN** the queue column is visible and no transport is active
- **THEN** no visual slot and no playback panel SHALL be rendered, and the queue panel SHALL begin
  on the row below the header row

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
  item the Session names, the header row's title and the seekbar row's times SHALL describe the
  Session's observed playback, and its transport SHALL dispatch the Session's supported remote
  commands

#### Scenario: Connected but idle does not keep the panel

- **WHEN** the queue column is visible, a remote session or cast target is connected, and nothing is
  playing on it
- **THEN** the visual slot and the panel SHALL NOT render

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
the queue column panel in queue-visible layouts and the right-column strip otherwise. In the queue
column's band the transport glyphs resolve on the controls row and the seekbar on the seekbar row's
bar span; the blank title row, the blank gap row, and the time labels resolve nothing. Every
transport glyph it paints, the previous control included, SHALL retain the geometry of its own glyph,
so a click resolves that control's intent and no two controls share one region. A transport control
whose action is unavailable SHALL NOT resolve an intent. A
panel that is not painted SHALL resolve nothing.

A transport glyph SHALL resolve on a single press. The seekbar SHALL resolve a seek only on a
recognized double-click: two presses inside the double-click interval at the same cell. A single
press on the bar SHALL resolve neither a seek nor a transport intent. A left-button drag that
anchored inside the bar span SHALL scrub — each admitted motion resolves the seek fraction of the
pointer's current column — and the panel SHALL admit at most one scrub seek per 150 ms. A drag that
anchored outside the bar span, and a motion whose pointer is outside it, SHALL resolve no seek.

#### Scenario: Sidebar panel accepts clicks in queue-only

- **WHEN** the layout is queue-only and the playback panel is painted
- **THEN** a click on its play/pause glyph SHALL toggle playback, and a double-click on its seekbar
  SHALL seek to the corresponding fraction

#### Scenario: Sidebar panel accepts clicks in the two-panel layout

- **WHEN** the layout shows both columns and the playback panel is painted in the queue column
- **THEN** a click on its transport glyphs SHALL emit the corresponding transport intent

#### Scenario: Queue band resolves only its glyphs and bar

- **WHEN** the queue column's band is painted
- **THEN** a click on a glyph in the controls row SHALL emit that control's intent and a
  double-click on the seekbar row's bar SHALL seek
- **AND** a single press on the seekbar row's bar SHALL emit no playback intent
- **AND** a click on the blank title row, the blank gap row, or either time label SHALL NOT emit a
  playback intent

#### Scenario: Strip accepts clicks in library-only

- **WHEN** the layout is library-only
- **THEN** clicks on the strip's glyphs SHALL resolve against the strip's painted geometry, and a
  double-click on its seekbar SHALL seek

#### Scenario: A drag on the bar scrubs at most once per interval

- **WHEN** the panel is painted and the user presses the left button inside the seekbar's bar span
  and moves the pointer along it
- **THEN** each admitted motion SHALL seek to the fraction of the pointer's current column
- **AND** no two admitted scrubs SHALL be less than 150 ms apart

#### Scenario: A drag that anchored outside the bar does not scrub

- **WHEN** the user drags the pointer across the seekbar's bar span from a press that landed outside
  it
- **THEN** the panel SHALL emit no seek intent

#### Scenario: Previous control accepts clicks

- **WHEN** the panel is painted with the previous control available
- **THEN** a click on the previous glyph SHALL emit the previous transport intent
- **AND** its retained region SHALL NOT overlap the next control's

#### Scenario: Collapsed panel resolves nothing

- **WHEN** the panel is not painted because playback is idle
- **THEN** a click in the rows it would have occupied SHALL NOT emit a playback intent

### Requirement: The playback panel title row names the media type

The now-playing title of the Queue playback panel (carried on its header row) and the transport title
row of the Library playback panel — the same content both panels render — SHALL name what is playing,
not just the item's own title. "The title row" in the scenarios below means whichever of the two
carries the title. The title SHALL be composed of up to two parts: the item's **title part**, and the
**context part** naming the container the item came from, when the item has one.

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
- **THEN** the two panels SHALL render the same title parts for that item, the queue column on its
  header row and the Library strip on its title row

### Requirement: The user can hide the visual slot

The user SHALL be able to hide and show the QueueColumn's visual slot (artwork, placeholder, loading
reservation, or visualizer) with the `hide_visual_slot` keyboard action, whose default chord is `h`.
The same hide action SHALL be reachable from the visual slot's context menu. While hidden, the visual
slot SHALL NOT be rendered and SHALL reserve zero rows and zero columns; the
header row and the playback panel's transport SHALL remain rendered whenever they would be without
this action. Below 100 columns the transport SHALL begin on the row directly below the header row.
At 100 columns or more the transport SHALL span the full width of the slot region, and the band
above the Queue panel SHALL be the transport's own height (its four rows). The Queue panel SHALL take
every row the slot released. The action SHALL toggle the hidden state in every playback state (idle
included) and every panel mode; the state SHALL take effect wherever the QueueColumn is rendered. The
hidden state SHALL persist across launches; a first launch with no saved state SHALL show the slot.
Idle collapse is unchanged: while idle the slot reserves zero rows whether or not it is hidden.

#### Scenario: Hiding the slot below 100 columns

- **WHEN** playback is active, the QueueColumn is narrower than 100 columns, and the user presses `h`
- **THEN** the visual slot SHALL NOT render, the transport SHALL begin directly below the header row,
  and the Queue panel SHALL begin directly below the transport's four rows

#### Scenario: Hiding the slot at 100 columns or more

- **WHEN** playback is active, the QueueColumn is 100 columns or wider, and the user presses `h`
- **THEN** the visual slot SHALL NOT render, the transport SHALL span the slot region's full width,
  and the Queue panel SHALL begin directly below the transport's own four rows

#### Scenario: Showing the slot again

- **WHEN** the visual slot is hidden and the user presses `h`
- **THEN** the visual slot SHALL render again with the currently selected content (artwork or
  visualizer) and the Queue panel SHALL move below it

#### Scenario: Hidden state survives a restart

- **WHEN** the user hides the visual slot and later relaunches mbv
- **THEN** the visual slot SHALL be hidden at launch

#### Scenario: Toggling while idle

- **WHEN** no transport is active and the user presses `h`
- **THEN** the layout SHALL NOT change, and the toggled hidden state SHALL apply when playback
  next starts

#### Scenario: Text entry keeps `h`

- **WHEN** a text-entry surface owns focus and the user types `h`
- **THEN** the character SHALL reach the text field and the hidden state SHALL NOT change

#### Scenario: A focused library screen does not intercept `h`

- **WHEN** a library screen that binds `h` locally has focus and the user presses `h`
- **THEN** the visual slot's hidden state SHALL toggle and the screen's local `h` action SHALL NOT run

#### Scenario: Hiding from the visual slot menu

- **WHEN** the visual slot shows and the user chooses the slot menu's hide entry
- **THEN** the slot SHALL hide with the same persisted hidden state and the same layout change the
  `h` action produces

### Requirement: Seekbar progress stays bounded and current

A non-positive runtime SHALL show an empty bar. With a positive runtime, a non-positive position SHALL show an empty bar and a position at or beyond runtime SHALL show a full bar. The bar SHALL recompute from the current projected position and current width on each paint, without extrapolating playback. A repaint with an unchanged paused position, runtime, and width SHALL leave the bar unchanged.

#### Scenario: Invalid or out-of-range progress stays bounded

- **WHEN** the Queue seekbar paints a non-positive runtime, or a non-positive position with
a positive runtime
- **THEN** its bar SHALL show empty
- **WHEN** its position is at or beyond a positive runtime
- **THEN** its bar SHALL show full

#### Scenario: Backward seeking and resizing use the current playback snapshot

- **WHEN** the active Queue seekbar repaints after a backward seek or a width change
- **THEN** its fill SHALL reflect the current projected position and current bar width

#### Scenario: Paused playback does not change the bar

- **WHEN** the Queue seekbar repaints with the same paused position, runtime, and width
- **THEN** its fill SHALL remain unchanged

### Requirement: Fractional Queue fill preserves the seek row and interaction boundaries

The bar span alone SHALL remain the pointer target. A partial cell SHALL be part of that
same target and SHALL resolve the same terminal-column-based seek fraction as before;
fractional painting SHALL NOT imply sub-cell pointer precision. The time labels, padding,
and blank transport rows SHALL continue to resolve no seek intent. When no bar width
remains, the existing clipped time-only fallback SHALL remain and SHALL expose no seek
region. No border, label, or extra row SHALL be added to the bar.

#### Scenario: A partially filled cell is still an ordinary seek target

- **WHEN** the user double-clicks a partially filled cell of the Queue seekbar
- **THEN** the seek SHALL resolve the same fraction that column resolved before
- **AND** a single press on the same cell SHALL NOT seek
- **AND** clicks on either flanking time label SHALL NOT seek

#### Scenario: A narrow seek row has no room for a bar

- **WHEN** the available Queue seek row leaves zero cells for the bar after its labels and
  padding are accounted for
- **THEN** the row SHALL retain its existing clipped time-only presentation
- **AND** it SHALL expose no seek region

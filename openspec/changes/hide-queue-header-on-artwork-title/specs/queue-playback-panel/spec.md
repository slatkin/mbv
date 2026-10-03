# Spec Delta

## MODIFIED Requirements

### Requirement: Queue and Library playback panels are distinct, and exactly one transport renders

The Queue playback panel (header row, visual slot and transport in the queue column) and the Library
playback panel (the right-column strip) SHALL be two distinct panels, each with its own presentation of
the same playback content. In every frame at most one of them SHALL render a transport: the Queue
playback panel's in queue-visible layouts, the Library playback panel's when the queue column is hidden.

The Queue playback panel SHALL be present in every queue-visible layout, including while idle. While
idle it renders only the header row; its visual slot and transport reserve zero rows. While playback is
active and the artwork carries the title (the `queue-artwork-title-overlay` capability's artwork site),
it renders the visual slot and transport with no header row: zero header rows are reserved. Throughout
this capability and the `panel-mode` and `idle-feed-rotation`
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

#### Scenario: Artwork-title playback renders no header row
- **WHEN** the queue column is visible and the artwork carries the now-playing title
- **THEN** the Queue playback panel paints its visual slot and transport and no header row

### Requirement: Queue-visible layouts paint a now-playing header row

In every layout that renders the queue column, the left column SHALL paint one header row at the top
of its content, above the visual slot, on the chrome surface — but only while playback is idle or
while the header carries the now-playing title (the fallback states the
`queue-artwork-title-overlay` capability defines). While playback is active and the artwork carries
the title, no header row SHALL be painted and the playback region SHALL reserve zero header rows:
the visual slot starts at the queue column's top content row, and the Queue panel takes the rows
the header band would have spent. The header SHALL NOT reappear transiently while an expected
overlay composes: on a setup whose artwork can carry the title, the header stays absent across
track changes while playing, and returns only when playback goes idle or a fallback state moves
the title back to the header.

When painted, the header SHALL sit in the queue column's recessed inset: one row of column
surface above it and two columns of column surface on each side, with no padding below it, so it is
not flush with the column's top, left, or right edge and its text aligns with the slot and
transport below.

While playback is idle the header SHALL state `IDLE` on the left and the playback target as
`[host]` on the right, with the brackets cream, the hostname muted when local and aqua when remote,
and the hostname truncated with an ellipsis (brackets kept) before it can overlap `IDLE`. The target
SHALL be the playback-target label the shell resolves: this machine's device name when playback is
local, and the connected session's device name (then its host) or the direct-remote label when it is
remote.

While a target plays and the header carries the title, the header row SHALL NOT carry a status word
(`PLAYING` or `PAUSED`) and SHALL NOT carry the target (`on <host>` or `[host]`): it SHALL begin
with the state icon, one space, and then either the now-playing title or the label `Now Playing`.
The state icon SHALL be the play icon painted aqua while playback is not paused, and the pause icon
painted yellow (the focus-accent role) while it is paused. The row SHALL carry the title — a
two-part title as the context part left-aligned (clipped to its room, never scrolling) and the
title part right-aligned with the overflow marquee on the remaining width, a one-part title as a
single yellow title with the marquee. No throbber, progress percent, or time SHALL paint in the
header.

The header SHALL follow the effective playback target — cast, then connected session, then local —
and SHALL NOT follow the queue scope being viewed.

#### Scenario: Header is recessed in the queue column

- **WHEN** a queue-visible layout is painted with a header row
- **THEN** the header SHALL sit one row below the column's top edge, inset two columns from each side
  edge, with no blank row between it and the slot/transport band below

#### Scenario: Now Playing label while the artwork carries the title
- **WHEN** a queue-visible layout is painted while the artwork carries the title
- **THEN** no header row SHALL be painted, no `Now Playing` label SHALL appear, and zero header
  rows SHALL be reserved
- **AND** the visual slot SHALL start at the queue column's top content row
- **AND** the Queue panel SHALL occupy the rows the header band would have spent

#### Scenario: Track change does not re-introduce the header
- **WHEN** the artwork carries the title and the playing track changes
- **THEN** no header row SHALL appear while the new track's overlay composes
- **AND** the title SHALL be painted on neither site until the new overlay lands

#### Scenario: Header returns in a fallback state
- **WHEN** the artwork carries the title and a fallback state moves the title back to the header
  (the visualizer replaces the artwork, images are disabled, the visual slot is hidden, or the
  terminal protocol cannot carry the overlay)
- **THEN** the header row SHALL paint with the title in its recessed inset

#### Scenario: Active local playback

- **WHEN** a queue-visible layout is painted while a target plays and the artwork does not carry
  the title
- **THEN** the header row SHALL read the state icon followed by the now-playing title
- **AND** the header row SHALL NOT contain `PLAYING`, `PAUSED`, `on <host>`, or `[host]`
- **AND** no throbber, progress percent, or time SHALL paint in the header row

#### Scenario: Play icon is aqua

- **WHEN** a queue-visible layout is painted while playback is active and not paused, and the header
  carries the title
- **THEN** the header row SHALL show the play icon painted aqua

#### Scenario: Two-part title alignment

- **WHEN** a queue-visible layout is painted while the now-playing title has a context part and the
  artwork does not carry the title
- **THEN** the context part SHALL be left-aligned beside the state icon and the title part
  right-aligned, with the marquee window on the width the context part leaves

#### Scenario: Paused playback

- **WHEN** a queue-visible layout is painted while playback is paused and the header carries the
  title
- **THEN** the header row SHALL show the pause icon painted yellow in place of the aqua play icon
- **AND** the header row SHALL NOT contain `PAUSED`

#### Scenario: Idle

- **WHEN** a queue-visible layout is painted while no transport is active
- **THEN** the header row SHALL read `IDLE` on the left and the resolved target as `[host]` on the
  right

#### Scenario: Active watched remote playback

- **WHEN** a queue-visible layout is painted while attached to a Session that reports a
  now-playing item the viewed queue does not hold (another device's own selection), and the header
  carries the title
- **THEN** the header row SHALL describe that Session's item as above, without a status word or target
- **AND** no queue row SHALL be painted as the playhead

#### Scenario: Header follows the playback target, not the viewed queue

- **WHEN** the layout is queue-visible while attached to a remote session and the local queue scope is
  selected for viewing
- **THEN** the header SHALL describe the remote target's playback, not the local queue's

#### Scenario: No header when the queue column is hidden

- **WHEN** the layout is library-only
- **THEN** no header row SHALL be painted


### Requirement: The playback panel renders in the queue column

In every queue-visible layout the playback panel (controls, seekbar and their times) SHALL render
inside the queue column, using the same content as the Library playback panel. The panel SHALL NOT
render in the right column of a queue-visible layout. The queue column's transport is a fixed
four-row band, top to bottom: the controls row, a blank title row, the seekbar row, and a blank gap
row. The band SHALL be four rows whether or not the now-playing title carries a context part; the
title SHALL NOT paint in the band (it rides the header row while one is painted, see the header
requirement; otherwise the artwork carries it, or it is temporarily unshown) and no
`pos / dur` time SHALL paint on a title row.

The controls row SHALL paint the transport controls on the left and the status indicators on the
right as plain text (no pill), on the slate backdrop fill edge to edge, one column of indent each
side. The stop, previous, and next buttons SHALL paint only when they fit beside the play/pause glyph
and the indicators; otherwise only the play/pause glyph paints. The seekbar row SHALL paint the
elapsed time, the bar, and the total time, in that order left to right, one space between each time
and the bar and one column of outer indent each side; a bar too narrow to paint SHALL leave only the
two times. The title row and the gap row SHALL paint blank on the panel fill. The transport hit
geometry rides the controls row (the glyphs) and the seekbar row (the bar span alone; the time labels
never seek).

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

#### Scenario: The band is four rows regardless of the title

- **WHEN** the queue column paints the transport for a one-part title and for a two-part title
- **THEN** both bands SHALL span four rows: controls, blank title row, seekbar, blank gap row
- **AND** neither band SHALL show the title or a `pos / dur` time on its title row

#### Scenario: Controls row carries the controls and plain-text indicators

- **WHEN** the queue column paints the transport with status indicators projected
- **THEN** the top row SHALL show the transport controls left and the indicators right as plain text
  with no pill

#### Scenario: Seekbar row flanks the bar with its times

- **WHEN** the queue column paints the transport with a known runtime
- **THEN** the seekbar row SHALL read the elapsed time, the bar, then the total time

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
reserve zero rows; the queue panel SHALL occupy those rows and begin directly below the header row,
with no blank row between them. While the band renders, the queue panel begins directly below the
band's blank gap row; no separator row exists in either state. Paused
playback counts as active and SHALL keep both. The artwork/visualizer
selection SHALL persist while idle and take effect on the next playback; pressing the artwork key
while idle SHALL NOT create a visual slot rectangle. A connected transport that is not playing SHALL
NOT keep the panel: the header row's `IDLE` wording and target are the only idle-state indicator.

#### Scenario: Idle queue-visible layout reclaims the slot and panel rows

- **WHEN** the queue column is visible and no transport is active
- **THEN** no visual slot and no playback panel SHALL be rendered, and the queue panel SHALL begin on
  the row directly below the header row

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
  item the Session names, the carried title (the header row's while one is painted, otherwise the
  artwork's) and the seekbar row's times SHALL describe the
  Session's observed playback, and its transport SHALL dispatch the Session's supported remote
  commands

#### Scenario: Connected but idle does not keep the panel

- **WHEN** the queue column is visible, a remote session or cast target is connected, and nothing is
  playing on it
- **THEN** the visual slot and the panel SHALL NOT render, and the header row SHALL read `IDLE` with
  the connected target

### Requirement: The playback panel title row names the media type

The now-playing title of the Queue playback panel (carried on its header row while one is painted;
on the artwork otherwise, per the header requirement) and the transport title
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
- **THEN** the two panels SHALL render the same title parts for that item, the queue column on
  whichever site carries it (its header row or the artwork) and the Library strip on its title row

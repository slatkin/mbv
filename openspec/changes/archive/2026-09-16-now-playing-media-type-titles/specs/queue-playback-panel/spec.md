## ADDED Requirements

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

The context part SHALL be omitted — leaving a single-part title — when the media type has no
container, and when the container name is absent from the item or cannot be resolved. In
particular an Audiobookshelf podcast episode queued outside its show, and a feed entry whose
subscription no longer resolves, SHALL render the title part alone.

While the panel is showing a connected remote Session or cast target whose now-playing item the
viewed queue does not hold, the row SHALL continue to render that target's own title.

#### Scenario: Emby episode shows series and episode

- **WHEN** the panel renders while an Emby episode is playing
- **THEN** the title row SHALL show the episode name and the series name
- **AND** neither part SHALL be dropped or reordered

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
  queue column's split lower title row

### Requirement: Title and context parts are delineated by colour

When a title row carries both parts, the parts SHALL be separated by exactly one space and SHALL
NOT be joined by a separator glyph — no hyphen, dash, pipe, bullet or any other delimiter. The
distinction between the two parts SHALL be carried by colour alone: the title part SHALL paint in
the playback panel's aqua title role and the context part in its yellow context role. A
single-part title SHALL paint wholly in the title role.

These roles SHALL be the playback panel's own semantic roles, not the shared focus-accent or
brand roles, so that changing the focus accent or a brand colour cannot move the now-playing
title. The roles SHALL apply wherever the row paints, including while the title is marqueed for
overflow and in the queue column's split lower row.

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

## MODIFIED Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, an optional secondary title (the container name of a split row, painted after the primary text), an optional metadata slot whose text role and placement are closed — a release year or a publish date both paint right-aligned in the fixed six-column date gutter in the green (`STATUS_AVAILABLE`) role, a progress badge in the FOAM metadata role — a media kind (`Collection` for navigable containers, `Media` for playable leaves), an optional duration string (painted right-aligned in its own (`DURATION`) role, distinct from the green gutter role), and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. Only the Queue list SHALL project a duration string; a library browse list row (Home, Podcast, TV, music, book, feeds) SHALL NOT carry one. A Heading SHALL paint its label bold in the FOAM metadata role, so every grouped list's group label reads the same. Heading and Spacer SHALL be excluded from selectable-target indexing. When a duration is shown it SHALL use the precise `M:SS`/`H:MM:SS` form (queue format, e.g. `4:32`, `1:02:03`); `Collection` rows SHALL NOT carry a duration. `Active` SHALL keep its existing meaning of stored resume progress rendered inline as trailing metadata; `NowPlaying` SHALL mean live playback, rendering its live progress inline as trailing metadata and its total duration like every other row — no throbber glyph appears in any row. On Wide lists, `NowPlaying` rows SHALL be marked by an aqua right-pointing play glyph before the title (one space from it) rather than an accent-coloured title; their title text SHALL keep the ordinary colour. A `NowPlaying` row SHALL NOT carry a secondary title, so the split-row palette below never applies to one. `Active` rows SHALL not be marked by the play glyph. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

A row carrying a secondary title SHALL paint as a split row: the primary text (the container/context name) SHALL paint in the split-row context (`SPLIT_ROW_CONTEXT_FG`) role and the secondary title (the item's own name) SHALL paint in the split-row title (`SPLIT_ROW_TITLE_FG`) role, with one separating space. A row with no secondary title SHALL keep the ordinary title role for its primary text. A played split row SHALL mute the item title to the played (`TEXT_MUTED`) role while the context part keeps the split-row context role. No other semantic state SHALL move the split-row palette, and a `NowPlaying` row SHALL NOT carry a secondary title, so the palette never overrides the now-playing row's ordinary-colour title. A split row truncates as one string: its parts SHALL be cut as a unit with at most one trailing ellipsis, the context part keeping its full width and the item title absorbing the cut.

#### Scenario: Queue-like progress is presented safely
- **WHEN** a parent supplies active progress
- **THEN** the control receives only a bounded percentage
- **AND** playback and queue authority remain with the parent/shell

#### Scenario: Now-playing renders like other rows
- **WHEN** a row carries the now-playing state
- **THEN** an aqua play glyph is painted one space before the title, and the title keeps the ordinary colour
- **AND** live progress renders inline next to the title exactly as resume progress does
- **AND** the duration slot shows the total duration
- **AND** no throbber glyph appears anywhere in the row

#### Scenario: Now-playing with unknown runtime
- **WHEN** a now-playing row carries no progress
- **THEN** no percentage renders and the duration slot stays empty

#### Scenario: Resume progress stays inline
- **WHEN** a row carries the active resume state
- **THEN** progress renders inline next to the title exactly as before this change
- **AND** the duration slot is unchanged

#### Scenario: Structural rows are displayed only
- **WHEN** a Heading or Spacer is rendered
- **THEN** it occupies display geometry
- **AND** it cannot be selected or activated

#### Scenario: Group headings share one treatment
- **WHEN** any grouped list (artist, feed age bucket, letter bucket, season) renders its Heading rows
- **THEN** each label paints bold in the FOAM metadata role

#### Scenario: Trailing metadata carries its own role
- **WHEN** a row carries a release year
- **THEN** the year paints right-aligned in the row's fixed six-column date gutter, in the green (`STATUS_AVAILABLE`) role
- **AND** a row carrying a progress badge instead paints it inline next to the title in the FOAM metadata role

#### Scenario: Publish date paints in the fixed right-aligned gutter
- **WHEN** a row carries a publish date
- **THEN** the date paints right-aligned in a six-column gutter at the row's right edge, in the same green (`STATUS_AVAILABLE`) role a release year uses, whatever the date string's own length is
- **AND** the title's slot shrinks by that gutter so no column collides
- **AND** a row carrying no publish date and no release year reserves no gutter and its title keeps the full slot

#### Scenario: Durations share one precise format
- **WHEN** the Queue list shows a duration
- **THEN** every row uses the same `M:SS`/`H:MM:SS` format
- **AND** the duration paints in its own (`DURATION`) role, not the green gutter role
- **AND** imprecise forms (`4m`, `1h12m`, unbounded `62:03`) never appear in list rows

#### Scenario: Library list rows carry no duration
- **WHEN** a library list row is projected (Home, Podcast, TV episode, music track, book chapter, feed entry)
- **THEN** it carries no duration string
- **AND** its row paints no right-aligned duration slot (a date gutter is not a duration)

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string
- **AND** the painter suppresses the duration slot even if one is projected

#### Scenario: Split rows share the now-playing palette
- **WHEN** a row carries both a primary text and a secondary title
- **THEN** the primary text paints in the split-row context role and the secondary title paints in the split-row title role
- **AND** the treatment is identical on every list that projects a split row, with no per-list opt-in
- **AND** the now-playing playback strip's title roles are unaffected

#### Scenario: Single-part rows keep the ordinary title role
- **WHEN** a row carries no secondary title
- **THEN** its primary text paints in the ordinary title role for its semantic state, not the split-row palette

#### Scenario: Played split rows mute the item title only
- **WHEN** a played row carries a secondary title
- **THEN** the secondary title paints in the played muted role
- **AND** the primary context text keeps the split-row context role

#### Scenario: Group headings never carry a gutter
- **WHEN** a Heading or Spacer row is rendered
- **THEN** it reserves no date-gutter column, whatever the surrounding item rows carry
- **AND** only selectable Item rows can carry a release year or publish date

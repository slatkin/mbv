# canonical-media-lists Delta

## MODIFIED Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, an optional secondary title (the container name of a split row, painted after the primary text), an optional left-aligned trailing slot whose text role is closed — a release year paints in the green (`STATUS_AVAILABLE`) metadata role, a progress badge in the FOAM metadata role — a media kind (`Collection` for navigable containers, `Media` for playable leaves), an optional duration string (painted right-aligned in its own (`DURATION`) role, distinct from the
green year role), and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. A Heading SHALL paint its label bold in the FOAM metadata role, so every grouped list's group label reads the same. Heading and Spacer SHALL be excluded from selectable-target indexing. When a duration is shown it SHALL use the precise `M:SS`/`H:MM:SS` form (queue format, e.g. `4:32`, `1:02:03`); `Collection` rows SHALL NOT carry a duration. `Active` SHALL keep its existing meaning of stored resume progress rendered inline as trailing metadata; `NowPlaying` SHALL mean live playback, rendering its live progress inline as trailing metadata and its total duration like every other row — no throbber glyph appears in any row. On Wide lists, `NowPlaying` rows SHALL be marked by an aqua right-pointing play glyph before the title (one space from it) rather than an accent-coloured title; their title text SHALL keep the ordinary colour. `Active` rows SHALL not be marked by the play glyph. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

A row carrying a secondary title SHALL paint as a split row in the now-playing palette: the primary text (the container/context name) SHALL paint in the playback-context gold (`PLAYBACK_CONTEXT_FG`) role and the secondary title (the item's own name) SHALL paint in the playback-title aqua (`PLAYBACK_TITLE_FG`) role, with one separating space. A row with no secondary title SHALL keep the ordinary title role for its primary text. A played split row SHALL mute the item title to the played (`TEXT_MUTED`) role while the context part keeps the playback-context gold role. No other semantic state SHALL move the split-row palette. The truncation priority of a split row is unchanged: the secondary title keeps its width (up to the whole title slot) and the primary text ellipsises first.

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
- **THEN** the year paints in the green metadata role
- **AND** a row carrying a progress badge instead paints it in the FOAM metadata role

#### Scenario: Durations share one precise format
- **WHEN** any media list shows a duration (queue, home, feeds, TV episode, music track, book chapter)
- **THEN** every row uses the same `M:SS`/`H:MM:SS` format
- **AND** the duration paints in its own (`DURATION`) role, not the green metadata role
- **AND** imprecise forms (`4m`, `1h12m`, unbounded `62:03`) never appear in list rows

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string
- **AND** the painter suppresses the duration slot even if one is projected

#### Scenario: Split rows share the now-playing palette
- **WHEN** a row carries both a primary text and a secondary title
- **THEN** the primary text paints in the playback-context gold role and the secondary title paints in the playback-title aqua role
- **AND** the treatment is identical on every list that projects a split row, with no per-list opt-in

#### Scenario: Single-part rows keep the ordinary title role
- **WHEN** a row carries no secondary title
- **THEN** its primary text paints in the ordinary title role for its semantic state, not the split-row palette

#### Scenario: Played split rows mute the item title only
- **WHEN** a played row carries a secondary title
- **THEN** the secondary title paints in the played muted role
- **AND** the primary context text keeps the playback-context gold role

## ADDED Requirements

### Requirement: Library Wide lists stripe with their panel's contrasting pair

Both library Wide lists SHALL paint zebra striping with the semantics the Queue list already ships:
item-only screen-row parity, `Heading` and `Spacer` rows never striped, and the selected row keeping
its own parity under the gutter accent. Each arm SHALL resolve its secondary pair from the surface
table identity of the panel body that surrounds it, never from a raw colour value:

- the Browser-pane list, whose list box is filled with the `LibraryPanel` surface, SHALL stripe with
  the `MainContentBox` pair (focused `#48584e`, unfocused `#2d353b`);
- the provider Workspace list, whose box is filled with the `MainContentBox` surface, SHALL stripe
  with the `LibraryPanel` pair (focused `#3c4841`, unfocused `#333c43`).

A stripe SHALL always resolve to a pair other than the one its own list box is filled with: a stripe
whose colours equal its panel body's paints no visible alternation. Zebra striping SHALL remain a
per-list policy — these pairs apply to the two library Wide lists and change nothing on any other
list.

#### Scenario: Browser-pane list stripes with the content-box pair

- **WHEN** the Browser-pane Wide list renders focused
- **THEN** the 1st, 3rd, and 5th visible selectable items carry the `MainContentBox` focused fill
- **AND** the remaining visible items carry no secondary background

#### Scenario: Workspace list stripes with the library-panel pair

- **WHEN** a provider Workspace Wide list renders focused
- **THEN** the striped positions carry the `LibraryPanel` focused fill
- **AND** that fill differs from the `MainContentBox` fill the Workspace box is painted with

#### Scenario: A stripe never equals its own panel body

- **WHEN** either library Wide arm renders in either focus state
- **THEN** its stripe colour differs from the fill its own list box is painted with in that state

#### Scenario: Library headings never stripe

- **WHEN** a `Heading` or `Spacer` row appears between two library items
- **THEN** it paints with no zebra background and does not shift the item stripe pattern

### Requirement: Wide presentation zebra striping

The Wide presentation SHALL accept an optional zebra-stripe policy on its paint policy. The policy
SHALL carry a focused and an unfocused secondary background colour. When zebra striping is enabled,
the painter SHALL apply the secondary background colour to every even-numbered visible selectable
`Item` row, counting only selectable `Item` rows in the visible window by screen-row order
(zero-indexed, so the first visible item is even and striped, the second is odd, etc.). Headings and
Spacers SHALL always paint with no zebra background regardless of the policy. When zebra striping is
disabled (the default), row backgrounds SHALL be unchanged from today's behaviour. Because every Wide
list marks its selection with the gutter accent rather than a selected-row background (see
*WideMediaList owns fixed-row mechanics*), the selected row SHALL keep its own zebra parity: a
selected row on a striped position paints the stripe and the accent title together, and a selected row
on an unstriped position paints neither. For an unselected striped row, the secondary background SHALL
be confined to the text-flow range inside the row's existing two-column left and right gutters; the
parent background SHALL remain visible in those gutters and in any scrollbar column. Row geometry,
width calculations, and hit geometry SHALL remain unchanged.

#### Scenario: Zebra stripes are contained within existing row gutters

- **WHEN** a Wide presentation renders an unselected striped row
- **THEN** its secondary background spans only the text-flow range inside the existing two-column
  gutters
- **AND** the gutters and scrollbar column retain the parent background without changing row geometry

#### Scenario: Zebra stripes alternate among selectable items

- **WHEN** a Wide presentation has zebra striping enabled and renders five visible selectable `Item`
  rows
- **THEN** the 1st, 3rd, and 5th visible items paint with the secondary background colour matching the
  current focus state
- **AND** the 2nd and 4th visible items paint with no secondary background

#### Scenario: Headings and spacers are excluded from zebra counting

- **WHEN** a visible Heading or Spacer row appears between two selectable `Item` rows
- **THEN** the Heading or Spacer has no zebra background
- **AND** the item-only counter does not increment for the structural row

#### Scenario: Selected row keeps its stripe

- **WHEN** the selected row falls on a zebra-striped position on a focused Wide list
- **THEN** the row paints the zebra background
- **AND** its title carries the gutter accent

#### Scenario: Zebra is screen-row-parity based

- **WHEN** the list scrolls by one row
- **THEN** the first visible selectable item is always striped and alternation follows screen-row order
  regardless of its source-row index

#### Scenario: Zebra is disabled by default

- **WHEN** a Wide presentation is configured without a zebra-stripe policy
- **THEN** all unselected rows paint with no explicit background, matching today's behaviour

## MODIFIED Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, an optional secondary title (the episode title of a series/show row, painted after the primary text in the yellow focus-accent role while the primary stays in the ordinary title role), optional trailing text, a media kind (`Collection` for navigable containers, `Media` for playable leaves), an optional duration string, and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. Heading and Spacer SHALL be excluded from selectable-target indexing. When a duration is shown it SHALL use the precise `M:SS`/`H:MM:SS` form (queue format, e.g. `4:32`, `1:02:03`); `Collection` rows SHALL NOT carry a duration. `Active` SHALL keep its existing meaning of stored resume progress rendered inline as trailing metadata; `NowPlaying` SHALL mean live playback, rendering its live progress inline as trailing metadata and its total duration like every other row — no throbber glyph appears in any row. On Wide lists, `NowPlaying` rows SHALL be marked by an aqua right-pointing play glyph before the title (one space from it) in place of any accent title colour; their title text SHALL keep the ordinary colour. `Active` rows SHALL not be marked by the play glyph. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

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

#### Scenario: Durations share one precise format
- **WHEN** any media list shows a duration (queue, home, feeds, TV episode, music track, book chapter)
- **THEN** every row uses the same `M:SS`/`H:MM:SS` format
- **AND** imprecise forms (`4m`, `1h12m`, unbounded `62:03`) never appear in list rows

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string
- **AND** the painter suppresses the duration slot even if one is projected


### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a
shared canonical media-list owner. It SHALL own fixed-height one-column row placement, semantic
painting delegation, scrollbar presentation, viewport clamping, and internal current-frame row
geometry, while cursor, scroll, selected target, and other row-local state remain in the one logical
shared owner. The parent SHALL retain ownership of the destination panel/frame and establish its
current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure
those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only
ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY
supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no
rectangle, callback, provider data, or effect. Every Wide presentation SHALL render its selected row
with the gutter accent: the selected title paints bold in the focus-accent role while the list holds
focus, the row paints no selected-row background, and it keeps its own zebra parity. There SHALL be
no per-list opt-in or opt-out, and no marker glyph. An unfocused list SHALL show no selection accent
at all. A multi-selected row follows the same treatment as the selected row, including while the
list is unfocused.

On the Wide path the policy's owning-surface identity resolves the scrollbar column's backing fill
only; the selected row never fills with it. The `InlineMediaBrowser` presentation keeps its
selected-row background treatment unchanged.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected
target/selected-row rectangle, and point-resolution facts, and expose no mutable row map or
`RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve
it from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area
rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point
and expose no selected/detail geometry.

It SHALL support Wide hero rails, provider workspace rows, and Queue fixed rows, but SHALL NOT
implement Inline replacement or Grid placement. Letter grouping SHALL use
`MediaListRow::Heading`/`Spacer` rows. Queue SHALL use the shared canonical owner with Wide
presentation in every panel mode.

#### Scenario: A gutter-accent selected row keeps default painting otherwise

- **WHEN** a Wide list renders its selected row while focused
- **THEN** the title paints bold in the focus-accent role
- **AND** the row background is whatever an unselected row in that position would paint, including a
  zebra stripe
- **AND** no icon or marker glyph appears in or beside the row

#### Scenario: An unfocused Wide list shows no selection accent

- **WHEN** a Wide list renders while unfocused
- **THEN** no selectable item row carries the bold focus-accent selected title
- **AND** striped positions still show the unfocused secondary background

#### Scenario: A multi-selected row follows the selected-row treatment

- **WHEN** a Wide list renders rows that are part of a multi-selection while the list is unfocused
- **THEN** each multi-selected row paints the gutter accent title and no selected-row background
- **AND** it keeps its own zebra parity

#### Scenario: Wide TV rail composes the control

- **WHEN** the TV surface is Wide hero
- **THEN** its series rail is painted by one Wide presentation over the shared owner
- **AND** the parent retains workspace, hero, images, chrome, and effects.

#### Scenario: Queue paints and resolves one current row flow

- **WHEN** Queue paints a non-empty fixed-row list in any panel mode
- **THEN** its Wide presentation receives equal established claim and row-flow rectangles through one
  component view and paints ordinary rows once
- **AND** Queue resolves a later row point to the `QueueSlotId` from that current retained result
- **AND** Queue does not rebuild row rectangles or a selectable row map.

#### Scenario: Grouped Music paints both Wide row flows

- **WHEN** Grouped Music paints its Wide album rail or Wide track table
- **THEN** each flow uses a Wide presentation over its shared owner
- **AND** Grouped Music resolves later row points from the current retained result without a cursor
  mirror or row map
- **AND** its existing panel framing and spacing are unchanged.

#### Scenario: Other provider workspaces paint fixed rows

- **WHEN** TV paints episodes, Audiobookshelf Podcast paints episodes, or Audiobookshelf Book paints
  chapter/audio-part rows
- **THEN** each flow uses a Wide presentation over its shared owner
- **AND** the destination resolves later row points from the current retained result without a cursor
  mirror or row map.

#### Scenario: A Wide result expires before another view

- **WHEN** a Wide presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the presentation as having no list target until the current view finishes.

## REMOVED Requirements

### Requirement: Wide presentation supports optional zebra striping

**Reason**: Superseded by "Wide presentation zebra striping" in this change. Its selected-row sentence
("The selected row SHALL always use the selected-row background, never the zebra background — except
on a list using the gutter-accent selection treatment") and its "Selected row overrides zebra" scenario
describe a per-list choice that stops existing once the gutter accent becomes the only Wide treatment;
keeping either wording would leave the synced spec asserting a selected-row background that no Wide
list paints.

**Migration**: Read "Wide presentation zebra striping". Parity, item-only counting, structural-row
exclusion, gutter containment, and geometry claims are unchanged; only the selected-row sentence and
its scenario move to "the selected row keeps its parity under the accent".

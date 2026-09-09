# canonical-media-lists Specification

## Purpose

Provide reusable embedded TuiRealm list controls with one owner for list interaction and geometry across the first canonical media-list migration slice.

## Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, optional trailing text, a media kind (`Collection` for navigable containers, `Media` for playable leaves), an optional duration string, and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. Heading and Spacer SHALL be excluded from selectable-target indexing. When a duration is shown it SHALL use the precise `M:SS`/`H:MM:SS` form (queue format, e.g. `4:32`, `1:02:03`); `Collection` rows SHALL NOT carry a duration. `Active` SHALL keep its existing meaning of stored resume progress rendered inline as trailing metadata; `NowPlaying` SHALL mean live playback and render only in the right-aligned slot. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

#### Scenario: Queue-like progress is presented safely
- **WHEN** a parent supplies active progress
- **THEN** the control receives only a bounded percentage
- **AND** playback and queue authority remain with the parent/shell

#### Scenario: Now-playing renders right with throbber
- **WHEN** a row carries the now-playing state
- **THEN** no inline progress appears next to the title
- **AND** the right-aligned slot shows the paint-time block-ramp throbber glyph followed by the progress percentage, replacing any duration
- **AND** the glyph uses the aqua liveness theme role and the percentage uses `TEXT_METADATA` (FOAM); neither uses the green duration role (`STATUS_AVAILABLE`)

#### Scenario: Now-playing with unknown runtime
- **WHEN** a now-playing row carries no progress
- **THEN** the right-aligned slot shows the throbber glyph alone

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

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm
`Component` that owns cursor, scroll, viewport, fixed-height one-column row
placement, semantic painting delegation, scrollbar, movement, clamping, and
internal row geometry for painting, scrolling, and point resolution. The
parent SHALL retain ownership of the destination panel/frame and establish its
current claim and row-flow rectangles using its existing arrangement; before
view, it SHALL configure those rectangles on the control. The control's
`Component::view` SHALL paint the established row flow once and SHALL be the
only ordinary-row painting entry point for that control in a frame. Before that
call, the parent MAY supply only a closed semantic policy for focused/selected
treatment and optional throbber; the policy SHALL contain no rectangle, raw
style, callback, provider data, or effect.

The control SHALL retain the current frame's read-only claim/content rectangles,
selected target/selected-row rectangle, and point-resolution facts, and expose
no mutable row map or `RowGeometry` to a migrated parent. A point-resolution
call after view SHALL accept only the point and resolve it from that retained
geometry. Configuring the control, beginning view, or viewing an empty/zero-area
rectangle SHALL invalidate a prior result; before the current view completes,
it SHALL claim no point and expose no selected/detail geometry. Untouched
destinations MAY retain their existing compatibility painter and geometry path
until their own migration.

It SHALL support Wide hero rails and Queue fixed rows, but SHALL NOT implement
Inline replacement or a non-hero two-column policy. It SHALL express letter
grouping through `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use
`WideMediaList<QueueSlotId>` as its fixed-row control in every panel mode.

#### Scenario: Wide TV rail composes the control

- **WHEN** the TV surface is Wide hero
- **THEN** its left rail is painted and interacted with by one `WideMediaList`
- **AND** the parent retains workspace, hero, images, and effects

#### Scenario: Queue paints and resolves one current row flow

- **WHEN** Queue paints a non-empty fixed-row list in any panel mode
- **THEN** its persistent `WideMediaList<QueueSlotId>` receives equal established
  claim and row-flow rectangles through one component view and paints ordinary
  rows once
- **AND** Queue resolves a later row point to the `QueueSlotId` from that
  current retained result
- **AND** Queue does not rebuild row rectangles or a selectable row map

#### Scenario: Grouped Music paints both Wide row flows

- **WHEN** Grouped Music paints its Wide album rail or Wide track table
- **THEN** each persistent `WideMediaList` receives its parent-established
  claim and row-flow rectangles through one component view and paints ordinary
  rows once
- **AND** Grouped Music resolves a later row point from that current retained
  result without a uniform row map
- **AND** its existing panel framing and spacing are unchanged

#### Scenario: A Wide result expires before another view

- **WHEN** a Wide control is configured for a new frame or receives an empty or
  zero-area view
- **THEN** its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the control as having no list target until the current
  view finishes

### Requirement: InlineMediaBrowser owns selected-row replacement

`InlineMediaBrowser<Target>` SHALL be a persistent embedded plain TuiRealm
`Component` owning one-column placement, selection visibility, variable-height
selected-row replacement admission, ordinary-row fallback when replacement
cannot fit, semantic painting delegation, and its internal row and replacement
geometry for painting, scrolling, and point resolution. The parent SHALL retain
ownership of destination framing and establish its current claim and row-flow
rectangles using its existing arrangement; before view, it SHALL configure those
rectangles on the control. The control's `Component::view` SHALL paint the
established row flow once and SHALL be its only ordinary-row painting entry point
for a frame. Before that call, the parent MAY supply only a closed semantic
policy for focused/selected treatment and desired detail admission; the policy
SHALL contain no rectangle, raw style, callback, provider data, or effect.

The control SHALL retain the current frame's read-only claim/content rectangles,
selected target/selected-row rectangle, admitted detail rectangle, and
point-resolution facts, and expose no mutable row map or `RowGeometry` to a
migrated parent. A point-resolution call after view SHALL accept only the point
and resolve ordinary and replacement targets from that retained geometry.
Configuring the control, beginning view, or viewing an empty/zero-area rectangle
SHALL invalidate a prior result; before the current view completes, it SHALL
claim no point and expose no detail rectangle. Untouched destinations MAY retain
their existing compatibility painter and geometry path until their own
migration.

It SHALL be distinct from Inline Search, SHALL NOT be constructed during a
render pass, and SHALL NOT become a second mounted identity, subscription,
focus target, gesture recognizer, or router.

#### Scenario: A selected row is replaced

- **WHEN** the selected item fits the Inline presentation
- **THEN** its ordinary row is replaced once by the detail block
- **AND** there is no blank duplicate row and the parent target remains stable

#### Scenario: Grouped Music consumes one admitted detail result

- **WHEN** Grouped Music paints its Inline album list with a selected album
- **THEN** its persistent `InlineMediaBrowser` receives the established claim
  and row-flow rectangles through one component view and admits or falls back
  from detail within that view
- **AND** Grouped Music reads only the current retained admitted-detail rectangle
  before painting its provider-owned detail
- **AND** Grouped Music does not rerun list layout or reconstruct selectable row
  geometry
- **AND** existing framing and spacing are unchanged

#### Scenario: An Inline result expires before another view

- **WHEN** an Inline control is configured for a new frame or receives an empty
  or zero-area view
- **THEN** its prior point claim and admitted-detail rectangle are unavailable
- **AND** a parent treats the control as having no list target or detail region
  until the current view finishes

### Requirement: Responsive handoff preserves an explicit anchor
At Wide↔Narrow transitions the parent SHALL hand off `ViewportAnchor { selected_target, selected_row_offset }`, with offset measured from viewport top to the selected ordinary row. The receiving control SHALL preserve the offset when possible and clamp it to its viewport otherwise. Ordinary refreshes SHALL preserve target and locally clamp without adopting shell cursor/scroll mirrors.

#### Scenario: TV re-anchors across breakpoints
- **WHEN** TV changes Wide→Narrow→Wide
- **THEN** characterization records the existing selected target, cursor, scroll, and screen-row offset
- **AND** replacement matches that evidence unless separately approved

### Requirement: Named destinations compose without changing provider authority
The slice SHALL compose persistent `WideMediaList` controls in the applicable Wide hero paths and persistent `InlineMediaBrowser` controls in the applicable Narrow paths for hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, and TV Series browsing. Non-hero two-column Emby catalogs SHALL keep their existing two-column arrangement policy and SHALL NOT be forced onto either canonical control. Provider workspaces, images, effects, persistence, Service and Player authority, and typed message translation SHALL remain in their existing parents/shell.

#### Scenario: One painter is active
- **WHEN** a listed destination is rendered at its applicable breakpoint
- **THEN** exactly one list painter runs
- **AND** the old loop is not run as an underpaint

### Requirement: Migration is accepted as one verified slice
The implementation, representative stateful and rendered tests, automated gates, review, and acceptance SHALL form one uninterrupted slice. There SHALL be no pre-test visual-approval checkpoint. Affected surfaces SHALL provide metadata/state/image-bearing rendered evidence, stateful target-and-anchor evidence, source-level one-painter evidence, manual/live Wide/Narrow evidence, and the 800-line file-size gate before acceptance. A visual defect found during review or acceptance SHALL be treated as a bug, fixed, and followed by rerunning the affected tests and gates.

#### Scenario: Evidence precedes acceptance
- **WHEN** the implementation changes a visual surface
- **THEN** representative tests and automated gates run before review and acceptance
- **AND** live Wide/Narrow review remains part of acceptance
- **AND** any defect found there is fixed before the slice is accepted

### Requirement: Home composes canonical list controls
Home SHALL compose a persistent `InlineMediaBrowser` for the inline section and a persistent `WideMediaList` where the approved Wide arrangement requires a fixed one-column rail. Section identity SHALL remain keyed by `pref_key` and restored through `restore_section`. Home SHALL keep exactly one active section with one flat cursor and scroll position owned by the active control; only the active section's rows SHALL be projected into that control. Ordinary refresh SHALL preserve stable target and locally clamp without adopting parent cursor/scroll. Breakpoint or discrete navigation transitions SHALL use one `ViewportAnchor`, with no per-section cursor cache and no App-wide interaction mirror.

#### Scenario: Home refresh preserves section state
- **WHEN** the active Home section refreshes or the active variant changes
- **THEN** refresh preserves or clamps the control-owned stable target locally
- **AND** a variant transition performs one target/offset `ViewportAnchor` handoff
- **AND** `pref_key`/`restore_section`, images, and workspace effects remain shell/parent-owned.

### Requirement: Feeds projects structural rows
The Feeds Service/tab SHALL project FeedAgeGroup/date labels as non-selectable `Heading` rows and separators as non-selectable `Spacer` rows as canonical-list content. Only media `Item` rows SHALL enter selectable indexing. The subscription/group selector pills and the watched selector SHALL remain parent-owned chrome outside the canonical control and SHALL NOT be projected as canonical rows.

#### Scenario: Structural rows do not capture selection
- **WHEN** a user moves through a grouped Feeds list
- **THEN** cursor movement skips headings and spacers and activation resolves the selected FeedEntry target.

### Requirement: Canonical source of truth owns row presentation
Migrated Home and Feeds rows SHALL use the canonical row model and painter. The deferred two-space row-indent correction from `restore-feeds-service-wide-list` (umbrella task 1.3a) SHALL be implemented at that source of truth, not by destination-specific offsets.

#### Scenario: Wide Feeds remains one column
- **WHEN** the Feeds Service/tab is rendered at an accepted Wide breakpoint
- **THEN** it uses one column with the accepted `restore-feeds-service-wide-list` (umbrella task 1.3a) framing/background and selected-row semantics.

### Requirement: Provider destinations compose canonical media controls

Grouped Music album browsing and Audiobookshelf Podcast show browsing and Book browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose `WideMediaList` for Wide rails and `InlineMediaBrowser` for Normal/Narrow selected-row replacement where the arrangement permits. The controls SHALL remain embedded beneath the mounted destination component; provider workspaces, images, selectors, surname buckets, effects, and typed intent translation remain parent-owned.

#### Scenario: Music groups retain provider authority
- **WHEN** a grouped Music album surface is rendered or navigated
- **THEN** album/group rows use the canonical control
- **AND** grouping, track authority, images, selection, and playback intents remain Music-owned
- **AND** no second list painter or App-owned interaction mirror runs.

#### Scenario: Audiobookshelf shows compose without losing episodes
- **WHEN** a Podcast library is shown Wide or Normal
- **THEN** shows use the canonical list presentation
- **AND** the selected show's episode workspace remains provider-owned, including episode filtering, images, and typed playback intents.

#### Scenario: Audiobookshelf books compose without duplicate detail
- **WHEN** a Book library is shown Wide
- **THEN** the selected book's persistent provider detail workspace renders in the right pane and the left rail shows ordinary fixed-height one-column canonical rows
- **AND** no selected-row replacement and no Inline hero is painted in the Wide left rail
- **AND** chapter rows remain provider-owned seek targets for the selected book.

### Requirement: Audiobookshelf geometry has complete breakpoint fallbacks

Audiobookshelf Podcast and Book surfaces SHALL use the shared Wide hero or Inline arrangement at the established Wide/Normal breakpoints, preserve the short-height fallback, and hand off stable selected target and viewport anchor across breakpoint changes. Non-list repairs required to make the composition correct SHALL live in shared arrangements or the owning destination component, not a bespoke exception.

#### Scenario: Wide and short layouts are deterministic
- **WHEN** terminal width/height crosses the Wide threshold or the short-height guard
- **THEN** the surface selects the defined Wide, Normal, or short fallback arrangement
- **AND** the selected target, row offset, images, framing, and focus remain stable.

### Requirement: TV and Movies establish the Wide composition precedent

When grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide width and minimum-height predicate, it SHALL follow the TV/Movies composition: its provider-owned detail/workspace SHALL occupy the right pane, and its parent-owned browser-level pills, followed by ordinary one-column canonical rows, SHALL occupy the left rail. The arrangement SHALL use the same shared predicate, pane framing, content spacing, and short-height fallback as TV/Movies. The Wide presentation SHALL NOT use an Inline hero or selected-row replacement in the left rail; when the shared predicate is not met, the destination SHALL use the shared Inline fallback (or suppress detail when the shared minimum cannot fit), not a bespoke arrangement. The arrangement mechanics of this precedent — shared predicate, pane framing, content spacing, and short-height fallback — are specified by the `right-panel-arrangements` spec; this requirement governs only how the canonical controls compose into that arrangement.

#### Scenario: Wide provider workspace and ordinary rail
- **WHEN** grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide geometry conditions
- **THEN** its provider-owned detail/workspace is on the right
- **AND** its browser-level pills and ordinary one-column canonical rows are on the left
- **AND** no Wide Inline hero or selected-row replacement is painted in the left rail.

#### Scenario: Shared predicate and fallback apply
- **WHEN** the destination crosses the shared width or minimum-height guard
- **THEN** it uses the same predicate, pane framing, content spacing, and short-height fallback as TV/Movies
- **AND** it does not introduce a destination-specific arrangement or breakpoint.

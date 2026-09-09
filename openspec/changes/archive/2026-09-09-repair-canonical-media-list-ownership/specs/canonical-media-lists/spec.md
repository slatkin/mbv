# Canonical Media-Lists Delta

## MODIFIED Requirements

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

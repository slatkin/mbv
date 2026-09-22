# Spec Delta

## REMOVED Requirements

### Requirement: One shared owner supports list-local extension

**Reason**: This requirement carved the Grouped Music tree browser out of its own scope ("The Grouped Music artist/album browser SHALL be owned by its shallow tree and is not a canonical media-row flow") because the flat list and the tree shared no machinery. The shared list seam removes that premise: both are now shapes over one row flow, and the guarantees this requirement made — exactly one owner per logical flow, no duplicated cursor/scroll/membership/hit state in destinations, one implementation site for list-local mechanics — are stated once in `shared-list-components` for every list shape rather than for the flat list alone. Keeping both would leave two contracts disagreeing about whether the tree is in scope.

**Migration**: The one-owner and single-implementation-site guarantees are carried by "Shared list mechanics have exactly one implementation site" and "Every list is one ordered row flow addressed by stable target" in `shared-list-components`, which apply to both list shapes. The enumerated in-scope-flow list is dropped rather than relocated: it named the destinations that had migrated at the time and was a migration checklist, not a behavior contract. Destination-side authority — that parents retain Service content, chrome, Workspaces, images, effects, persistence, and typed intent translation, and retain no second cursor, scroll, membership, hit map, or selected-row identity — remains stated in "Provider destinations compose canonical media controls" and "Stable targets cross every canonical boundary".

### Requirement: Responsive handoff preserves an explicit anchor

**Reason**: Geometry-change behavior is shared list mechanics, not flat-list-specific behavior. The seam owns viewport preservation and clamping for every list shape, so stating it again here would mean two specs describing the same arithmetic — and only one of them mentioning the tree.

**Migration**: Replaced by "The viewport keeps the selection visible and clamps at bounds" in `shared-list-components`, which preserves the prior viewport offset where bounds permit, otherwise scrolls the minimum needed, clamps in place on a geometry change, and forbids copying cursor or viewport state into a second control. The explicit anchor and its re-anchoring allowance are preserved as first-class clauses of that same requirement: the anchor is the selected stable target plus its zero-based viewport row offset (the `ViewportAnchor` contract migrated from `media_list/anchor.rs`), a responsive handoff restores it with clamping, and only a discrete navigation or restoration boundary MAY explicitly re-anchor a list from a shell-owned stable target and row offset.

### Requirement: Canonical geometry has no compatibility path

**Reason**: Retained-paint-geometry and point resolution are shared list mechanics. This requirement and the tree's own independently grown equivalent are the concrete duplication this change exists to remove — the two are spelled differently in code today (one invalidates through an explicit call, the other through a flag cleared at three sites), which is exactly the drift a single contract prevents.

**Migration**: Replaced by "Point resolution uses only completed-frame geometry" in `shared-list-components`, which retains only the latest completed paint's geometry, invalidates on content or geometry change and on an incomplete paint, claims no point and exposes no selected-row geometry while invalid, accepts only the point, returns a stable target, and forbids exposing a mutable row map, a per-row rectangle set, or caller-supplied resolution geometry while permitting the one retained read-only selected-row rect. It applies to every list shape rather than to in-scope canonical flows only.

## MODIFIED Requirements

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for the flat list shape. It SHALL own fixed-height one-column row placement, semantic painting delegation, and scrollbar presentation. Row flow, cursor, scroll, viewport clamping and anchor restore, selected target, retained current-frame geometry, and point resolution SHALL come from the shared list seam rather than from this presentation. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no rectangle, callback, provider data, or effect. Every fixed-row presentation SHALL render its selected row with the selected-row bar: while the list holds focus the whole row paints the bar fill across its full width, overriding its zebra stripe, its two-column gutters, and any owning-surface identity, while every span keeps the ordinary unselected foreground role, so no title is bold and no accent title colour is introduced. There SHALL be no per-list opt-in or opt-out, and no marker glyph. An unfocused list SHALL paint no bar for its cursor row. A multi-selected row SHALL paint the bar too, including while the list is unfocused.

The policy's owning-surface identity SHALL NOT move the selected row's fill: every selected row paints the bar whatever surface owns it. A list that paints a scrollbar column SHALL keep the parent background there — the bar reaches the panel edge on its own row only.

It SHALL support Wide Hero browser rails, non-Wide Library browser lists, provider Workspace rows, and Queue fixed rows for destinations whose flow is flat, but SHALL NOT implement selected-row replacement or Grid placement. A destination whose flow nests uses the tree list shape over the same seam instead. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use this presentation in every Panel mode.

#### Scenario: A selected row paints the selected-row bar
- **WHEN** a fixed-row list renders its selected row while focused
- **THEN** the row paints the bar fill across its full width, overriding its zebra stripe and its two-column gutters
- **AND** every span keeps the ordinary unselected foreground role, with no bold title
- **AND** no icon or marker glyph appears in or beside the row

#### Scenario: An unfocused list paints no bar for its cursor row
- **WHEN** a fixed-row list renders while unfocused
- **THEN** no selectable cursor row carries the bar fill
- **AND** striped positions still show the unfocused secondary background when striping applies

#### Scenario: A multi-selected row paints the bar
- **WHEN** a fixed-row list renders rows that are part of a multi-selection while the list is unfocused
- **THEN** each multi-selected row paints the bar with its ordinary foreground
- **AND** the bar overrides its zebra stripe

#### Scenario: Wide TV rail composes the control
- **WHEN** the TV browser renders in Wide or non-Wide geometry
- **THEN** its series rail is painted by one fixed-row presentation over the shared seam
- **AND** the parent retains Workspace, Hero, images, chrome, overlay, and effects

#### Scenario: Queue paints and resolves one current row flow
- **WHEN** Queue paints a non-empty fixed-row list in any Panel mode
- **THEN** its presentation receives equal established claim and row-flow rectangles through one component view and paints ordinary rows once
- **AND** Queue resolves a later row point to the `QueueSlotId` from the seam's retained result
- **AND** Queue does not rebuild row rectangles or a selectable row map

#### Scenario: Grouped Music paints both Wide row flows
- **WHEN** Grouped Music paints its album-track or artist-track Workspace
- **THEN** the flow uses the fixed-row presentation over the shared seam
- **AND** Grouped Music resolves later row points from the seam's retained result without a cursor mirror or row map
- **AND** the tree browser uses the tree list shape rather than this presentation

#### Scenario: Other provider workspaces paint fixed rows
- **WHEN** TV paints episodes, Audiobookshelf Podcast paints episodes, or Audiobookshelf Book paints chapter or audio-part rows
- **THEN** each flow uses the fixed-row presentation over the shared seam
- **AND** the destination resolves later row points from the seam's retained result without a cursor mirror or row map

#### Scenario: A Wide result expires before another view
- **WHEN** a fixed-row presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** the seam's retained geometry is invalidated, so its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the presentation as having no list target until the current view finishes

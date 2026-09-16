## MODIFIED Requirements

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a shared canonical media-list owner. It SHALL own fixed-height one-column row placement, semantic painting delegation, scrollbar presentation, viewport clamping, and internal current-frame row geometry, while cursor, scroll, selected target, and other row-local state remain in the one logical shared owner. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no rectangle, callback, provider data, or effect. Every fixed-row presentation SHALL render its selected row with the gutter accent: the selected title paints bold in the focus-accent role while the list holds focus, the row paints no selected-row background, and it keeps its own zebra parity. There SHALL be no per-list opt-in or opt-out, and no marker glyph. An unfocused list SHALL show no selection accent at all. A multi-selected row follows the same treatment as the selected row, including while the list is unfocused.

The policy's owning-surface identity resolves the scrollbar column's backing fill only; the selected row never fills with it.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected target/selected-row rectangle, and point-resolution facts, and expose no mutable row map or `RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve it from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point and expose no selected geometry.

It SHALL support Wide Hero browser rails, non-Wide Library browser lists, provider Workspace rows, and Queue fixed rows, but SHALL NOT implement selected-row replacement or Grid placement. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use the shared canonical owner with this presentation in every Panel mode.

#### Scenario: A gutter-accent selected row keeps default painting otherwise

- **WHEN** a fixed-row list renders its selected row while focused
- **THEN** the title paints bold in the focus-accent role
- **AND** the row background is whatever an unselected row in that position would paint, including a zebra stripe
- **AND** no icon or marker glyph appears in or beside the row

#### Scenario: An unfocused Wide list shows no selection accent

- **WHEN** a fixed-row list renders while unfocused
- **THEN** no selectable item row carries the bold focus-accent selected title
- **AND** striped positions still show the unfocused secondary background when striping applies

#### Scenario: A multi-selected row follows the selected-row treatment

- **WHEN** a fixed-row list renders rows that are part of a multi-selection while the list is unfocused
- **THEN** each multi-selected row paints the gutter accent title and no selected-row background
- **AND** it keeps its own zebra parity

#### Scenario: Wide TV rail composes the control

- **WHEN** the TV browser renders in Wide or non-Wide geometry
- **THEN** its series rail is painted by one fixed-row presentation over the shared owner
- **AND** the parent retains Workspace, Hero, images, chrome, overlay, and effects

#### Scenario: Queue paints and resolves one current row flow

- **WHEN** Queue paints a non-empty fixed-row list in any Panel mode
- **THEN** its presentation receives equal established claim and row-flow rectangles through one component view and paints ordinary rows once
- **AND** Queue resolves a later row point to the `QueueSlotId` from that current retained result
- **AND** Queue does not rebuild row rectangles or a selectable row map

#### Scenario: Grouped Music paints both Wide row flows

- **WHEN** Grouped Music paints its album rail or track table
- **THEN** each flow uses the fixed-row presentation over its shared owner
- **AND** Grouped Music resolves later row points from the current retained result without a cursor mirror or row map
- **AND** its existing panel framing and spacing are unchanged

#### Scenario: Other provider workspaces paint fixed rows

- **WHEN** TV paints episodes, Audiobookshelf Podcast paints episodes, or Audiobookshelf Book paints chapter or audio-part rows
- **THEN** each flow uses the fixed-row presentation over its shared owner
- **AND** the destination resolves later row points from the current retained result without a cursor mirror or row map

#### Scenario: A Wide result expires before another view

- **WHEN** a fixed-row presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the presentation as having no list target until the current view finishes

### Requirement: Responsive handoff preserves an explicit anchor

A geometry change for one logical list SHALL reuse its shared canonical owner and fixed-row presentation. The presentation SHALL preserve the selected ordinary row's viewport offset when possible and clamp it to its new viewport otherwise. It SHALL NOT copy cursor, selected target, scroll, or other row-local state into a presentation-specific control. Ordinary refresh SHALL preserve the stable target and locally clamp. Only a discrete navigation or restoration boundary MAY explicitly re-anchor the shared owner from a shell-owned stable target and row offset.

#### Scenario: TV re-anchors across breakpoints

- **WHEN** TV changes between Wide and non-Wide geometry
- **THEN** the same logical series owner and fixed-row presentation preserve the selected stable target and row-local state
- **AND** the viewport preserves or clamps the selected row offset
- **AND** no shell cursor or scroll mirror is adopted

### Requirement: One shared owner supports list-local extension

Every in-scope logical media-row flow SHALL have exactly one shared owner for row content order, selectable-target indexing, cursor, scroll, authoritative selected-row identity, row-local interaction state, and row-local behavior. Its fixed-row presentation SHALL operate on that owner in Wide and non-Wide geometry rather than synchronize independent copies. A purely list-local state transition and row decoration SHALL be implementable in the shared canonical media-list subsystem without changing destination production code.

The in-scope flows SHALL be Queue slots; Home rows; generic Emby catalog rows; Movies and the Emby homevideos feed view; Grouped Music albums and tracks; TV series and episodes; Feeds entries; Audiobookshelf Podcast shows and filtered episodes; and Audiobookshelf Book titles and chapter/audio-part rows.

Parent destinations SHALL retain Service content, stable-target-to-domain lookup, active pane and component focus, section/group/filter/bucket/season/scope chrome, Workspaces, loading, images, effects, persistence, and provider-specific typed intent translation. They SHALL NOT retain a second row cursor, row scroll, row-local membership or range state, row hit map, or authoritative selected-row identity.

#### Scenario: A list-local behavior has one implementation site

- **WHEN** a developer adds a purely list-local state transition and visual decoration
- **THEN** the production change is confined to the shared canonical media-list state/behavior owner and shared row painter
- **AND** no destination production file changes
- **AND** browser and provider-Workspace media rows receive the behavior through their existing composition

#### Scenario: Responsive presentation does not synchronize local state

- **WHEN** one logical list changes between Wide and non-Wide geometry
- **THEN** the same shared owner and fixed-row presentation remain active
- **AND** no cursor, scroll, membership, range, or other row-local state is copied between controls

#### Scenario: Parent authority remains outside the row owner

- **WHEN** a destination changes a section, group, filter, surname bucket, season, queue scope, focused pane, or Library Hero overlay state
- **THEN** the destination or owning Panel remains authoritative for that chrome, Workspace, or overlay state
- **AND** it projects the resulting media rows into the shared owner
- **AND** provider-specific effects remain typed destination intents

## REMOVED Requirements

### Requirement: InlineMediaBrowser owns selected-row replacement

**Reason**: Non-Wide browsing now keeps ordinary fixed-height rows and opens detail in the Library Hero overlay, so a second selected-row-replacement presentation is no longer needed.

**Migration**: Reuse the fixed-row canonical presentation in every geometry and present Hero and Workspace content through the Library Hero overlay.

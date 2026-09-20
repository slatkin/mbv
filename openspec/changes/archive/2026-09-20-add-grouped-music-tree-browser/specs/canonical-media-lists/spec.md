## MODIFIED Requirements

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a shared canonical media-list owner. It SHALL own fixed-height one-column row placement, semantic painting delegation, scrollbar presentation, viewport clamping, and internal current-frame row geometry, while cursor, scroll, selected target, and other row-local state remain in the one logical shared owner. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no rectangle, callback, provider data, or effect. Every fixed-row presentation SHALL render its selected row with the selected-row bar: while the list holds focus the whole row paints the bar fill across its full width, overriding its zebra stripe, its two-column gutters, and any owning-surface identity, while every span keeps the ordinary unselected foreground role, so no title is bold and no accent title colour is introduced. There SHALL be no per-list opt-in or opt-out, and no marker glyph. An unfocused list SHALL paint no bar for its cursor row. A multi-selected row SHALL paint the bar too, including while the list is unfocused.

The policy's owning-surface identity SHALL NOT move the selected row's fill: every selected row paints the bar whatever surface owns it. A list that paints a scrollbar column SHALL keep the parent background there — the bar reaches the panel edge on its own row only.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected target/selected-row rectangle, and point-resolution facts, and expose no mutable row map or `RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve it from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point and expose no selected geometry.

It SHALL support Wide Hero browser rails except the Grouped Music tree browser, non-Wide Library browser lists except that tree browser, provider Workspace rows, and Queue fixed rows, but SHALL NOT implement selected-row replacement or Grid placement. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use the shared canonical owner with this presentation in every Panel mode.

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
- **THEN** its series rail is painted by one fixed-row presentation over the shared owner
- **AND** the parent retains Workspace, Hero, images, chrome, overlay, and effects

#### Scenario: Queue paints and resolves one current row flow
- **WHEN** Queue paints a non-empty fixed-row list in any Panel mode
- **THEN** its presentation receives equal established claim and row-flow rectangles through one component view and paints ordinary rows once
- **AND** Queue resolves a later row point to the `QueueSlotId` from that current retained result
- **AND** Queue does not rebuild row rectangles or a selectable row map

#### Scenario: Grouped Music paints both Wide row flows
- **WHEN** Grouped Music paints its album-track or artist-track Workspace
- **THEN** the flow uses the fixed-row presentation over its shared owner
- **AND** Grouped Music resolves later row points from the current retained result without a cursor mirror or row map
- **AND** the tree browser is not projected into this owner

#### Scenario: Other provider workspaces paint fixed rows
- **WHEN** TV paints episodes, Audiobookshelf Podcast paints episodes, or Audiobookshelf Book paints chapter or audio-part rows
- **THEN** each flow uses the fixed-row presentation over its shared owner
- **AND** the destination resolves later row points from the current retained result without a cursor mirror or row map

#### Scenario: A Wide result expires before another view
- **WHEN** a fixed-row presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the presentation as having no list target until the current view finishes

### Requirement: Provider destinations compose canonical media controls

Grouped Music album-track and artist-track Workspaces, Audiobookshelf Podcast episode browsing, and Audiobookshelf Book and chapter/audio-part browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose shared Wide or Inline presentations where their arrangements require them. The Grouped Music artist/album browser SHALL instead use the shallow tree specified by `grouped-music-tree-browser`; it SHALL NOT project artist roots as canonical `Heading` rows or album leaves into a parallel canonical owner. The controls SHALL remain embedded beneath the mounted destination component. Provider detail workspaces, images, selectors, surname buckets, filters, effects, and typed intent translation SHALL remain parent-owned; every canonical media-row flow's cursor, scroll, authoritative selected target, row-local behavior, and retained row geometry SHALL remain in its shared canonical owner.

#### Scenario: Music groups retain provider authority
- **WHEN** a grouped Music browser or Workspace is rendered or navigated
- **THEN** the artist/album browser uses its tree owner and each track Workspace uses its canonical media-list owner
- **AND** grouping, active-pane focus, images, content lookup, and playback intents remain Music-owned
- **AND** no parallel album-list owner, track cursor mirror, second painter, or App-owned interaction mirror runs

#### Scenario: Audiobookshelf shows compose without losing episodes
- **WHEN** an Audiobookshelf podcast library is shown Wide or Normal
- **THEN** episode rows use shared canonical ownership and delegation
- **AND** the pill row's selection and filtering, active-pane focus, images, content lookup, and typed playback intents remain Audiobookshelf-podcast-owned
- **AND** episode rows are not reseeded or reselected during painting

#### Scenario: Audiobookshelf books compose without duplicate detail
- **WHEN** a Book library is shown Wide or Normal
- **THEN** book and chapter/audio-part rows use shared canonical ownership and delegation
- **AND** surname buckets, active-pane focus, images, content lookup, and absolute chapter-seek intents remain Book-owned
- **AND** chapter/audio-part rows are not reseeded or reselected during painting

### Requirement: TV and Movies establish the Wide composition precedent

When grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide width and minimum-height predicate, it SHALL follow the TV/Movies composition: its provider-owned detail/workspace SHALL occupy the right pane, and its parent-owned browser-level pills followed by its one-column browser SHALL occupy the left rail. Grouped Music SHALL place its shallow artist/album tree in that browser slot; the other destinations SHALL place ordinary canonical rows there. The arrangement SHALL use the same shared predicate, pane framing, content spacing, and short-height fallback as TV/Movies. The Wide presentation SHALL NOT use an Inline hero or selected-row replacement in the left rail; when the shared predicate is not met, the destination SHALL use the shared Inline fallback (or suppress detail when the shared minimum cannot fit), not a bespoke arrangement. The arrangement mechanics of this precedent — shared predicate, pane framing, content spacing, and short-height fallback — are specified by the `right-panel-arrangements` spec; this requirement governs only how the browser control composes into that arrangement.

#### Scenario: Wide provider workspace and ordinary rail
- **WHEN** grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide geometry conditions
- **THEN** its provider-owned detail/workspace is on the right
- **AND** its browser-level pills and one-column browser are on the left
- **AND** Grouped Music uses the tree browser while the other destinations use canonical media rows
- **AND** no Wide Inline hero or selected-row replacement is painted in the left rail

#### Scenario: Shared predicate and fallback apply
- **WHEN** the destination crosses the shared width or minimum-height guard
- **THEN** it uses the same predicate, pane framing, content spacing, and short-height fallback as TV/Movies
- **AND** it does not introduce a destination-specific arrangement or breakpoint

### Requirement: One shared owner supports list-local extension

Every in-scope logical canonical media-row flow SHALL have exactly one shared owner for row content order, selectable-target indexing, cursor, scroll, authoritative selected-row identity, row-local interaction state, and row-local behavior. Its fixed-row presentation SHALL operate on that owner in Wide and non-Wide geometry rather than synchronize independent copies. A purely list-local state transition and row decoration SHALL be implementable in the shared canonical media-list subsystem without changing destination production code.

The in-scope flows SHALL be Queue slots; Home rows; generic Emby catalog rows; Movies and the Emby homevideos feed view; Grouped Music album-track and artist-track Workspace rows; TV series and episodes; Feeds entries; Audiobookshelf Podcast episode rows; and Audiobookshelf Book titles and chapter/audio-part rows. The Grouped Music artist/album browser SHALL be owned by its shallow tree and is not a canonical media-row flow.

Parent destinations SHALL retain Service content, stable-target-to-domain lookup, active pane and component focus, section/group/filter/bucket/season/scope chrome, Workspaces, loading, images, effects, persistence, and provider-specific typed intent translation. They SHALL NOT retain a second cursor, scroll, membership or range state, hit map, or authoritative selected-row identity for either a canonical flow or the Grouped Music tree.

#### Scenario: A list-local behavior has one implementation site
- **WHEN** a developer adds a purely canonical-list-local state transition and visual decoration
- **THEN** the production change is confined to the shared canonical media-list state/behavior owner and shared row painter
- **AND** no destination production file changes
- **AND** browser and provider-Workspace canonical media rows receive the behavior through their existing composition

#### Scenario: Responsive presentation does not synchronize local state
- **WHEN** one logical canonical list changes between Wide and non-Wide geometry
- **THEN** the same shared owner and fixed-row presentation remain active
- **AND** no cursor, scroll, membership, range, or other row-local state is copied between controls

#### Scenario: Parent authority remains outside the row owner
- **WHEN** a destination changes a section, group, filter, surname bucket, season, queue scope, focused pane, or Library Hero overlay state
- **THEN** the destination or owning Panel remains authoritative for that chrome, Workspace, or overlay state
- **AND** it projects the resulting media rows into the shared owner, or the settled Grouped Music catalog into the tree owner
- **AND** provider-specific effects remain typed destination intents

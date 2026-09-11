## MODIFIED Requirements

### Requirement: InlineMediaBrowser owns selected-row replacement

`InlineMediaBrowser<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a shared canonical media-list owner. It SHALL own one-column placement, selection visibility, variable-height selected-row replacement admission, ordinary-row fallback when replacement cannot fit, semantic painting delegation, and internal current-frame row and replacement geometry. Cursor, scroll, selected target, and other row-local state SHALL remain in the same logical owner used by the corresponding Wide presentation.

The owning panel (the Library panel or the Queue panel) SHALL retain framing and establish current claim and row-flow rectangles through its arrangement, and SHALL select which presentation of the shared owner is active from its own breakpoint. The presentation's `Component::view` SHALL paint the established row flow once and SHALL be its only ordinary-row painting entry point for a frame. Before that call, the owning panel MAY supply only a closed semantic policy for focused/selected treatment and desired detail admission; the policy SHALL contain no rectangle, raw style, callback, provider data, or effect.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected target/selected-row rectangle, admitted detail rectangle, and point-resolution facts, and expose no mutable row map or `RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve ordinary and replacement targets from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point and expose no detail rectangle.

It SHALL remain distinct from Inline Search and SHALL NOT become a second mounted identity, subscription, focus target, gesture recognizer, or router.

#### Scenario: A selected row is replaced

- **WHEN** the selected item fits the Inline presentation
- **THEN** its ordinary row is replaced once by the detail block
- **AND** there is no blank duplicate row and the shared target remains stable.

#### Scenario: Grouped Music consumes one admitted detail result

- **WHEN** the Narrow library panel paints Grouped Music's Inline album list with a selected album
- **THEN** its Inline presentation admits or falls back from detail within one view over the shared owner
- **AND** the Library panel reads only the current retained admitted-detail rectangle before painting the one inline hero form into it
- **AND** neither the panel nor Grouped Music reruns list layout, reconstructs selectable-row geometry, or transfers interaction state from another presentation
- **AND** Grouped Music paints no detail of its own.

#### Scenario: Inline and Wide read one owner

- **WHEN** another logical list changes between Inline and Wide presentation
- **THEN** both presentations read the same cursor, scroll, selected target, and row-local state
- **AND** no presentation-to-presentation interaction-state transfer occurs.

#### Scenario: An Inline result expires before another view

- **WHEN** an Inline presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** its prior point claim and admitted-detail rectangle are unavailable
- **AND** a parent treats the presentation as having no list target or detail region until the current view finishes.

### Requirement: Responsive handoff preserves an explicit anchor

A Wide or Inline presentation change for one logical list SHALL reuse its shared canonical owner. The outgoing presentation SHALL record the selected ordinary row's viewport offset, and the receiving presentation SHALL preserve that offset when possible and clamp it to its viewport otherwise. It SHALL NOT copy cursor, selected target, scroll, or other row-local state between presentation-specific controls. Ordinary refresh SHALL preserve the stable target and locally clamp. Only a discrete navigation or restoration boundary MAY explicitly re-anchor the shared owner from a shell-owned stable target and row offset.

#### Scenario: TV re-anchors across breakpoints

- **WHEN** TV changes Wide to Narrow and later returns to Wide
- **THEN** the same logical series owner preserves the selected stable target and row-local state
- **AND** the presentation handoff preserves or clamps only its viewport offset
- **AND** no shell cursor/scroll mirror is adopted.

### Requirement: Named destinations compose without changing provider authority

The slice SHALL compose the shared canonical media-list owner with applicable Wide and Inline presentations for generic Emby catalogs, Movies, the Emby homevideos feed view, and TV Series browsing. Every generic Emby catalog is hero-bearing; there is no non-hero two-column catalog presentation. Provider workspaces, images, effects, persistence, Service and Player authority, and typed message translation SHALL remain in their existing parents/shell; media-row interaction state and behavior SHALL remain in the shared owner.

#### Scenario: One painter is active

- **WHEN** a listed destination is rendered at its applicable presentation
- **THEN** exactly one shared media-list presentation paints its rows
- **AND** the old loop is not run as an underpaint or compatibility fallback.

#### Scenario: Two-column policy remains presentation-only

- **WHEN** a generic Emby catalog renders at any width
- **THEN** it renders through the Wide or Narrow library panel with a Wide or Inline presentation
- **AND** no two-column grid presentation renders.

### Requirement: One shared owner supports list-local extension

Every in-scope logical media-row flow SHALL have exactly one shared owner for row content order, selectable-target indexing, cursor, scroll, authoritative selected-row identity, row-local interaction state, and row-local behavior. Its Wide and Inline presentations SHALL operate on that same owner rather than synchronize independent copies. A purely list-local state transition and row decoration SHALL be implementable in the shared canonical media-list subsystem without changing destination production code.

The in-scope flows SHALL be Queue slots; Home rows; generic Emby catalog rows; Movies and the Emby homevideos feed view; Grouped Music albums and tracks; TV series and episodes; Feeds entries; Audiobookshelf Podcast shows and filtered episodes; and Audiobookshelf Book titles and chapter/audio-part rows.

Parent destinations SHALL retain Service content, stable-target-to-domain lookup, active pane and component focus, section/group/filter/bucket/season/scope chrome, detail workspaces, loading, images, effects, persistence, and provider-specific typed intent translation. They SHALL NOT retain a second row cursor, row scroll, row-local membership or range state, row hit map, or authoritative selected-row identity.

#### Scenario: A list-local behavior has one implementation site

- **WHEN** a developer adds a purely list-local state transition and visual decoration
- **THEN** the production change is confined to the shared canonical media-list state/behavior owner and shared row painter
- **AND** no destination production file changes
- **AND** Wide, Inline, and provider-workspace media rows receive the behavior through their existing composition.

#### Scenario: Responsive presentation does not synchronize local state

- **WHEN** one logical list changes between Wide and Inline presentation
- **THEN** the new presentation reads the same shared owner
- **AND** no cursor, scroll, membership, range, or other row-local state is copied between presentation-specific controls.

#### Scenario: Parent authority remains outside the row owner

- **WHEN** a destination changes a section, group, filter, surname bucket, season, queue scope, or focused pane
- **THEN** the destination remains authoritative for that chrome or workspace state
- **AND** it projects the resulting media rows into the shared owner
- **AND** provider-specific effects remain typed destination intents.

## REMOVED Requirements

### Requirement: Grid presentation shares canonical ownership

**Reason**: No library in use lacks a hero, so the non-hero two-column catalog and its Grid presentation are unreachable. Every library renders through the Library panel (`library-panel`).

**Migration**: Generic Emby catalogs render through the Wide or Narrow library panel with the Wide or Inline presentation of the same shared owner.

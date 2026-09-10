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

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a shared canonical media-list owner. It SHALL own fixed-height one-column row placement, semantic painting delegation, scrollbar presentation, viewport clamping, and internal current-frame row geometry, while cursor, scroll, selected target, and other row-local state remain in the one logical shared owner. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment and optional throbber; the policy SHALL contain no rectangle, raw style, callback, provider data, or effect.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected target/selected-row rectangle, and point-resolution facts, and expose no mutable row map or `RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve it from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point and expose no selected/detail geometry.

It SHALL support Wide hero rails, provider workspace rows, and Queue fixed rows, but SHALL NOT implement Inline replacement or Grid placement. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use the shared canonical owner with Wide presentation in every panel mode.

#### Scenario: Wide TV rail composes the control

- **WHEN** the TV surface is Wide hero
- **THEN** its series rail is painted by one Wide presentation over the shared owner
- **AND** the parent retains workspace, hero, images, chrome, and effects.

#### Scenario: Queue paints and resolves one current row flow

- **WHEN** Queue paints a non-empty fixed-row list in any panel mode
- **THEN** its Wide presentation receives equal established claim and row-flow rectangles through one component view and paints ordinary rows once
- **AND** Queue resolves a later row point to the `QueueSlotId` from that current retained result
- **AND** Queue does not rebuild row rectangles or a selectable row map.

#### Scenario: Grouped Music paints both Wide row flows

- **WHEN** Grouped Music paints its Wide album rail or Wide track table
- **THEN** each flow uses a Wide presentation over its shared owner
- **AND** Grouped Music resolves later row points from the current retained result without a cursor mirror or row map
- **AND** its existing panel framing and spacing are unchanged.

#### Scenario: Other provider workspaces paint fixed rows

- **WHEN** TV paints episodes, Audiobookshelf Podcast paints episodes, or Audiobookshelf Book paints chapter/audio-part rows
- **THEN** each flow uses a Wide presentation over its shared owner
- **AND** the destination resolves later row points from the current retained result without a cursor mirror or row map.

#### Scenario: A Wide result expires before another view

- **WHEN** a Wide presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the presentation as having no list target until the current view finishes.
### Requirement: InlineMediaBrowser owns selected-row replacement

`InlineMediaBrowser<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a shared canonical media-list owner. It SHALL own one-column placement, selection visibility, variable-height selected-row replacement admission, ordinary-row fallback when replacement cannot fit, semantic painting delegation, and internal current-frame row and replacement geometry. Cursor, scroll, selected target, and other row-local state SHALL remain in the same logical owner used by the corresponding Wide or Grid presentation.

The parent SHALL retain destination framing and establish current claim and row-flow rectangles through its arrangement. The presentation's `Component::view` SHALL paint the established row flow once and SHALL be its only ordinary-row painting entry point for a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment and desired detail admission; the policy SHALL contain no rectangle, raw style, callback, provider data, or effect.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected target/selected-row rectangle, admitted detail rectangle, and point-resolution facts, and expose no mutable row map or `RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve ordinary and replacement targets from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point and expose no detail rectangle.

It SHALL remain distinct from Inline Search and SHALL NOT become a second mounted identity, subscription, focus target, gesture recognizer, or router.

#### Scenario: A selected row is replaced

- **WHEN** the selected item fits the Inline presentation
- **THEN** its ordinary row is replaced once by the detail block
- **AND** there is no blank duplicate row and the shared target remains stable.

#### Scenario: Grouped Music consumes one admitted detail result

- **WHEN** Grouped Music paints its Inline album list with a selected album
- **THEN** its Inline presentation admits or falls back from detail within one view over the shared owner
- **AND** Grouped Music reads only the current retained admitted-detail rectangle before painting its provider-owned detail
- **AND** Grouped Music does not rerun list layout, reconstruct selectable-row geometry, or transfer interaction state from another presentation
- **AND** existing framing and spacing are unchanged.

#### Scenario: Inline and Wide read one owner

- **WHEN** another logical list changes between Inline and Wide presentation
- **THEN** both presentations read the same cursor, scroll, selected target, and row-local state
- **AND** no presentation-to-presentation interaction-state transfer occurs.

#### Scenario: An Inline result expires before another view

- **WHEN** an Inline presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** its prior point claim and admitted-detail rectangle are unavailable
- **AND** a parent treats the presentation as having no list target or detail region until the current view finishes.
### Requirement: Responsive handoff preserves an explicit anchor

A Wide, Inline, or Grid presentation change for one logical list SHALL reuse its shared canonical owner. The outgoing presentation SHALL record the selected ordinary row's viewport offset, and the receiving presentation SHALL preserve that offset when possible and clamp it to its viewport otherwise. It SHALL NOT copy cursor, selected target, scroll, or other row-local state between presentation-specific controls. Ordinary refresh SHALL preserve the stable target and locally clamp. Only a discrete navigation or restoration boundary MAY explicitly re-anchor the shared owner from a shell-owned stable target and row offset.

#### Scenario: TV re-anchors across breakpoints

- **WHEN** TV changes Wide to Normal and later returns to Wide
- **THEN** the same logical series owner preserves the selected stable target and row-local state
- **AND** the presentation handoff preserves or clamps only its viewport offset
- **AND** no shell cursor/scroll mirror is adopted.
### Requirement: Named destinations compose without changing provider authority

The slice SHALL compose the shared canonical media-list owner with applicable Wide and Inline presentations for hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, and TV Series browsing. Non-hero two-column Emby catalogs SHALL keep their existing two-column arrangement policy while composing the Grid presentation over the same shared owner. Provider workspaces, images, effects, persistence, Service and Player authority, and typed message translation SHALL remain in their existing parents/shell; media-row interaction state and behavior SHALL remain in the shared owner.

#### Scenario: One painter is active

- **WHEN** a listed destination is rendered at its applicable presentation
- **THEN** exactly one shared media-list presentation paints its rows
- **AND** the old loop is not run as an underpaint or compatibility fallback.

#### Scenario: Two-column policy remains presentation-only

- **WHEN** a generic Emby destination uses its non-hero two-column policy
- **THEN** Grid controls placement and current-frame geometry
- **AND** the same shared owner supplies its stable target and row-local behavior.
### Requirement: Migration is accepted as one verified slice
The implementation, representative stateful and rendered tests, automated gates, review, and acceptance SHALL form one uninterrupted slice. There SHALL be no pre-test visual-approval checkpoint. Affected surfaces SHALL provide metadata/state/image-bearing rendered evidence, stateful target-and-anchor evidence, source-level one-painter evidence, manual/live Wide/Narrow evidence before acceptance. The 800-line file-size gate SHALL be enforced as a pre-push check only and SHALL NOT gate acceptance of individual changes. A visual defect found during review or acceptance SHALL be treated as a bug, fixed, and followed by rerunning the affected tests and gates.

#### Scenario: Evidence precedes acceptance
- **WHEN** the implementation changes a visual surface
- **THEN** representative tests and automated gates run before review and acceptance
- **AND** live Wide/Narrow review remains part of acceptance
- **AND** any defect found there is fixed before the slice is accepted
### Requirement: Home composes canonical list controls

Home SHALL compose one shared canonical owner for the active section's flat media rows and use Inline or Wide presentation where the approved arrangement requires it. Section identity SHALL remain keyed by `pref_key` and restored through `restore_section`. Home SHALL keep exactly one active section; only that section's rows SHALL be projected into the shared owner. Ordinary refresh SHALL preserve its stable target and locally clamp. A presentation transition SHALL preserve the selected row's viewport offset without a second cursor, per-section cursor cache, or App-wide interaction mirror.

#### Scenario: Home refresh preserves section state

- **WHEN** the active Home section refreshes or its presentation changes
- **THEN** refresh preserves or clamps the shared owner’s stable target locally
- **AND** a presentation transition preserves or clamps the selected row offset
- **AND** `pref_key`/`restore_section`, images, and workspace effects remain shell/parent-owned.

#### Scenario: Home effect carries its target

- **WHEN** a Home row requests play, enqueue, delete, watched-toggle, or context intent
- **THEN** the typed request carries the component-resolved stable target
- **AND** the shell does not query Home's cursor or resolve the effect from a flat numeric row index.
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

Grouped Music album and track browsing, Audiobookshelf Podcast show and episode browsing, and Audiobookshelf Book and chapter/audio-part browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose shared Wide or Inline presentations where their arrangements require them. The controls SHALL remain embedded beneath the mounted destination component. Provider detail workspaces, images, selectors, surname buckets, filters, effects, and typed intent translation SHALL remain parent-owned; every media-row flow's cursor, scroll, authoritative selected target, row-local behavior, and retained row geometry SHALL remain in its shared canonical owner.

#### Scenario: Music groups retain provider authority

- **WHEN** a grouped Music album or track surface is rendered or navigated
- **THEN** album and track rows use shared canonical ownership and delegation
- **AND** grouping, active-pane focus, images, content lookup, and playback intents remain Music-owned
- **AND** no track cursor mirror, second list painter, or App-owned interaction mirror runs.

#### Scenario: Audiobookshelf shows compose without losing episodes

- **WHEN** a Podcast library is shown Wide or Normal
- **THEN** show and filtered episode rows use shared canonical ownership and delegation
- **AND** episode filtering, active-pane focus, images, content lookup, and typed playback intents remain Podcast-owned
- **AND** episode rows are not reseeded or reselected during painting.

#### Scenario: Audiobookshelf books compose without duplicate detail

- **WHEN** a Book library is shown Wide or Normal
- **THEN** book and chapter/audio-part rows use shared canonical ownership and delegation
- **AND** surname buckets, active-pane focus, images, content lookup, and absolute chapter-seek intents remain Book-owned
- **AND** chapter/audio-part rows are not reseeded or reselected during painting.
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
### Requirement: One shared owner supports list-local extension

Every in-scope logical media-row flow SHALL have exactly one shared owner for row content order, selectable-target indexing, cursor, scroll, authoritative selected-row identity, row-local interaction state, and row-local behavior. Its Wide, Inline, and Grid presentations SHALL operate on that same owner rather than synchronize independent copies. A purely list-local state transition and row decoration SHALL be implementable in the shared canonical media-list subsystem without changing destination production code.

The in-scope flows SHALL be Queue slots; Home rows; generic Emby catalog rows including non-hero two-column catalogs; Movies and the Emby homevideos feed view; Grouped Music albums and tracks; TV series and episodes; Feeds entries; Audiobookshelf Podcast shows and filtered episodes; and Audiobookshelf Book titles and chapter/audio-part rows.

Parent destinations SHALL retain Service content, stable-target-to-domain lookup, active pane and component focus, section/group/filter/bucket/season/scope chrome, detail workspaces, loading, images, effects, persistence, and provider-specific typed intent translation. They SHALL NOT retain a second row cursor, row scroll, row-local membership or range state, row hit map, or authoritative selected-row identity.

#### Scenario: A list-local behavior has one implementation site

- **WHEN** a developer adds a purely list-local state transition and visual decoration
- **THEN** the production change is confined to the shared canonical media-list state/behavior owner and shared row painter
- **AND** no destination production file changes
- **AND** Wide, Inline, Grid, and provider-workspace media rows receive the behavior through their existing composition.

#### Scenario: Responsive presentation does not synchronize local state

- **WHEN** one logical list changes between Wide, Inline, or Grid presentation
- **THEN** the new presentation reads the same shared owner
- **AND** no cursor, scroll, membership, range, or other row-local state is copied between presentation-specific controls.

#### Scenario: Parent authority remains outside the row owner

- **WHEN** a destination changes a section, group, filter, surname bucket, season, queue scope, or focused pane
- **THEN** the destination remains authoritative for that chrome or workspace state
- **AND** it projects the resulting media rows into the shared owner
- **AND** provider-specific effects remain typed destination intents.
### Requirement: Row-local input uses one delegation contract

After the mounted destination resolves overlay, chrome, and active-pane precedence, it SHALL offer every remaining eligible row-local key and normalized pointer gesture to one provider-neutral media-list delegation contract. The shared owner SHALL apply local state transitions and SHALL return only whether input was unhandled, consumed locally, changed the selected stable target, or requested a provider-neutral external row intent. A newly added purely local behavior SHALL use the existing consumed outcome and SHALL NOT require a new destination dispatch arm.

The mounted destination SHALL remain the sole TuiRealm event boundary and SHALL retain mouse gesture recognition. Embedded media-list presentations SHALL NOT be mounted, focused, subscribed, assigned a component identity, or made into another keyboard router.

#### Scenario: Local movement is delegated once

- **WHEN** an eligible movement, page, edge, or row-selection gesture reaches an active media-row flow
- **THEN** the shared delegation contract mutates the shared owner
- **AND** destination code does not call row cursor, scroll, or point-selection mutators directly.

#### Scenario: External row intent stays provider-specific

- **WHEN** shared row handling resolves activation or context intent for a stable target
- **THEN** the parent translates that provider-neutral result into its existing typed destination intent
- **AND** Service, Player, persistence, and effect authority do not enter the shared list.
### Requirement: Grid presentation shares canonical ownership

The preserved non-hero two-column Emby catalog SHALL use a Grid presentation over the same shared media-list owner and delegation contract as Wide and Inline presentations. Grid SHALL own two-column row placement, viewport clamping, scrollbar facts, and retained current-frame cell geometry without changing the established two-column arrangement policy.

#### Scenario: Generic catalog remains two columns

- **WHEN** a non-hero Emby catalog is rendered at a width that selects its established two-column presentation
- **THEN** it remains a two-column catalog
- **AND** its cursor, scroll, stable target, row-local behavior, and point resolution come from the shared owner and Grid presentation
- **AND** no parent cursor, scroll, selectable map, or cell hit map runs beside it.
### Requirement: Stable targets cross every canonical boundary

Every selectable media row SHALL use a stable opaque target whose identity survives reorder and ordinary refresh. A destination request caused by a row SHALL carry the component-resolved stable target rather than a cursor, display-row index, or provider-relative index for shell re-resolution. Targets that are only unique within a parent SHALL include that parent identity.

#### Scenario: Refresh reorders selected rows

- **WHEN** ordinary refresh reorders rows while the selected target remains present
- **THEN** the shared owner preserves that target and its row-local state
- **AND** no destination translates the old numeric position into the new order.

#### Scenario: Nested row intent crosses the shell boundary

- **WHEN** an episode, chapter, audio part, track, or Home row requests an external effect
- **THEN** the typed request carries its stable opaque target
- **AND** the shell does not query component cursor state or recompute the target from a row index.
### Requirement: Canonical geometry has no compatibility path

Every in-scope media-row presentation SHALL paint through its shared presentation adapter once and retain the completed current frame's read-only claim, content, selected-row, and point-resolution facts. Configuring content or geometry, beginning view, or viewing an empty or zero-area region SHALL invalidate prior facts. Destination parents SHALL NOT receive or reconstruct mutable row maps, selectable maps, row rectangles, or caller-supplied point-resolution geometry.

#### Scenario: Workspace row painting completes once

- **WHEN** a Music track, TV episode, Podcast episode, or Book chapter/audio-part flow paints
- **THEN** its shared presentation adapter paints the media rows once
- **AND** content and selection are not reseeded during painting
- **AND** a later point resolves only through retained current-frame geometry.

#### Scenario: Stale geometry cannot claim input

- **WHEN** a presentation is configured for a new frame but has not completed its current view
- **THEN** it claims no row point
- **AND** the parent has no compatibility fallback using prior or reconstructed geometry.

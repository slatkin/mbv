# canonical-media-lists Specification

## Purpose

Provide reusable embedded TuiRealm list controls with one owner for list interaction and geometry across the first canonical media-list migration slice.

## Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, an optional secondary title (the container name of a split row, painted after the primary text), an optional left-aligned trailing slot whose text role is closed — a release year paints in the green (`STATUS_AVAILABLE`) metadata role, a progress badge in the FOAM metadata role — a media kind (`Collection` for navigable containers, `Media` for playable leaves), an optional duration string (painted right-aligned in its own (`DURATION`) role, distinct from the
green year role), and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. Only the Queue list SHALL project a duration string; a library browse list row (Home, Podcast, TV, music, book, feeds) SHALL NOT carry one. A Heading SHALL paint its label bold in the FOAM metadata role, so every grouped list's group label reads the same. Heading and Spacer SHALL be excluded from selectable-target indexing. When a duration is shown it SHALL use the precise `M:SS`/`H:MM:SS` form (queue format, e.g. `4:32`, `1:02:03`); `Collection` rows SHALL NOT carry a duration. `Active` SHALL keep its existing meaning of stored resume progress rendered inline as trailing metadata; `NowPlaying` SHALL mean live playback, rendering its live progress inline as trailing metadata and its total duration like every other row — no throbber glyph appears in any row. On Wide lists, `NowPlaying` rows SHALL be marked by an aqua right-pointing play glyph before the title (one space from it) rather than an accent-coloured title; their title text SHALL keep the ordinary colour. A `NowPlaying` row SHALL NOT carry a secondary title, so the split-row palette below never applies to one. `Active` rows SHALL not be marked by the play glyph. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

A row carrying a secondary title SHALL paint as a split row: the primary text (the container/context name) SHALL paint in the playback-context gold (`PLAYBACK_CONTEXT_FG`) role and the secondary title (the item's own name) SHALL paint in the split-row sage (`SPLIT_ROW_TITLE_FG`) role, with one separating space. A row with no secondary title SHALL keep the ordinary title role for its primary text. A played split row SHALL mute the item title to the played (`TEXT_MUTED`) role while the context part keeps the playback-context gold role. No other semantic state SHALL move the split-row palette, and a `NowPlaying` row SHALL NOT carry a secondary title, so the palette never overrides the now-playing row's ordinary-colour title. A split row truncates as one string: its parts SHALL be cut as a unit with at most one trailing ellipsis, the context part keeping its full width and the item title absorbing the cut.

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
- **WHEN** the Queue list shows a duration
- **THEN** every row uses the same `M:SS`/`H:MM:SS` format
- **AND** the duration paints in its own (`DURATION`) role, not the green metadata role
- **AND** imprecise forms (`4m`, `1h12m`, unbounded `62:03`) never appear in list rows

#### Scenario: Library list rows carry no duration
- **WHEN** a library list row is projected (Home, Podcast, TV episode, music track, book chapter, feed entry)
- **THEN** it carries no duration string
- **AND** its row paints no right-aligned time slot

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string
- **AND** the painter suppresses the duration slot even if one is projected

#### Scenario: Split rows share the now-playing palette
- **WHEN** a row carries both a primary text and a secondary title
- **THEN** the primary text paints in the playback-context gold role and the secondary title paints in the split-row sage role
- **AND** the treatment is identical on every list that projects a split row, with no per-list opt-in
- **AND** the now-playing playback strip's title roles are unaffected

#### Scenario: Single-part rows keep the ordinary title role
- **WHEN** a row carries no secondary title
- **THEN** its primary text paints in the ordinary title role for its semantic state, not the split-row palette

#### Scenario: Played split rows mute the item title only
- **WHEN** a played row carries a secondary title
- **THEN** the secondary title paints in the played muted role
- **AND** the primary context text keeps the playback-context gold role

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for a shared canonical media-list owner. It SHALL own fixed-height one-column row placement, semantic painting delegation, scrollbar presentation, viewport clamping, and internal current-frame row geometry, while cursor, scroll, selected target, and other row-local state remain in the one logical shared owner. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no rectangle, callback, provider data, or effect. Every fixed-row presentation SHALL render its selected row with the selected-row bar: while the list holds focus the whole row paints the bar fill across its full width, overriding its zebra stripe, its two-column gutters, and any owning-surface identity, while every span keeps the ordinary unselected foreground role, so no title is bold and no accent title colour is introduced. There SHALL be no per-list opt-in or opt-out, and no marker glyph. An unfocused list SHALL paint no bar for its cursor row. A multi-selected row SHALL paint the bar too, including while the list is unfocused.

The policy's owning-surface identity SHALL NOT move the selected row's fill: every selected row paints the bar whatever surface owns it. A list that paints a scrollbar column SHALL keep the parent background there — the bar reaches the panel edge on its own row only.

The presentation SHALL retain the current frame's read-only claim/content rectangles, selected target/selected-row rectangle, and point-resolution facts, and expose no mutable row map or `RowGeometry` to a parent. A point-resolution call after view SHALL accept only the point and resolve it from retained geometry. Configuring the presentation, beginning view, or viewing an empty/zero-area rectangle SHALL invalidate a prior result; before the current view completes, it SHALL claim no point and expose no selected geometry.

It SHALL support Wide Hero browser rails, non-Wide Library browser lists, provider Workspace rows, and Queue fixed rows, but SHALL NOT implement selected-row replacement or Grid placement. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use the shared canonical owner with this presentation in every Panel mode.

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

Grouped Music album and track browsing, Audiobookshelf Podcast episode browsing, and Audiobookshelf Book and chapter/audio-part browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose shared Wide or Inline presentations where their arrangements require them. The controls SHALL remain embedded beneath the mounted destination component. Provider detail workspaces, images, selectors, surname buckets, filters, effects, and typed intent translation SHALL remain parent-owned; every media-row flow's cursor, scroll, authoritative selected target, row-local behavior, and retained row geometry SHALL remain in its shared canonical owner.

#### Scenario: Music groups retain provider authority

- **WHEN** a grouped Music album or track surface is rendered or navigated
- **THEN** album and track rows use shared canonical ownership and delegation
- **AND** grouping, active-pane focus, images, content lookup, and playback intents remain Music-owned
- **AND** no track cursor mirror, second list painter, or App-owned interaction mirror runs.

#### Scenario: Audiobookshelf shows compose without losing episodes

- **WHEN** an Audiobookshelf podcast library is shown Wide or Normal
- **THEN** episode rows use shared canonical ownership and delegation
- **AND** the pill row's selection and filtering, active-pane focus, images, content lookup, and typed playback intents remain Audiobookshelf-podcast-owned
- **AND** episode rows are not reseeded or reselected during painting.

#### Scenario: Audiobookshelf books compose without duplicate detail

- **WHEN** a Book library is shown Wide or Normal
- **THEN** book and chapter/audio-part rows use shared canonical ownership and delegation
- **AND** surname buckets, active-pane focus, images, content lookup, and absolute chapter-seek intents remain Book-owned
- **AND** chapter/audio-part rows are not reseeded or reselected during painting.

### Requirement: Audiobookshelf geometry has complete breakpoint fallbacks

Audiobookshelf Book surfaces SHALL use the shared Wide hero or Inline arrangement at the established Wide/Normal breakpoints. Audiobookshelf Podcast surfaces SHALL use the shared Wide hero when it fits and their ordinary episode rows at every other geometry, because the podcast tab has no Inline hero. Both SHALL preserve the short-height fallback and hand off stable selected target and viewport anchor across breakpoint changes. Non-list repairs required to make the composition correct SHALL live in shared arrangements or the owning destination component, not a bespoke exception.

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

Every in-scope logical media-row flow SHALL have exactly one shared owner for row content order, selectable-target indexing, cursor, scroll, authoritative selected-row identity, row-local interaction state, and row-local behavior. Its fixed-row presentation SHALL operate on that owner in Wide and non-Wide geometry rather than synchronize independent copies. A purely list-local state transition and row decoration SHALL be implementable in the shared canonical media-list subsystem without changing destination production code.

The in-scope flows SHALL be Queue slots; Home rows; generic Emby catalog rows; Movies and the Emby homevideos feed view; Grouped Music albums and tracks; TV series and episodes; Feeds entries; Audiobookshelf Podcast episode rows; and Audiobookshelf Book titles and chapter/audio-part rows.

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

### Requirement: Row-local input uses one delegation contract

After the mounted destination resolves overlay, chrome, and active-pane precedence, it SHALL offer every remaining eligible row-local key and normalized pointer gesture to one provider-neutral media-list delegation contract. Pointer input SHALL be resolved through current painted geometry to a stable target before the shared owner applies the corresponding target-bearing operation; keyboard and pointer delivery SHALL remain distinct before this resolution and SHALL converge on the same row-local state operations afterward.

The shared owner SHALL return one transition whose independent facts describe whether the input was unhandled or consumed, whether the selected stable target changed, whether a multi-selection presentation summary changed, and whether a provider-neutral external row intent was requested. A transition MAY report more than one fact for the same operation. A newly added purely local behavior SHALL use the consumed disposition and SHALL NOT require a new destination dispatch arm or a shell mirror of list-local state.

The mounted destination SHALL remain the sole TuiRealm event boundary and SHALL retain mouse gesture recognition. Embedded media-list presentations SHALL NOT be mounted, focused, subscribed, assigned a component identity, or made into another keyboard router. The shared owner SHALL receive no raw terminal event and SHALL NOT resolve screen coordinates supplied without the presentation's stable target result.

#### Scenario: Local movement is delegated once

- **WHEN** an eligible movement, page, edge, or row-selection operation reaches an active media-row flow
- **THEN** the shared delegation contract mutates the shared owner
- **AND** destination code does not call row cursor, scroll, membership, anchor, or point-selection mutators directly
- **AND** the transition independently reports every resulting target or summary change

#### Scenario: One operation has several consequences

- **WHEN** one row-local operation consumes input, changes the selected target or multi-selection, and requests an external intent
- **THEN** one transition reports all applicable facts
- **AND** destination code does not reproduce the operation by calling delegation more than once or sequencing private mutators

#### Scenario: External row intent stays provider-specific

- **WHEN** shared row handling resolves activation or context intent for one or more stable targets
- **THEN** the parent translates that provider-neutral intent into its typed destination intent
- **AND** Service, Player, persistence, target materialization, and effect authority do not enter the shared list

#### Scenario: Keyboard and pointer select through one state transition

- **WHEN** a keyboard Visual operation and a modifier-click express the same selection operation
- **THEN** both apply the same shared-owner transition after pointer geometry has been resolved to a stable target
- **AND** their resulting membership, anchor, selected target, and presentation summary are identical

#### Scenario: Pointer resolution remains geometry-owned

- **WHEN** a normalized pointer gesture lands on a canonical media row
- **THEN** the active presentation resolves the point from its completed current-frame geometry before delegation
- **AND** the shared owner receives the stable target rather than raw coordinates for later re-resolution

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

- **WHEN** a Music track, TV episode, Audiobookshelf podcast episode, or Book chapter/audio-part flow paints
- **THEN** its shared presentation adapter paints the media rows once
- **AND** content and selection are not reseeded during painting
- **AND** a later point resolves only through retained current-frame geometry.

#### Scenario: Stale geometry cannot claim input

- **WHEN** a presentation is configured for a new frame but has not completed its current view
- **THEN** it claims no row point
- **AND** the parent has no compatibility fallback using prior or reconstructed geometry.

### Requirement: Selection summaries do not duplicate list authority

A canonical media list SHALL remain the sole owner of its multi-selection membership and anchor. When those values change, its transition MAY expose a read-only presentation summary containing only facts needed by another surface, such as selected count and originating-list identity. The summary SHALL NOT contain a writable membership copy, SHALL NOT be accepted as an ordinary content projection, and SHALL NOT be used to reconstruct or overwrite selection.

Library and Queue lists MAY retain independent multi-selections simultaneously. Panel focus SHALL determine which list's summary appears in the status presentation and which list receives keyboard input; changing panel focus SHALL NOT clear either list. A selection action SHALL clear only its originating list.

#### Scenario: Library and Queue selections survive focus changes

- **WHEN** Library and Queue each hold a multi-selection and panel focus moves between them
- **THEN** neither shared owner loses membership or anchor state
- **AND** the status presentation changes to the newly focused list's summary

#### Scenario: Destination switch remains list-local

- **WHEN** the active Library destination changes while Queue also holds a multi-selection
- **THEN** the departing Library destination follows its defined selection-retention or clearing policy
- **AND** Queue's selection is unchanged

#### Scenario: Summary cannot restore selection

- **WHEN** a list clears or prunes selected targets after content changes
- **THEN** a previously published summary cannot restore those targets
- **AND** the next summary reflects the owner's resulting state

### Requirement: Selection intents preserve list order and origin

A context intent over a multi-selection SHALL carry the selected stable targets in canonical list order and a stable identity for the originating list. The destination SHALL resolve those targets against one coherent owned content snapshot and translate them to effect-ready domain values. Generic list code SHALL NOT retain provider objects or destination lookup callbacks.

Missing targets SHALL be handled explicitly and SHALL NOT be replaced by the current cursor or silently resolved from a different snapshot. Context-menu lifetime and focus changes SHALL NOT change the origin or ordered target snapshot used by the action.

#### Scenario: Context selection resolves in list order

- **WHEN** targets were selected in an order different from their displayed order
- **THEN** the context intent carries them in canonical list order
- **AND** the destination resolves them against one content snapshot in that order

#### Scenario: Focus changes while menu is open

- **WHEN** a menu opened from Queue while Library also retains a selection
- **THEN** the menu action remains addressed to Queue and its captured ordered targets
- **AND** completing the action clears Queue's selection without clearing Library's selection

#### Scenario: Selected target disappears before resolution

- **WHEN** an emitted target is absent from the destination snapshot used to materialize the action
- **THEN** the destination reports or deliberately rejects the missing target according to the action contract
- **AND** it does not substitute the cursor target or another occurrence

### Requirement: Selected row marquees an overflowing title instead of truncating

When the shared row painter renders the row that is both the list's current selection and on a focused list, and that row's title does not fit its title slot, the painter SHALL animate the title through a bounded back-and-forth marquee (hold at the start, scroll to reveal the tail, hold at the end, scroll back) rather than ellipsis-truncating it. A list that declares its rows reveal their title only on selection SHALL marquee that selected row's title whether or not it fits its slot, carrying the title fully out of the window and back instead of holding a fitting title static. Every other row — unselected rows, the selected row on an unfocused list, and any row whose title already fits its slot in a list that reveals titles on every row — SHALL continue to render with ellipsis truncation exactly as before this change. The marquee's timing SHALL match the existing player-strip title marquee's cadence (used for the Now Playing and idle-feed titles), so the two forms of marqueeing feel identical to the user.

Each list owns an independent marquee clock (mirroring its ownership of cursor and scroll). The clock SHALL key on the full text it marquees — both parts of a split row's title, not the primary text alone — and SHALL restart from the beginning whenever that text changes: a new row becomes selected, or the selected row's own title text changes. A freshly-marqueed title therefore never opens mid-scroll, and two rows that share a primary text never share a clock position.

#### Scenario: Focused selected row with an overflowing title marquees

- **WHEN** the row under the cursor on a focused list has a title wider than its available slot
- **THEN** the row's title animates through the hold/scroll/hold/scroll-back marquee cycle
- **AND** no ellipsis appears in the row while it is marqueeing

#### Scenario: Unfocused selection keeps truncating

- **WHEN** the row under the cursor has an overflowing title but its list is not focused
- **THEN** the row's title is ellipsis-truncated exactly as an ordinary row

#### Scenario: Non-selected rows keep truncating

- **WHEN** a row is not the list's current selection
- **THEN** its overflowing title is ellipsis-truncated regardless of focus

#### Scenario: Fitting title never marquees

- **WHEN** the selected, focused row's title already fits its slot and its list reveals titles on every row
- **THEN** it renders in full, static, with no marquee and no ellipsis

#### Scenario: A fitting title in a reveal-on-selection list marquees

- **WHEN** the selected, focused row's title already fits its slot and its list reveals titles only on selection
- **THEN** the title marquees through the same cycle, scrolling fully out of the window and back
- **AND** it does not sit static or ellipsis-truncated

#### Scenario: Marquee restarts on a new selection

- **WHEN** the cursor moves to a different row — including one whose context text matches the previous row's — or the selected row's title text itself changes
- **THEN** that row's marquee begins again from its held starting position rather than resuming mid-cycle

### Requirement: Library Wide lists stripe with their panel's contrasting pair

Both library Wide lists SHALL paint zebra striping with the semantics the Queue list already ships:
a group-position alternation that restarts at each `Heading`, so every group header carries the
secondary fill and its group's rows alternate from there, the `Spacer` row above a header keeping the
surface fill outside the sequence, and the selected row painting
the bar over its parity. Each arm SHALL resolve its secondary pair from the surface
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
- **THEN** the 2nd, 4th, and 6th visible rows carry the `MainContentBox` focused fill
- **AND** the remaining visible rows carry no secondary background

#### Scenario: Workspace list stripes with the library-panel pair

- **WHEN** a provider Workspace Wide list renders focused
- **THEN** the striped positions carry the `LibraryPanel` focused fill
- **AND** that fill differs from the `MainContentBox` fill the Workspace box is painted with

#### Scenario: A stripe never equals its own panel body

- **WHEN** either library Wide arm renders in either focus state
- **THEN** its stripe colour differs from the fill its own list box is painted with in that state

#### Scenario: Library group headings keep the surface fill

- **WHEN** a grouping `Heading` row appears between two library items
- **THEN** it keeps the surface fill whatever the screen row it lands on
- **AND** the group's first member below it carries the secondary fill, whatever fill the group above it ended on

#### Scenario: A `Spacer` row stays on the surface fill

- **WHEN** the `Spacer` row that separates two library groups is visible
- **THEN** it carries the surface fill wherever it sits in the sequence
- **AND** the group below it starts from its own `Heading`, unaffected by the rows above

### Requirement: Wide presentation zebra striping

The Wide presentation SHALL accept an optional zebra-stripe policy on its paint policy. The policy
SHALL carry a focused and an unfocused secondary background colour. When zebra striping is enabled,
the painter SHALL keep grouping `Heading` rows and `Spacer` rows on the surface fill, outside the
sequence — consuming no position, so each group opens its own alternation regardless of the rows above.
A group's member rows SHALL alternate from the secondary fill at the group's first member: the first
member carries the secondary background colour and every second member after it reverts to the surface
fill, whatever the membership count. A row with
no `Heading` above it counts from the list's first row, which is therefore unstriped. When zebra striping is
disabled (the default), row backgrounds SHALL be unchanged from today's behaviour. Because every Wide
list marks its selection with the selected-row bar rather than with its own row fill (see
*WideMediaList owns fixed-row mechanics*), the selected row SHALL NOT stripe: the bar replaces the
zebra background on the selected row while every other row keeps its own parity. For an unselected striped row, the secondary background SHALL
be confined to the text-flow range inside the row's existing two-column left and right gutters; the
parent background SHALL remain visible in those gutters and in any scrollbar column. Row geometry,
width calculations, and hit geometry SHALL remain unchanged.

#### Scenario: Zebra stripes are contained within existing row gutters

- **WHEN** a Wide presentation renders an unselected striped row
- **THEN** its secondary background spans only the text-flow range inside the existing two-column
  gutters
- **AND** the gutters and scrollbar column retain the parent background without changing row geometry

#### Scenario: Zebra stripes alternate among visible rows

- **WHEN** a Wide presentation has zebra striping enabled and renders five visible rows with no
  `Heading` above them, the window opening at the list's first row
- **THEN** the 2nd and 4th visible rows paint with the secondary background colour matching the
  current focus state
- **AND** the 1st, 3rd, and 5th visible rows paint with no secondary background

#### Scenario: A group boundary restarts the sequence

- **WHEN** a grouping `Heading` row sits between two `Item` rows
- **THEN** the `Heading` keeps the surface fill
- **AND** the group's first member below it carries the secondary background colour, with every second
  member after it reverting to the surface fill, whatever position the `Item` above the
  `Heading` took

#### Scenario: Grouping rows never stripe

- **WHEN** a grouping `Heading` or separating `Spacer` row is visible
- **THEN** it keeps the surface fill in every position
- **AND** the two-column gutters and any scrollbar column keep the parent background

#### Scenario: Selected row paints the bar over its stripe

- **WHEN** the selected row falls on a zebra-striped position on a focused Wide list
- **THEN** the row paints the bar fill instead of the zebra background
- **AND** its spans keep the ordinary foreground roles

#### Scenario: Zebra follows the row's group position

- **WHEN** the list scrolls by one row within a group
- **THEN** each row keeps the stripe its position in its group resolves to, regardless of the screen
  row it lands on
- **AND** a window that opens mid-group keeps the group's pattern

#### Scenario: Zebra is disabled by default

- **WHEN** a Wide presentation is configured without a zebra-stripe policy
- **THEN** all unselected rows paint with no explicit background, matching today's behaviour

### Requirement: The non-Wide library list is the Wide Browser-pane list

The non-Wide library list SHALL be the Wide Browser-pane list at the non-Wide pane width: it SHALL
resolve the same `LibraryPanel` list-box fill and the same `MainContentBox` stripe pair, and SHALL
carry no per-geometry body fill or scrollbar-column fill. Its list box SHALL be filled by the panel
that composes the surface, as the Wide Browser-pane list's is, so the Wide list policy is the only
one the non-Wide library uses. A list that carries no body fill SHALL keep the parent background in
its scrollbar column; the selected row's bar paints only its own row, so the Queue is unaffected.

#### Scenario: The non-Wide library list uses the Wide Browser-pane policy

- **WHEN** a non-Wide library list renders in either focus state
- **THEN** its list box carries the `LibraryPanel` fill and its alternating rows carry the
  `MainContentBox` fill for that state
- **AND** no body fill or scrollbar-column fill is set on its paint policy

#### Scenario: Scrollbar column parity with the Wide Browser-pane list

- **WHEN** a non-Wide library list overflows and paints its scrollbar column
- **THEN** the column carries the same fill the Wide Browser-pane list's column carries
- **AND** it does not carry a library-body fill

#### Scenario: Lists without their own body fill keep the parent background

- **WHEN** a Wide library list or the Queue list paints its scrollbar column
- **THEN** the column keeps the parent background
- **AND** the selected row's bar does not bleed down the column

### Requirement: Inline Search results compose the shared canonical media-list owner

An open Inline Search session SHALL present its scored result rows through the shared canonical media-list owner, embedded inside the Inline Search control, rendered by the one canonical fixed-row presentation in the Library panel's list box at every breakpoint — never through a bespoke search-row renderer. The search control SHALL retain what is search-specific (query, candidate pool, scoring, debounce, open/dismiss, typed activation intents); cursor, scroll, selected stable target, viewport clamping, and retained row geometry SHALL remain in the shared canonical owner, exactly as for every other list.

Result rows SHALL be selectable `Item` rows with stable opaque targets (the matched items' ids). The search flow SHALL project no `Heading` or `Spacer` rows: its result set is flat by construction. The search bar itself SHALL remain the Library panel's selector-row chrome outside the canonical row flow, replacing the pill row exactly as before.

Selection SHALL reset to the first result whenever the scored results re-fire for a changed query. A corpus or pool refresh with an unchanged query SHALL preserve the selected stable target through the shared owner's ordinary refresh rule. A responsive presentation transition SHALL keep the session open, reuse the same shared owner, and clamp the viewport, per the shared owner's geometry-change rule.

Pointer input against search result rows SHALL flow through the panel's normalized media-list surface path like every other list: a click selects the row, a double-click activates it, a wheel gesture moves the local cursor, and a right-click resolves the row's ordinary item-based context-menu intent. The search results SHALL NOT carry a private raw-event mouse path.

#### Scenario: Search results render through the fixed-row presentation

- **WHEN** Inline Search is open with scored results in either Wide or Narrow presentation
- **THEN** the result rows SHALL paint through the same canonical fixed-row presentation as every other library list
- **AND** the selected row SHALL paint the selected-row bar while the list holds focus

#### Scenario: Query change resets the selection

- **WHEN** the scored results re-fire for a changed query
- **THEN** the canonical owner's selection SHALL reset to the first result row
- **AND** the viewport SHALL rest at the top

#### Scenario: Corpus arrival preserves the selected result

- **WHEN** the corpus fetch completes or the pool is re-projected while the query is unchanged
- **THEN** the selected stable target SHALL remain selected when it is still present in the new rows
- **AND** the selection SHALL clamp to the first valid result otherwise

#### Scenario: Presentation transition reuses the shared owner

- **WHEN** an open Inline Search session crosses a Wide/Narrow presentation transition
- **THEN** the same shared canonical owner SHALL remain active with its selection preserved and its viewport clamped
- **AND** no row-local state SHALL be copied into a second control

#### Scenario: Right-click on a search result row

- **WHEN** the user right-clicks a painted search result row
- **THEN** the row's ordinary item-based context-menu intent SHALL resolve through the standard media-list row-intent path

### Requirement: The window raise keeps a group's label visible

When the shared media-list owner raises the visible window to bring the selection back into view,
it SHALL continue raising over the contiguous non-selectable rows (`Heading`/`Spacer`) directly
above the selection, so the `Heading` that labels the group containing the selection stays painted.
The raise SHALL NOT continue past the first selectable row above that label run. No other window
behaviour SHALL change: the wheel, `PgUp`/`PgDn`, cursor chords, restore, and clamping keep their
current meaning, and the window has no writer besides the existing selection-following rule.

#### Scenario: Scrolling back up re-shows the group heading

- **WHEN** a grouped list is scrolled down and the cursor is moved back up to a group's first
  selectable row, entering the window from above
- **THEN** the window's first row is that group's `Heading`
- **AND** the selection is painted directly below it

#### Scenario: The raise shows only the selection's own group label

- **WHEN** the raise walks the rows above the selection
- **THEN** it stops at the first selectable row above that label run
- **AND** the previous group's rows stay outside the window

#### Scenario: A raise with no label above is unchanged

- **WHEN** the selection enters the window from above and the row above it is selectable
- **THEN** the window top is the selection's display row, exactly as before this change

### Requirement: Lists may reveal their rows' titles on selection

The shared list owner SHALL accept a closed title-reveal policy for its rows, defaulting to every row painting its full title. When a list declares the reveal-on-selection policy:

- A row that is not part of the list's current selection SHALL paint only its primary (context) text. No fragment of the secondary title SHALL appear in that row, and the row SHALL NOT reserve the hidden title's width.
- A row that is part of the list's current selection — the cursor row, or a multi-selected row — SHALL paint its full context-and-title text.
- The reveal on its own SHALL NOT animate anything: a selected row's title marquees exactly according to the selected-row marquee requirement, which this policy extends.

The policy SHALL be declared once by the destination that composes the list and resolved at the shared row-paint seam. No destination SHALL paint its own rows, fork the row painter, or pass a per-row flag to express the policy, and no list that never declares it SHALL change behaviour.

#### Scenario: Unselected rows hide the item title

- **WHEN** a reveal-on-selection list paints a row that is not part of its current selection
- **THEN** that row paints its primary context text only
- **AND** no part of the row's secondary title appears, with no ellipsis standing in for it

#### Scenario: The selected row reveals the item title

- **WHEN** a reveal-on-selection list paints its cursor row on an unfocused list
- **THEN** that row paints its full context-and-title text
- **AND** the title renders static, truncated at the row's title slot if it does not fit

#### Scenario: Lists that never declare the policy are unchanged

- **WHEN** a list composes rows without declaring a title-reveal policy
- **THEN** every row paints its full title exactly as before this change, including lists whose rows carry a secondary title

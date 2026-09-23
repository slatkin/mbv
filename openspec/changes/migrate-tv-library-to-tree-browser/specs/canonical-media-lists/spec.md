# Spec Delta

## ADDED Requirements

### Requirement: TV show modes browse through the tree list shape

TV show-browsing modes (`All` and the alphabet ranges) SHALL present their shows, seasons, and loaded episodes through one tree list shape over the shared list seam — the shared TreeBrowser specified by `shared-list-components` — and SHALL NOT project show rows into the fixed-row `WideMediaList` presentation. Their established show group headings and between-group spacers SHALL be projected as tree structural rows, never as selectable rows. `Latest`, `Upcoming`, the TV Hero's season pills, and the Hero's episode Workspace SHALL remain fixed-row flat lists through the `WideMediaList` presentation with their existing Hero, direct-play, and grouping rules unchanged; inline tree episodes deliberately duplicate Workspace episode rows. Each TV show-mode surface SHALL retain exactly one painter: the shared tree paints the show modes, and the shared fixed-row presentation paints the flat episode surfaces.

#### Scenario: A show mode paints through the tree

- **WHEN** a TV show mode (`All` or an alphabet range) is rendered in any Panel mode
- **THEN** its rows paint through the one shared tree list shape with group headings and spacers as structural rows
- **AND** no fixed-row presentation underpaints or falls back beneath it

#### Scenario: Flat episode surfaces stay flat

- **WHEN** `Latest` or `Upcoming` is rendered, or the TV Hero presents its season pills and episode Workspace
- **THEN** those rows remain flat fixed-row lists with their existing activation and Hero rules
- **AND** they acquire no show/season nesting or tree structural rows

#### Scenario: Inline and Workspace episode rows coexist

- **WHEN** a show and season are expanded inline to reveal an episode
- **THEN** that episode is also still available in the Hero's episode Workspace
- **AND** the two surfaces keep independent cursors and resolve actions from the cursor that produced the intent

## MODIFIED Requirements

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for the flat list shape. It SHALL own fixed-height one-column row placement, semantic painting delegation, and scrollbar presentation. Row flow, cursor, scroll, viewport clamping, selected target, retained current-frame geometry, and point resolution SHALL come from the shared list seam rather than from this presentation. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no rectangle, callback, provider data, or effect. Every fixed-row presentation SHALL render its selected row with the selected-row bar: while the list holds focus the whole row paints the bar fill across its full width, overriding its zebra stripe, its two-column gutters, and any owning-surface identity, while every span keeps the ordinary unselected foreground role, so no title is bold and no accent title colour is introduced. There SHALL be no per-list opt-in or opt-out, and no marker glyph. An unfocused list SHALL paint no bar for its cursor row. A multi-selected row SHALL paint the bar too, including while the list is unfocused.

The policy's owning-surface identity SHALL NOT move the selected row's fill: every selected row paints the bar whatever surface owns it. A list that paints a scrollbar column SHALL keep the parent background there — the bar reaches the panel edge on its own row only.

It SHALL support Wide Hero browser rails, non-Wide Library browser lists, provider Workspace rows, and Queue fixed rows for destinations whose flow is flat, but SHALL NOT implement selected-row replacement or Grid placement. A destination whose flow nests uses the tree list shape over the same seam instead; for TV, the show-browsing modes are such a nested flow, while `Latest`, `Upcoming`, and the Hero's episode Workspace remain flat flows of this presentation. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use this presentation in every Panel mode.

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

- **WHEN** TV renders one of its flat fixed-row surfaces (`Latest`, `Upcoming`, or the Hero's episode Workspace) in Wide or non-Wide geometry
- **THEN** that rail is painted by one fixed-row presentation over the shared seam
- **AND** the parent retains Workspace, Hero, images, chrome, overlay, and effects
- **AND** TV show-browsing modes instead paint through the tree list shape, not this presentation

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

- **WHEN** TV paints Hero Workspace episodes, Audiobookshelf Podcast paints episodes, or Audiobookshelf Book paints chapter or audio-part rows
- **THEN** each flow uses the fixed-row presentation over the shared seam
- **AND** the destination resolves later row points from the seam's retained result without a cursor mirror or row map

#### Scenario: A Wide result expires before another view

- **WHEN** a fixed-row presentation is configured for a new frame or receives an empty or zero-area view
- **THEN** the seam's retained geometry is invalidated, so its prior point claim and selected-row geometry are unavailable
- **AND** a parent treats the presentation as having no list target until the current view finishes

### Requirement: Named destinations compose without changing provider authority

The slice SHALL compose the shared canonical media-list owner with applicable Wide and Inline presentations for generic Emby catalogs, Movies, and the Emby homevideos feed view. TV's show-browsing modes SHALL compose the tree list shape over the same seam instead of this flat owner, and TV's flat episode surfaces (`Latest`, `Upcoming`, and the Hero's episode Workspace) SHALL compose the shared canonical media-list owner as before. Every generic Emby catalog is hero-bearing; there is no non-hero two-column catalog presentation. Provider workspaces, images, effects, persistence, Service and Player authority, and typed message translation SHALL remain in their existing parents/shell; media-row interaction state and behavior SHALL remain in the shared owner.

#### Scenario: One painter is active

- **WHEN** a listed destination is rendered at its applicable presentation
- **THEN** exactly one shared media-list presentation paints its rows
- **AND** the old loop is not run as an underpaint or compatibility fallback.

#### Scenario: Two-column policy remains presentation-only

- **WHEN** a generic Emby catalog renders at any width
- **THEN** it renders through the Wide or Narrow library panel with a Wide or Inline presentation
- **AND** no two-column grid presentation renders.

### Requirement: TV and Movies establish the Wide composition precedent

When grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide width and minimum-height predicate, it SHALL follow the TV/Movies composition: its provider-owned detail/workspace SHALL occupy the right pane, and its parent-owned browser-level pills followed by its one-column browser SHALL occupy the left rail. Grouped Music SHALL place its shallow artist/album tree in that browser slot, TV show-browsing modes SHALL place the shared show/season/episode tree there, and the other destinations SHALL place ordinary canonical rows there. The arrangement SHALL use the same shared predicate, pane framing, content spacing, and short-height fallback as TV/Movies. The Wide presentation SHALL NOT use an Inline hero or selected-row replacement in the left rail; when the shared predicate is not met, the destination SHALL use the shared Inline fallback (or suppress detail when the shared minimum cannot fit), not a bespoke arrangement. The arrangement mechanics of this precedent — shared predicate, pane framing, content spacing, and short-height fallback — are specified by the `right-panel-arrangements` spec; this requirement governs only how the browser control composes into that arrangement.

#### Scenario: Wide provider workspace and ordinary rail

- **WHEN** grouped Music or an Audiobookshelf Podcast or Book destination meets the shared Wide geometry conditions
- **THEN** its provider-owned detail/workspace is on the right
- **AND** its browser-level pills and one-column browser are on the left
- **AND** Grouped Music and TV show-browsing modes use the tree browser while the other destinations use canonical media rows
- **AND** no Wide Inline hero or selected-row replacement is painted in the left rail

#### Scenario: Shared predicate and fallback apply

- **WHEN** the destination crosses the shared width or minimum-height guard
- **THEN** it uses the same predicate, pane framing, content spacing, and short-height fallback as TV/Movies
- **AND** it does not introduce a destination-specific arrangement or breakpoint

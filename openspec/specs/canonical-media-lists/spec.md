# canonical-media-lists Specification

## Purpose

Provide reusable embedded TuiRealm list controls with one owner for list interaction and geometry across the first canonical media-list migration slice.

## Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, an optional
secondary title (the container name of a split row), an optional metadata slot, a media kind
(`Collection` for navigable containers, `Media` for playable leaves), an optional duration string,
and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`,
now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable
Heading and Spacer rows. Feed entries and `Collection` rows SHALL NOT carry a duration. Heading and
Spacer SHALL be excluded from selectable-target indexing. `Active` SHALL mean stored resume
progress; `NowPlaying` SHALL mean live playback with live progress. A now-playing row with unknown
runtime SHALL show no progress. Only selectable Item rows SHALL carry a release year or publish
date. The model SHALL contain no provider client, `App`, source/header, raw style, callback,
breakpoint, or effect.

#### Scenario: Queue-like progress is presented safely
- **WHEN** a parent supplies active progress
- **THEN** the control receives only a bounded percentage
- **AND** playback and queue authority remain with the parent/shell

#### Scenario: Now-playing with unknown runtime
- **WHEN** a now-playing row carries no progress
- **THEN** no percentage renders

#### Scenario: Structural rows are displayed only
- **WHEN** a Heading or Spacer is rendered
- **THEN** it occupies display geometry
- **AND** it cannot be selected or activated

#### Scenario: Library list rows carry no duration
- **WHEN** a library list row is projected (Home, Podcast, feed entry)
- **THEN** it carries no duration string

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string

#### Scenario: Only Item rows carry dates
- **WHEN** a Heading or Spacer row is rendered
- **THEN** it carries no release year or publish date

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm presentation adapter for the flat list shape. It SHALL own fixed-height one-column row placement and semantic painting delegation. Row flow, cursor, scroll, viewport clamping, selected target, retained current-frame geometry, and point resolution SHALL come from the shared list seam rather than from this presentation. The parent SHALL retain ownership of the destination panel/frame and establish its current claim and row-flow rectangles using its existing arrangement; before view, it SHALL configure those rectangles on the presentation.

The presentation's `Component::view` SHALL paint the established row flow once and SHALL be the only ordinary-row painting entry point for that presentation in a frame. Before that call, the parent MAY supply only a closed semantic policy for focused/selected treatment; the policy SHALL contain no rectangle, callback, provider data, or effect. Every fixed-row presentation SHALL visibly distinguish its selected row while the list holds focus, with no per-list opt-in or opt-out. An unfocused list SHALL NOT mark its cursor row as selected. A multi-selected row SHALL be visibly distinguished as selected, focused or not.

It SHALL support Wide Hero browser rails, non-Wide Library browser lists, provider Workspace rows, and Queue fixed rows for destinations whose flow is flat, but SHALL NOT implement selected-row replacement or Grid placement. A destination whose flow nests uses the tree list shape over the same seam instead; for TV, the show-browsing modes are such a nested flow, while `Latest`, `Upcoming`, and the Hero's episode Workspace remain flat flows of this presentation. Letter grouping SHALL use `MediaListRow::Heading`/`Spacer` rows. Queue SHALL use this presentation in every Panel mode.

#### Scenario: Selection is visibly distinguished only while focused

- **WHEN** a fixed-row list renders its selected row
- **THEN** the row is visibly distinguished as selected while the list holds focus
- **AND** an unfocused list marks no cursor row as selected
- **AND** a multi-selected row stays distinguished, focused or not

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
Migrated Home and Feeds rows SHALL use the canonical row model and painter. Row-indent corrections SHALL be implemented at that source of truth, not by destination-specific offsets.

#### Scenario: Wide Feeds remains one column
- **WHEN** the Feeds Service/tab is rendered at an accepted Wide breakpoint
- **THEN** it uses one column with the accepted `restore-feeds-service-wide-list` (umbrella task 1.3a) framing/background and selected-row semantics.

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

### Requirement: Audiobookshelf geometry has complete breakpoint fallbacks

Audiobookshelf Book surfaces SHALL use the shared Wide hero or Inline arrangement at the established Wide/Normal breakpoints. Audiobookshelf Podcast surfaces SHALL use the shared Wide hero when it fits and their ordinary episode rows at every other geometry, because the podcast tab has no Inline hero. Both SHALL preserve the short-height fallback and hand off stable selected target and viewport anchor across breakpoint changes. Non-list repairs required to make the composition correct SHALL live in shared arrangements or the owning destination component, not a bespoke exception.

#### Scenario: Wide and short layouts are deterministic
- **WHEN** terminal width/height crosses the Wide threshold or the short-height guard
- **THEN** the surface selects the defined Wide, Normal, or short fallback arrangement
- **AND** the selected target, row offset, images, framing, and focus remain stable.

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



### Requirement: Inline Search results compose the shared canonical media-list owner

An open Inline Search session SHALL present its scored result rows through the shared canonical media-list owner, embedded inside the Inline Search control, rendered by the one canonical fixed-row presentation in the Library panel's list box at every breakpoint — never through a bespoke search-row renderer. The search control SHALL retain what is search-specific (query, candidate pool, scoring, debounce, open/dismiss, typed activation intents); cursor, scroll, selected stable target, viewport clamping, and retained row geometry SHALL remain in the shared canonical owner, exactly as for every other list.

Result rows SHALL be selectable `Item` rows with stable opaque targets (the matched items' ids). The search flow SHALL project no `Heading` or `Spacer` rows: its result set is flat by construction. The search bar itself SHALL remain the Library panel's selector-row chrome outside the canonical row flow, replacing the pill row exactly as before.

Selection SHALL reset to the first result whenever the scored results re-fire for a changed query. A corpus or pool refresh with an unchanged query SHALL preserve the selected stable target through the shared owner's ordinary refresh rule. A responsive presentation transition SHALL keep the session open, reuse the same shared owner, and clamp the viewport, per the shared owner's geometry-change rule.

Pointer input against search result rows SHALL flow through the panel's normalized media-list surface path like every other list: a click selects the row, a double-click activates it, a wheel gesture scrolls the results viewport without moving the selection, and a right-click resolves the row's ordinary item-based context-menu intent. The search results SHALL NOT carry a private raw-event mouse path.

#### Scenario: Search results render through the fixed-row presentation

- **WHEN** Inline Search is open with scored results in either Wide or Narrow presentation
- **THEN** the result rows SHALL paint through the same canonical fixed-row presentation as every other library list

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

#### Scenario: Wheel over search results scrolls without selecting

- **WHEN** the user turns the wheel over painted search result rows
- **THEN** the results viewport scrolls by one wheel step
- **AND** the selected result is unchanged

### Requirement: The window raise keeps a group's label visible

When the shared media-list owner raises the visible window to bring the selection back into view,
it SHALL continue raising over the contiguous non-selectable rows (`Heading`/`Spacer`) directly
above the selection, so the `Heading` that labels the group containing the selection stays painted.
The raise SHALL NOT continue past the first selectable row above that label run. No other window
behaviour SHALL change: `PgUp`/`PgDn`, cursor chords, restore, and clamping keep their
current meaning, and the window's only writers are the selection-following rule and the wheel's
viewport scroll, which does not raise over label rows.

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

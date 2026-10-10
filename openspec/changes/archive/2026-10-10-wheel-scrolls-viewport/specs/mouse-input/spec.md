# Spec Delta

## MODIFIED Requirements

### Requirement: Pointer gestures are recognized by the mounted parent

Each mounted destination parent SHALL recognize click, double-click, right-click,
wheel, and drag gestures from the raw mouse events it receives, using a private
`MouseGestureState`. The double-click interval SHALL NOT be held as
shell-global state keyed by screen position. Wheel recognition SHALL NOT
throttle, coalesce or drop wheel events: every vertical wheel event the parent
receives SHALL produce exactly one scroll gesture in its direction. An embedded
canonical media-list control SHALL NOT recognize gestures — it only resolves a
point from its completed current-frame retained geometry to a stable target,
and the parent delegates list-point resolution to it. An unmigrated destination
MAY retain its existing painted-rectangle compatibility path until its own
migration.

A drag SHALL be recognized as a left-button press that arms a drag anchor at the
press position, followed by pointer motion while the button is held, and ended
by the button release. The recognizer SHALL report the anchor position and the
current pointer position with each motion, and SHALL report the end of the drag
so the parent can release any state it holds for it. Recognizing a drag SHALL
NOT suppress the click the press already produced: a press remains a click, and
a drag is an additional gesture that follows it. A press that is released
without intervening motion SHALL produce no drag gesture at all.

Drag anchor state SHALL be private to the recognizing parent, in the same way
the double-click interval is. A parent that does not interpret drag SHALL be
unaffected by the gesture's existence.

Hit geometry for a migrated uniform row flow SHALL be resolved from the completed
current-frame result the control retains, not from a separately stored per-row
rectangle list. An unmigrated destination MAY retain its existing compatibility
flow until its own migration. A stored rectangle registry is for irregular painted controls — pills,
scope buttons, transport controls, group selectors, overlay rows — whose owner
populates it in the same code that paints those rectangles.

A mounted parent SHALL translate a recognized gesture into a semantic typed `Msg`
carrying the resolved target (a child-returned row identity, a control, a pill
index, a seek fraction), never raw coordinates for the shell to re-resolve. The
shell handler for that `Msg` SHALL accept the resolved target as an argument, and
SHALL NOT read the painted geometry of the component that emitted it. A drag
gesture SHALL be translated the same way: the parent resolves both the anchor
and the current position to stable targets before emitting, and SHALL NOT emit
positions for the shell to resolve.

The gesture vocabulary SHALL remain open to hover (`enter`, `leave`) gestures
without changing the delivery or arbitration mechanism; those gestures are out
of scope for this capability but SHALL NOT be precluded by its design.

#### Scenario: A double-click activates the pointed row

- **WHEN** the user clicks the same row twice within the double-click interval
- **THEN** the mounted parent recognizes a double-click, delegates row resolution
  to the embedded list control, and emits the activation intent for that row's
  child-returned identity
- **AND** a single click on the same row emits only a focus/selection intent

#### Scenario: A wheel event scrolls the pointed list

- **WHEN** the user turns the wheel over a scrollable canonical list in any panel
- **THEN** the mounted parent recognizes the scroll gesture, and the embedded
  list control scrolls its own viewport and keeps its own selection, whether or
  not the list holds keyboard focus

#### Scenario: Rapid wheel events are all recognized

- **WHEN** several wheel events in the same direction reach a parent in quick
  succession
- **THEN** the parent recognizes one scroll gesture for each event
- **AND** none is dropped because it arrived soon after the previous one

#### Scenario: A right-click opens the context menu at the pointer

- **WHEN** the user right-clicks a selectable row on any migrated interactive
  surface that paints selectable rows
- **THEN** the row is focused and the context menu opens anchored at the click
  position

#### Scenario: A press and drag is recognized as a drag

- **WHEN** the user presses the left button over a surface and moves the pointer
  while holding it
- **THEN** the parent recognizes a click at the press position, and then a drag
  gesture for each motion, carrying both the press position and the current
  position
- **AND** releasing the button ends the drag

#### Scenario: A press without motion is only a click

- **WHEN** the user presses and releases the left button without moving the
  pointer
- **THEN** the parent recognizes a click and no drag gesture

#### Scenario: A parent that does not interpret drag is unaffected

- **WHEN** the user drags over a surface whose parent has no drag behaviour
- **THEN** the surface behaves exactly as it did before drag recognition existed

## REMOVED Requirements

### Requirement: Wheel scrolling has a uniform interaction policy

**Reason**: The one-row selection step is replaced by a viewport scroll; see "Wheel scrolling moves the viewport by a uniform step".
**Migration**: Wheel handlers apply the shared viewport scroll instead of a one-row selection move.

## ADDED Requirements

### Requirement: Wheel scrolling moves the viewport by a uniform step

Every scrollable interactive surface SHALL interpret an accepted wheel gesture as a scroll of its viewport by one fixed step of three rows or text lines, toward the preceding rows for ScrollUp and the following rows for ScrollDown, clamped at the flow bounds. A wheel gesture SHALL NOT move, select or extend the selection or cursor. No surface SHALL use its own wheel step or a page-sized wheel movement.

#### Scenario: Wheel scrolls a canonical list by one step

- **WHEN** the user turns the wheel down once over a painted canonical media list with at least three rows below the viewport
- **THEN** the list's owning component moves its viewport down by three rows
- **AND** its selection, multi-selection and live range are unchanged

#### Scenario: Wheel scrolls the selection out of view

- **WHEN** the user turns the wheel until the selected row is outside the viewport
- **THEN** the selected row stays selected and is not painted
- **AND** the detail, hero and resting-position effects that follow the selection do not run

#### Scenario: Wheel moves a text viewport by one step

- **WHEN** the user turns the wheel up once over a painted text viewport with at least three preceding lines
- **THEN** the viewport's owning component moves its local offset back by three lines
- **AND** its cursor or selection remains unchanged

#### Scenario: A boundary clamps wheel movement

- **WHEN** the user turns the wheel toward the start or end of a scrollable surface whose viewport is already at, or fewer than three rows from, that boundary
- **THEN** the viewport stops at the boundary

### Requirement: The owning component applies the wheel scroll

The component that owns the pointed list or text viewport SHALL apply the scroll locally. It SHALL send a message beyond that component only when the resolved viewport requires an existing shell-owned effect such as pagination or scroll persistence. The shell SHALL NOT recompute the wheel step or re-resolve the pointed surface.

#### Scenario: No shell-side wheel movement

- **WHEN** the user turns the wheel over a painted canonical media list
- **THEN** the list's owning component scrolls its viewport
- **AND** no shell-side wheel movement is calculated for that list

### Requirement: A wheel gesture is claimed only by its painted surface

A migrated canonical media list SHALL accept a wheel gesture only when its embedded list control claims the pointed region from its completed current-frame retained result; an unmigrated destination MAY keep its compatibility claim path. A competing text viewport or irregular-row surface SHALL accept it only inside geometry its own painter published. A sole eligible focused overlay SHALL accept it regardless of pointer position.

#### Scenario: A competing surface ignores an outside wheel gesture

- **WHEN** the user turns the wheel over painted chrome or blank space outside a competing surface's relevant list or viewport region
- **THEN** that competing surface does not change its selection, cursor, or viewport offset

#### Scenario: A focused sole overlay accepts wheel independently of pointer

- **WHEN** a focused sidebar is the sole eligible overlay and the user turns the wheel with the pointer outside its painted content
- **THEN** the sidebar scrolls its local viewport by one step
- **AND** focus does not follow the pointer

#### Scenario: The Playlists overlay wheel scrolls the visible list

This scenario changes meaning. Before this change, the Playlists wheel moved the visible list's cursor by one row.

- **WHEN** the Playlists overlay is the focused sole eligible overlay, showing either the saved-playlists list or an open playlist, and the user turns the wheel
- **THEN** the visible list's viewport scrolls by one step through the shared list owner
- **AND** that list's cursor is unchanged

#### Scenario: The Global Search sidebar wheel scrolls the results

This scenario changes meaning. Before this change, the wheel over a result row moved the result cursor by one row.

- **WHEN** the user turns the wheel over a painted Global Search sidebar result row
- **THEN** the results viewport scrolls by one step through the shared list owner
- **AND** the selected result is unchanged
- **AND** a wheel over the query row, the type-filter chips or blank space changes nothing

## MODIFIED Requirements

### Requirement: Wheel behavior is verified for each scrollable surface

Every scrollable interactive surface recorded in the interactive-surface ledger SHALL identify its uniform one-step viewport wheel behavior, whether its wheel claim is pointer-gated or focus-owned as the sole eligible overlay, and its verification. A surface rendered at more than one breakpoint SHALL verify wheel behavior at every breakpoint where its scroll region can differ.

#### Scenario: A multi-breakpoint list receives a wheel gesture

- **WHEN** a list surface is rendered in each of its supported breakpoints and the user turns the wheel over its painted list region
- **THEN** the owning component performs the same one-step viewport scroll at each breakpoint
- **AND** the ledger records the verification for each breakpoint

#### Scenario: An obsolete wheel relay is removed

- **WHEN** a component can apply an accepted wheel gesture entirely within its own local interaction state
- **THEN** it does not emit a shell request solely to relay that movement
- **AND** verification confirms the local state changes without a shell-side wheel handler

**Verification record: affected surfaces**

The implementation and interactive-surface ledger record the following affected
scrollable surfaces. Each uses the normalized signed gesture directly, claims
the listed painted region, and retains only the stated semantic boundary:

| Surface | Local owner and painted claim | Breakpoint evidence | Semantic boundary |
| --- | --- | --- | --- |
| Emby library (plain kinds: Movies / Home videos / Generic) | `EmbyLibraryContent` over the Library panel's canonical list; `WideMediaList` in Wide or `InlineMediaBrowser` in Normal/Narrow | Wide and Normal/Narrow owner paths; panel tick coverage | resolved viewport reach for pagination and resolved scroll for persistence only |
| Wide TV | `TvContent`; painted series rail claimed by its embedded list | Wide workspace tests; narrow ownership is TV's own content owner | none for wheel; no relay |
| Home | `HomeComponent`; canonical list or inline-hero claim | Wide and Normal/Narrow | none for wheel |
| Queue | `QueueComponent`; painted `WideMediaList` queue region | Wide and narrow | none for wheel |
| Music | `MusicWorkspaceComponent`; Wide rail or Normal/Narrow inline list | Wide and Normal/Narrow | resolved viewport reach for pagination |
| Feeds | `FeedsComponent`; active canonical list region | Wide and Normal/Narrow | none for wheel |
| Audiobookshelf podcast | `PodcastContent` over the Library panel's canonical list; painted episode-row geometry | Wide and Normal/Narrow | none for wheel |
| Audiobookshelf books | `AudiobookshelfBookComponent`; painted book- or chapter-row geometry | Wide and Normal/Narrow | panel focus only |
| Inline Search | active host component; painted results `left_area`, first refusal | Emby library, Music, and TV host paths | local results viewport only |
| Global Search sidebar | `SearchSidebarComponent`'s shared list owner (`MediaListCarrier`) over the filtered results; claim through painter-published result-row hit regions | fixed overlay geometry (breakpoint-invariant) | local results viewport only |
| Settings | `SettingsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Help | `HelpComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Sessions | `SessionsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Playlists | `PlaylistsComponent`'s shared list owners (`MediaListCarrier`, one for the saved playlists and one for the open playlist); focus-owned wheel while sole eligible overlay scrolls the visible list | fixed overlay geometry (breakpoint-invariant) | none for wheel |

Focused verification names and geometry evidence are maintained with the rows in
`docs/architecture/interactive-surface-ledger.md`; canonical-list proofs retain
pointed-region rejection, while focused-sidebar proofs cover down/up direction,
boundaries, and an off-panel pointer. These records do not add a second
interaction policy or require a shell wheel handler.

## ADDED Requirements

### Requirement: Wheel scrolling loads further library pages

A lazily paged library list SHALL treat the last row painted in its viewport as a paging position, in addition to its selection. When a wheel scroll brings that last painted row within the list's existing prefetch distance of the loaded edge, the next page SHALL be requested immediately, with no navigation-idle gate.

#### Scenario: Scrolling toward the loaded edge fetches the next page

- **WHEN** a paged library list has more items on the server than are loaded and the user wheel-scrolls until the last painted row is within the prefetch distance of the loaded edge
- **THEN** the next page is requested
- **AND** the selection is unchanged

#### Scenario: A fully loaded list requests nothing

- **WHEN** the user wheel-scrolls a list whose items are all loaded
- **THEN** no page request is made

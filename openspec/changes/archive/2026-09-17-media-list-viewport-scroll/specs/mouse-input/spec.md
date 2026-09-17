## MODIFIED Requirements

### Requirement: Wheel scrolling has a uniform interaction policy

Every scrollable interactive surface SHALL interpret an accepted wheel gesture as exactly one forward or backward logical step: ScrollUp moves toward the preceding row or viewport line, and ScrollDown moves toward the following row or viewport line. A surface SHALL NOT use a per-surface wheel multiplier or page-sized wheel movement.

The component that owns the pointed list, selection, or text viewport SHALL apply the step locally. It SHALL send a message beyond that component only when the resolved result requires an existing shell-owned effect or persistence operation; the shell SHALL NOT recompute the wheel step or re-resolve the pointed surface.

A migrated canonical media-list surface SHALL accept a wheel gesture only when its
embedded list control claims the pointed list region from its completed current-frame
retained result. An unmigrated destination MAY retain its existing compatibility
claim path until its own migration. A text viewport or irregular-row surface that competes with another eligible surface SHALL accept a wheel gesture only when the point is inside geometry published by its own painter. A sole eligible focused overlay SHALL accept its wheel gesture independently of pointer position; its local boundary clamp SHALL retain valid state.

A canonical media list SHALL apply an accepted wheel step to its viewport rather than to its selection: the visible window moves one display row and the selection stays where it is, unless the step would put the selection outside the window, in which case the selection is dragged to the nearest row the window shows. The list SHALL apply the step to its viewport even when the pointed list does not hold keyboard focus.

#### Scenario: Wheel moves a canonical list by one row

- **WHEN** the user turns the wheel down once over a painted canonical media list with a following row
- **THEN** the list's owning component moves its local viewport down by one row
- **AND** the selection moves only when it would otherwise leave the viewport
- **AND** no shell-side wheel movement is calculated for that list

#### Scenario: Wheel moves a text viewport by one line

- **WHEN** the user turns the wheel up once over a painted text viewport with a preceding line
- **THEN** the viewport's owning component moves its local offset back by one line
- **AND** its cursor or selection remains unchanged unless that surface's ordinary one-step navigation also changes it

#### Scenario: A wheel step reaches the first display row of a grouped list

- **WHEN** the user turns the wheel up repeatedly over a grouped canonical list whose first selectable row follows a group `Heading`
- **THEN** the viewport reaches the first display row
- **AND** the first group's `Heading` is painted

#### Scenario: A competing surface ignores an outside wheel gesture

- **WHEN** the user turns the wheel over painted chrome or blank space outside a competing surface's relevant list or viewport region
- **THEN** that competing surface does not change its selection, cursor, or viewport offset

#### Scenario: A focused sole overlay accepts wheel independently of pointer

- **WHEN** a focused sidebar is the sole eligible overlay and the user turns the wheel with the pointer outside its painted content
- **THEN** the sidebar moves its local selection or viewport by one step
- **AND** focus does not follow the pointer

#### Scenario: A boundary clamps wheel movement

- **WHEN** the user turns the wheel toward the start or end of a scrollable surface that is already at that boundary
- **THEN** the component retains its valid boundary selection and viewport offset
- **AND** no shell-side movement is requested

### Requirement: Wheel behavior is verified for each scrollable surface

Every scrollable interactive surface recorded in the interactive-surface ledger SHALL identify its uniform one-step wheel behavior, whether its wheel claim is pointer-gated or focus-owned as the sole eligible overlay, and its verification. A surface rendered at more than one breakpoint SHALL verify wheel behavior at every breakpoint where its scroll region can differ.

#### Scenario: A multi-breakpoint list receives a wheel gesture

- **WHEN** a list surface is rendered in each of its supported breakpoints and the user turns the wheel over its painted list region
- **THEN** the owning component performs the same one-step movement at each breakpoint
- **AND** the ledger records the verification for each breakpoint

#### Scenario: An obsolete wheel relay is removed

- **WHEN** a component can apply an accepted wheel gesture entirely within its own local interaction state
- **THEN** it does not emit a shell request solely to relay that movement
- **AND** verification confirms the local state changes without a shell-side wheel handler

#### Scenario: A viewport-only wheel step reports no selection move

- **WHEN** the user turns the wheel over a canonical list whose viewport can move without dragging the selection
- **THEN** the owning component emits no selection or cursor message for that step
- **AND** it still reports the resolved position when an existing persistence or pagination operation needs it

**Verification record: affected surfaces**

The implementation and interactive-surface ledger record the following affected
scrollable surfaces. Each uses the normalized signed gesture directly, claims
the listed painted region, and retains only the stated semantic boundary:

| Surface | Local owner and painted claim | Breakpoint evidence | Semantic boundary |
| --- | --- | --- | --- |
| Emby library (plain kinds: Movies / Home videos / Generic) | `EmbyLibraryContent` over the Library panel's canonical list; the same `WideMediaList` fixed-row presentation at both breakpoints — a narrower painted height in Normal/Narrow, never a second presentation | Wide and Normal/Narrow owner paths; panel tick coverage | resolved position for persistence and pagination; resolved `EmbyLibraryCursorIndex` only when the step dragged the selection |
| Wide TV | `TvContent`; painted series rail claimed by its embedded list | Wide workspace tests; narrow ownership is TV's own content owner | none for wheel; no relay |
| Home | `HomeComponent`; canonical list or inline-hero claim | Wide and Normal/Narrow | resolved Continue Watching position for persistence; cursor effect only when the step dragged the selection |
| Queue | `QueueComponent`; painted `WideMediaList` queue region | Wide and narrow | none for wheel |
| Music | `MusicWorkspaceComponent`; Wide rail or Normal/Narrow inline list | Wide and Normal/Narrow | resolved album position for persistence; album cursor request only when the step dragged the selection |
| Feeds | `FeedsComponent`; active canonical list region | Wide and Normal/Narrow | none for wheel |
| Audiobookshelf podcast | `AudiobookshelfPodcastComponent`; painted show-row geometry | Wide and Normal/Narrow | resolved show position; selection only when the step dragged it |
| Audiobookshelf books | `AudiobookshelfBookComponent`; painted book- or chapter-row geometry | Wide and Normal/Narrow | resolved book/chapter position; selection or focus only when the step dragged it |
| Inline Search | active host component; painted results `left_area`, first refusal | Emby library, Music, and TV host paths | local result viewport and selection |
| Global Search sidebar | `SearchSidebarComponent`; painter-published result-row hit regions | fixed overlay geometry (breakpoint-invariant) | local result selection only |
| Settings | `SettingsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Help | `HelpComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Sessions | `SessionsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | selection/connect action remains semantic |
| Playlists | `PlaylistsComponent`; focus-owned wheel while sole eligible overlay | fixed overlay geometry (breakpoint-invariant) | playlist/open-item actions remain semantic |

Focused verification names and geometry evidence are maintained with the rows in
`docs/architecture/interactive-surface-ledger.md`; canonical-list proofs retain
pointed-region rejection, while focused-sidebar proofs cover down/up direction,
boundaries, and an off-panel pointer. These records do not add a second
interaction policy or require a shell wheel handler.

A canonical-list row's verification SHALL additionally prove the viewport step: the
window moves one row, the selection keeps its painted screen row unless the step
drags it, and the window reaches the first and last display row. Surfaces outside the
canonical media-list owner (Global Search, Settings, Help, Sessions, Playlists) keep
their existing selection- or offset-driven wheel behavior for now.

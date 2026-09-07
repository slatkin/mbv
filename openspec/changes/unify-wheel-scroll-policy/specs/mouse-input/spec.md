## ADDED Requirements

### Requirement: Wheel scrolling has a uniform interaction policy
Every scrollable interactive surface SHALL interpret an accepted wheel gesture as exactly one forward or backward logical step: ScrollUp moves toward the preceding row or viewport line, and ScrollDown moves toward the following row or viewport line. A surface SHALL NOT use a per-surface wheel multiplier or page-sized wheel movement.

The component that owns the pointed list, selection, or text viewport SHALL apply the step locally. It SHALL send a message beyond that component only when the resolved result requires an existing shell-owned effect or persistence operation; the shell SHALL NOT recompute the wheel step or re-resolve the pointed surface.

A canonical media-list surface SHALL accept a wheel gesture only when its embedded list control claims the pointed list region. A text viewport or an irregular row surface without an embedded canonical control SHALL accept a wheel gesture only when the point is inside the geometry published by its own painter. A gesture outside the relevant painted scroll region SHALL leave the surface unchanged.

#### Scenario: Wheel moves a canonical list by one row
- **WHEN** the user turns the wheel down once over a painted canonical media list with a following row
- **THEN** the list's owning component advances its local selection and viewport by one row
- **AND** no shell-side wheel movement is calculated for that list

#### Scenario: Wheel moves a text viewport by one line
- **WHEN** the user turns the wheel up once over a painted text viewport with a preceding line
- **THEN** the viewport's owning component moves its local offset back by one line
- **AND** its cursor or selection remains unchanged unless that surface's ordinary one-step navigation also changes it

#### Scenario: Wheel outside a scroll region is ignored
- **WHEN** the user turns the wheel over painted chrome or blank space outside a surface's relevant list or viewport region
- **THEN** that surface does not change its selection, cursor, or viewport offset

#### Scenario: A boundary clamps wheel movement
- **WHEN** the user turns the wheel toward the start or end of a scrollable surface that is already at that boundary
- **THEN** the component retains its valid boundary selection and viewport offset
- **AND** no shell-side movement is requested

### Requirement: Wheel behavior is verified for each scrollable surface
Every scrollable interactive surface recorded in the interactive-surface ledger SHALL identify its uniform one-step wheel behavior, the painted region that accepts it, and its verification. A surface rendered at more than one breakpoint SHALL verify wheel behavior at every breakpoint where its scroll region can differ.

#### Scenario: A multi-breakpoint list receives a wheel gesture
- **WHEN** a list surface is rendered in each of its supported breakpoints and the user turns the wheel over its painted list region
- **THEN** the owning component performs the same one-step movement at each breakpoint
- **AND** the ledger records the verification for each breakpoint

#### Scenario: An obsolete wheel relay is removed
- **WHEN** a component can apply an accepted wheel gesture entirely within its own local interaction state
- **THEN** it does not emit a shell request solely to relay that movement
- **AND** verification confirms the local state changes without a shell-side wheel handler

### Verification record: affected surfaces
The implementation and interactive-surface ledger record the following affected
scrollable surfaces. Each uses the normalized signed gesture directly, claims
the listed painted region, and retains only the stated semantic boundary:

| Surface | Local owner and painted claim | Breakpoint evidence | Semantic boundary |
| --- | --- | --- | --- |
| Browser, including narrow TV | `BrowserComponent`; `WideMediaList` in Wide or `InlineMediaBrowser` in Normal/Narrow | Wide and Normal/Narrow component paths; narrow mounted-owner tick coverage | resolved `BrowserCursorIndex` for persistence only |
| Wide TV | `TvWorkspaceComponent`; painted series rail claimed by its embedded list | Wide workspace tests; narrow ownership is Browser | none for wheel; no relay |
| Home | `HomeComponent`; canonical list or inline-hero claim | Wide and Normal/Narrow | resolved Continue Watching cursor effect only |
| Queue | `QueueComponent`; painted `WideMediaList` queue region | Wide and narrow | none for wheel |
| Music | `MusicWorkspaceComponent`; Wide rail or Normal/Narrow inline list | Wide and Normal/Narrow | resolved album cursor request |
| Feeds | `FeedsComponent`; active canonical list region | Wide and Normal/Narrow | none for wheel |
| Audiobookshelf podcast | `AudiobookshelfPodcastComponent`; painted show-row geometry | Wide and Normal/Narrow | resolved show selection |
| Audiobookshelf books | `AudiobookshelfBookComponent`; painted book- or chapter-row geometry | Wide and Normal/Narrow | resolved book/chapter selection or focus |
| Inline Search | active host component; painted results `left_area`, first refusal | Browser, Music, and TV host paths | local result selection only |
| Settings | `SettingsComponent`; painter-published `content_area` | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Help | `HelpComponent`; painter-published `content_area` | fixed overlay geometry (breakpoint-invariant) | none for wheel |
| Sessions | `SessionsComponent`; painter-published session-row hit regions | fixed overlay geometry (breakpoint-invariant) | selection/connect action remains semantic |
| Playlists | `PlaylistsComponent`; painter-published wrapped-row hit regions | fixed overlay geometry (breakpoint-invariant) | playlist/open-item actions remain semantic |

Focused verification names and geometry evidence are maintained with the rows in
`docs/architecture/interactive-surface-ledger.md`; this record does not add a
second interaction policy or require a shell wheel handler.

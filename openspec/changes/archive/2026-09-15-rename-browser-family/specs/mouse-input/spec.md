## MODIFIED Requirements

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

**Verification record: affected surfaces**

The implementation and interactive-surface ledger record the following affected
scrollable surfaces. Each uses the normalized signed gesture directly, claims
the listed painted region, and retains only the stated semantic boundary:

| Surface | Local owner and painted claim | Breakpoint evidence | Semantic boundary |
| --- | --- | --- | --- |
| Emby library (plain kinds: Movies / Home videos / Generic) | `EmbyLibraryContent` over the Library panel's canonical list; `WideMediaList` in Wide or `InlineMediaBrowser` in Normal/Narrow | Wide and Normal/Narrow owner paths; panel tick coverage | resolved `EmbyLibraryCursorIndex` for persistence only |
| Wide TV | `TvContent`; painted series rail claimed by its embedded list | Wide workspace tests; narrow ownership is TV's own content owner | none for wheel; no relay |
| Home | `HomeComponent`; canonical list or inline-hero claim | Wide and Normal/Narrow | resolved Continue Watching cursor effect only |
| Queue | `QueueComponent`; painted `WideMediaList` queue region | Wide and narrow | none for wheel |
| Music | `MusicWorkspaceComponent`; Wide rail or Normal/Narrow inline list | Wide and Normal/Narrow | resolved album cursor request |
| Feeds | `FeedsComponent`; active canonical list region | Wide and Normal/Narrow | none for wheel |
| Audiobookshelf podcast | `AudiobookshelfPodcastComponent`; painted show-row geometry | Wide and Normal/Narrow | resolved show selection |
| Audiobookshelf books | `AudiobookshelfBookComponent`; painted book- or chapter-row geometry | Wide and Normal/Narrow | resolved book/chapter selection or focus |
| Inline Search | active host component; painted results `left_area`, first refusal | Emby library, Music, and TV host paths | local result selection only |
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

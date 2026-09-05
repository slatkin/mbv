## ADDED Requirements

### Requirement: Library-panel context and search shortcuts reach every Emby library kind

The library-panel context-menu shortcut (`.`) and search-library shortcut
(`/`) SHALL be handled for every Emby library kind the left panel can select:
generic/Movies/HomeVideos, Music, and TV. Each SHALL be resolved by the
interactive component that owns the focused destination's surface. For `.`,
the component SHALL resolve the target item from its own cursor and content
and emit a typed request; the shell SHALL open the Service context menu for
that item against the explicitly selected Emby library, running the same
effect it runs for `.` on the generic Emby browser. For `/`, the component
SHALL emit the shared open-inline-search request the generic browser emits.
Neither shortcut SHALL be handled for only a subset of library kinds, and
neither SHALL depend on a legacy fallback keyboard endpoint.

#### Scenario: Context menu opens on a Music library album

- **WHEN** an Emby Music library has left-panel focus, an album is
  highlighted, and no inline track is focused
- **AND** the user presses `.`
- **THEN** the Music workspace component emits a typed context-menu request
  carrying the highlighted album
- **AND** the shell opens the Emby context menu for that album, the same menu
  it opens for `.` on the generic Emby browser

#### Scenario: Context menu opens on a TV library

- **WHEN** an Emby TV library has left-panel focus and a series or episode is
  highlighted
- **AND** the user presses `.`
- **THEN** the TV workspace component emits a typed context-menu request
  carrying the series-list selection, which stays authoritative even while
  the Episodes pane is focused
- **AND** the shell opens the Emby context menu for that item

#### Scenario: Inline track focus keeps the track context menu

- **WHEN** an inline album track is focused in the Music workspace
- **AND** the user presses `.`
- **THEN** the context menu targets the focused track, not the album
- **AND** the album-level context-menu request does not also fire

#### Scenario: Search opens the library search on Music and TV

- **WHEN** an Emby Music or TV library has left-panel focus
- **AND** the user presses `/`
- **THEN** the component emits the same open-inline-search request the
  generic Emby browser emits
- **AND** the shell opens inline search scoped to the selected Emby library

#### Scenario: Empty list leaves the shortcut unclaimed

- **WHEN** the focused Music or TV library has no highlightable item
- **AND** the user presses `.`
- **THEN** the component emits no request and no context menu opens

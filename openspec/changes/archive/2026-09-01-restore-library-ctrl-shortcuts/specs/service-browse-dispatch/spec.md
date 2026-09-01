## ADDED Requirements

### Requirement: Library-panel item shortcuts reach every Emby library kind

The library-panel item shortcuts — play (`Ctrl+P`), enqueue (`Ctrl+A`),
toggle watched (`Ctrl+W`), shuffle (`Ctrl+S`), rescan (`Ctrl+R`), and refresh
(bare `r`) — SHALL be handled for every Emby library kind the left panel can
select: generic/Movies/HomeVideos, Music, and TV. Each shortcut SHALL be
resolved by the interactive component that owns the focused destination's
surface, which SHALL resolve the target item from its own cursor and content
and emit a typed request; the shell SHALL apply that request to the
explicitly selected Emby library. A shortcut SHALL NOT be handled for only a
subset of library kinds, and SHALL NOT depend on a legacy fallback keyboard
endpoint.

#### Scenario: Shortcut works on a Music library

- **WHEN** an Emby Music library has left-panel focus, an album is
  highlighted, and no inline track is focused
- **AND** the user presses `Ctrl+S` (shuffle), `Ctrl+R` (rescan), `Ctrl+W`
  (toggle watched), `Ctrl+A` (enqueue), or `Ctrl+P` (play)
- **THEN** the Music workspace component emits the corresponding typed
  request carrying the highlighted album
- **AND** the shell runs the same effect it runs for that shortcut on the
  generic Emby browser, against the selected Music library

#### Scenario: Shortcut works on a TV library

- **WHEN** an Emby TV library has left-panel focus and a series or episode is
  highlighted
- **AND** the user presses `Ctrl+S`, `Ctrl+R`, `Ctrl+W`, `Ctrl+A`, or
  `Ctrl+P`
- **THEN** the TV workspace component emits the corresponding typed request
  carrying the highlighted item
- **AND** the shell runs the same effect it runs for that shortcut on the
  generic Emby browser, against the selected TV library

#### Scenario: Inline track focus keeps its own meaning

- **WHEN** an inline album track is focused in the Music workspace
- **AND** the user presses `Ctrl+P` or `Ctrl+A`
- **THEN** the shortcut acts on the focused track, not the album
- **AND** the album-level library shortcut does not also fire

#### Scenario: Empty list leaves the shortcut unclaimed

- **WHEN** the focused Music or TV library has no highlightable item
- **AND** the user presses one of the library-panel item shortcuts
- **THEN** the component emits no request and destination, queue, playback,
  and Service state are unchanged

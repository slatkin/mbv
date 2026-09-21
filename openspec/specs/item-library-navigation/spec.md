# item-library-navigation Specification

## Purpose
Lets the user jump an Emby library straight to the surface state representing a
chosen item from another surface — the queue context menu's "Go to Library" and
Search sidebar activation — landing on what that destination actually renders
instead of a generic ancestor chain.

## Requirements

### Requirement: Item navigation routes per destination kind

When the shell navigates an Emby library to a chosen item (queue "Go to
Library", Search sidebar activation), the landing state SHALL be derived per
destination kind from the item's type, not by rebuilding every ancestor as a
browse level. A Movie or generic video item SHALL land on the library's root
browse level with the cursor on the item. A Series SHALL land on the root
browse level with the cursor on the series. An Episode or Season SHALL resolve
its owning Series and land on that show; a Season or Episode SHALL never exist
as the top browse level of the landed navigation. A Music track or album
SHALL resolve to its album and land with that album selected in the Music
surface, with the track list as the workspace content in the album-grouped
view (in a flat music library the landed top level IS the track list). A
MusicArtist SHALL NOT land: the navigation SHALL fail with a flash and leave
the current view unchanged (an artist has no single owning album, and a plain
artist browse chain does not render on a grouped Music surface - verified by
a real-tick render check).

#### Scenario: Episode in the queue navigates to its show

- **WHEN** the user chooses "Go to Library" on a queued Episode
- **THEN** the TV library tab becomes the active tab selection with the show
  selected in the series list
- **AND** no browse level below the series list exists in the landed
  navigation

#### Scenario: Track in the queue navigates to its album

- **WHEN** the user chooses "Go to Library" on a queued Music track
- **THEN** the Music library tab becomes the active tab selection with the
  track's album selected and its track list available as the workspace content

#### Scenario: Movie in the queue navigates to its list row

- **WHEN** the user chooses "Go to Library" on a queued Movie
- **THEN** the movie's library tab becomes the active tab selection with the
  cursor resting on the movie in the root browse level

### Requirement: A navigated show opens its detail presentation

When item navigation lands on a Series (chosen directly, or resolved from an
Episode or Season), the show's detail SHALL open through the same hand-off the
destination uses for an ordinary series activation: the Workspace in the Wide
hero arrangement, the Library Hero overlay otherwise. The landing SHALL NOT
depend on a follow-up manual activation to make the show's seasons and episodes
visible.

#### Scenario: Navigated show opens its workspace in Wide

- **WHEN** item navigation resolves a show and the library is in the Wide hero
  arrangement
- **THEN** the show is selected in the series list with its Workspace open

#### Scenario: Navigated show opens the overlay in Narrow

- **WHEN** item navigation resolves a show and the library is not in the Wide
  hero arrangement
- **THEN** the show is selected in the series list and the Library Hero overlay
  opens for it

### Requirement: Navigation replaces the saved Library position

When an item navigation completes for a library, the landed state SHALL become
that library's saved Library position, and the tab-switch activation SHALL NOT
restore a previously saved position over it. A pending restore for a
pre-navigation position SHALL NOT apply after the navigation completes.

#### Scenario: Tab switch keeps the navigated state

- **WHEN** item navigation replaces the browse state and switches the tab
  selection to the target library
- **THEN** the landed cursor and any opened detail survive the tab-switch
  activation pass

#### Scenario: Stale restore does not clobber

- **WHEN** a position restore for the pre-navigation state completes after the
  navigation landed
- **THEN** the landed state remains and the stale restore is discarded

### Requirement: Retained destinations re-anchor to the navigation

When item navigation targets a destination whose Interactive Component is
already mounted, the component SHALL re-anchor its selection to the landed
item before the next presentation push, so the workspace or list reflects the
navigation rather than the pre-navigation selection.

#### Scenario: Previously visited TV library follows the navigation

- **WHEN** item navigation lands on a TV library the user visited earlier in
  the session
- **THEN** the retained TV destination's series selection points at the
  navigated show before the next content push

### Requirement: Navigated workspaces select the chosen episode or track

When item navigation resolves an Episode to its show, the opened TV Workspace
SHALL select that episode (season + episode resolved, fetching season episodes
when uncached) with episode focus. When item navigation resolves a Music track
to its album, the album's workspace track list SHALL select that track. A
Season reveal keeps the show-level landing with default selection. When the
chosen episode or track is absent from the fetched detail, the show/album
landing SHALL stand with default selection and no error. Movie/generic
behavior is unchanged (the cursor already rests on the item).

#### Scenario: Navigated workspace selects the episode

- **WHEN** the user chooses "Go to Library" on a queued Episode and the
  library is in the Wide hero arrangement
- **THEN** the show is selected with its Workspace open
- **AND** the episode is selected in the workspace

#### Scenario: Navigated track list selects the track

- **WHEN** the user chooses "Go to Library" on a queued Music track
- **THEN** the track's album is selected with its track list as the workspace
  content
- **AND** the track is selected in the track list

### Requirement: Grouped Music navigation lands and never silently no-ops

A queue or Search sidebar navigation to a Music item in a Grouped Music library SHALL land the tree on the resolved album with that album's Workspace content available, and SHALL select the navigated track in that Workspace once its tracks are available. The landing SHALL apply its own grouped-catalog state and re-anchor a previously retained Music destination rather than depending on an unrelated later event. A navigation SHALL prepare its resolved grouped-catalog landing before committing any tab, nav-stack, saved-position, or retained-component change. A navigation that cannot resolve, activate, or apply that prepared landing SHALL surface the existing library-error feedback and leave the current view unchanged; it SHALL NOT silently do nothing. For a music library whose configured `music.levels` shape is absent or does not terminate at an album level, music item navigation SHALL surface the existing library-error feedback and change no destination state; it SHALL NOT silently no-op.

#### Scenario: Navigated music track lands in the grouped tree

- **WHEN** the user chooses "Go to Library" on a queued Music track in a library whose Grouped Music tree is active
- **THEN** the Music tab is active with the resolved album selected in the tree
- **AND** the album-track Workspace is available with the navigated track selected once its tracks arrive

#### Scenario: Retained grouped Music destination follows the navigation

- **WHEN** the Grouped Music tree for the target library was already mounted and showing another album
- **THEN** the tree re-anchors to the resolved album and no stale selection remains

#### Scenario: Unresolvable music navigation reports a failure

- **WHEN** a music navigation cannot resolve its album or cannot apply a landing
- **THEN** the existing library-error feedback is shown
- **AND** the active tab, nav stack, saved Library position, and retained destination selection remain unchanged

#### Scenario: Apply-stage failure is atomic

- **WHEN** album resolution succeeds but grouped-state construction or application fails
- **THEN** no partial landing becomes visible or saved
- **AND** the existing library-error feedback is shown

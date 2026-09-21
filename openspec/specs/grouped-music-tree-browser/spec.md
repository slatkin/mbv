# grouped-music-tree-browser Specification

## Purpose

Provide a shallow, directly navigable artist-and-album tree for Grouped Music, including local fuzzy filtering and group-scoped actions, without changing other library lists.

## Requirements

### Requirement: Grouped Music projects one stable shallow tree
The Grouped Music browser SHALL present each settled artist group as a focusable artist root with its settled albums as leaf children, and each album leaf MAY present its cached tracks as selectable track items ordered by disc/track order (three-level projection: artist → album → track). Each track item's label SHALL begin with its track number in `<number>. <title>` form, using the item's index number when present and its position within the album otherwise, matching the album- and artist-Workspace track rows. It SHALL preserve settled artist and album order. Artist identity SHALL use stable Service identity when available so equal display names remain distinct, and SHALL use a deterministic fallback identity when the Service supplies none. Stable artist identity SHALL come only from `ArtistItems` pairs carried on album/item payloads; IDs obtained from an `/Artists` listing SHALL NOT be mixed into artist keys, artwork requests, or artist-track queries. Album leaves SHALL retain their existing stable album targets.

The tree SHALL have one owner for its selected node, expansion, viewport, multi-selection, and current-frame hit geometry. Track items SHALL be projected only from the shell-owned artist-detail cache (and the per-album track cache on the fallback path) already established for the focused artist; the tree SHALL NOT issue track fetches of its own, and an album without cached tracks SHALL simply present no track children. Ordinary settled-catalog replacement SHALL preserve the selected node, expansion state, and surviving multi-selection by stable identity. When the selected node survives, its prior viewport row SHALL be preserved when projection bounds permit; otherwise the viewport SHALL apply only the minimum scroll needed to keep it visible and clamp at projection bounds. A responsive presentation change SHALL reuse that owner and apply the same visibility rule rather than copying state into another control.

#### Scenario: Equal artist names remain distinct
- **WHEN** two settled artist groups have the same display name but different stable Service identities
- **THEN** the browser presents two independently focusable and expandable artist roots
- **AND** albums and actions remain scoped to the correct root

#### Scenario: A settled replacement preserves tree state
- **WHEN** a replacement settled catalog still contains the selected node and expanded artist roots
- **THEN** the selected node and surviving expansion state remain in use
- **AND** its prior viewport row is retained when bounds permit, otherwise the viewport scrolls only enough to keep it visible

#### Scenario: Fallback artist remains browsable
- **WHEN** an album has no stable artist identity from the Service
- **THEN** it belongs to a deterministic fallback artist root that remains stable across an ordinary refresh
- **AND** that root does not request artist artwork

#### Scenario: /Artists listing IDs are not mixed into keys
- **WHEN** an artist identity from an `/Artists` listing is available for the same display name
- **THEN** no artist key, artwork request, or artist-track query uses that ID
- **AND** the root's identity still comes only from `ArtistItems` or the deterministic fallback

#### Scenario: Track nodes come from the cache without refetch
- **WHEN** an album leaf expands to its track items
- **THEN** those items are projected from the shell-owned cached track data for the focused artist in disc/track order
- **AND** the tree issues no track fetch of its own and shows no track children for an album with no cached tracks

#### Scenario: Track rows carry their track number
- **WHEN** an album leaf's cached tracks paint as track items
- **THEN** each row's label begins with that track's number followed by a period and the track title
- **AND** a track without an index number uses its position within the album

### Requirement: Artist roots and album leaves have distinct navigation
Up, Down, `j`, and `k` SHALL move across visible artist roots and album leaves. With no tree filter active, Enter on an artist root SHALL open or focus that root's Hero — the Wide artist Workspace where the Wide hero arrangement applies, otherwise the Library Hero overlay with that artist-track Workspace focused — exactly as Enter on an album leaf opens its Hero; Enter SHALL NOT toggle expansion. While the tree filter is active, Enter on an artist root SHALL retain the filter's local expansion behavior and SHALL NOT open the Library Hero overlay. Right on a collapsed artist root SHALL expand it; Right on an already expanded artist root SHALL enter its artist-track Workspace, opening the Library Hero overlay first in non-Wide geometry. Left SHALL collapse a focused expanded artist root, or move a focused album leaf to its artist parent. Enter on an album leaf SHALL retain the existing album activation behavior; when filtering is active, it SHALL first dismiss the filter and focus the album in the unfiltered tree before enabling album-track selection. Enter on a track item SHALL invoke the library playback policy for that Audio item through the existing playback and admission executor. With autoload enabled, the replacement queue SHALL contain that track's cached album tracks in disc/track order and start at the selected track; with autoload disabled, it SHALL contain only the selected track. When the target queue that playback would replace is populated, Enter SHALL ask for confirmation before replacing it; an empty target queue SHALL play immediately. Album-leaf and artist-root navigation and activation behaviors SHALL be unchanged by the presence of track items. Page navigation SHALL operate on the tree's visible-node viewport and SHALL NOT inherit Heading-based canonical-list group jumps.

#### Scenario: Collapse removes descendants from navigation
- **WHEN** the user collapses a focused artist root
- **THEN** its album leaves leave the visible projection
- **AND** selection, viewport, and later hit resolution remain valid

#### Scenario: Left returns a leaf to its parent
- **WHEN** an album leaf is focused and the user presses Left
- **THEN** focus moves to that album's artist root without collapsing a different root

#### Scenario: Right enters an expanded artist Workspace
- **WHEN** an expanded artist root is focused and the user presses Right
- **THEN** its artist-track Workspace receives focus
- **AND** non-Wide geometry opens that Workspace in the Library Hero overlay

#### Scenario: Enter opens the unfiltered artist Hero
- **WHEN** the user presses Enter on a focused artist root with no tree filter active
- **THEN** that root's Hero opens or takes focus, with the artist-track Workspace focused
- **AND** the root's expansion state is unchanged

#### Scenario: Filtered artist Enter stays local
- **WHEN** the user presses Enter on a focused artist root while the tree filter is active
- **THEN** the filter's existing local expansion behavior runs
- **AND** no Library Hero overlay opens

#### Scenario: Album activation is preserved
- **WHEN** the user presses Enter on an album leaf
- **THEN** the existing album Hero and album-track Workspace behavior is invoked

#### Scenario: Filtered album activation dismisses filtering
- **WHEN** the user presses Enter on an album leaf while Grouped Music filtering is active
- **THEN** filtering closes and the unfiltered tree focuses that album
- **AND** the existing album-track selection behavior is enabled

#### Scenario: Track activation uses the existing playback arms
- **WHEN** the user presses Enter on a track item under an album leaf
- **THEN** autoload on queues the cached album tracks from that track while autoload off queues only that track
- **AND** both paths use the existing playback and admission executor
- **AND** artist-root and album-leaf behaviors are unchanged

#### Scenario: Track Enter confirms queue replacement
- **WHEN** the user presses Enter on a track item and the target queue is populated
- **THEN** confirmation is requested before that queue is replaced
- **AND** cancelling leaves the queue and playback unchanged

#### Scenario: Track Enter with an empty queue plays immediately
- **WHEN** the user presses Enter on a track item and the target queue is empty
- **THEN** playback starts immediately: the empty target queue skips the queue-replacement confirmation prompt, while the save/discard protection for an unsaved local queue change still applies as it does for every other queue-replacing flow

#### Scenario: Hero and Workspace remain the full-album surface
- **WHEN** track items are visible in the tree
- **THEN** the album Hero and album-track Workspaces remain available and unchanged as the full-album listening surface

### Requirement: Artist actions resolve visible album descendants
Play, enqueue, shuffle, and context actions invoked on an artist root SHALL resolve to that root's visible album leaves in settled display order. While no filter is active, all settled child albums are visible regardless of expansion. While a filter is active, only matching visible child albums are in scope. Artist identities SHALL never be emitted as playback, queue, or context effect targets.

#### Scenario: Action on a collapsed unfiltered artist
- **WHEN** an artist root is collapsed with no filter active and the user invokes enqueue
- **THEN** every settled child album is materialized in display order
- **AND** only album identities cross the effect boundary

#### Scenario: Filtered artist action uses visible matches
- **WHEN** a filter leaves two of an artist's five albums visible and the user invokes play on that artist root
- **THEN** only those two albums are materialized in settled display order

#### Scenario: A name-only filtered artist action is inert
- **WHEN** a filter matches only an artist's name, so none of its album leaves match and none are visible
- **AND** the user invokes play or enqueue on that artist root while the filter is active
- **THEN** the action resolves to no visible album leaves and emits no playback or queue effect targets

### Requirement: Fuzzy filtering narrows the settled tree in place
Pressing `/` on the focused Grouped Music browser SHALL open the existing one-row Inline Search bar while retaining the tree in the browser area. Query text SHALL appear immediately and, after a 300 ms debounce, every node SHALL be matched under the same shared word-local fuzzy rule as the other library searches — every word of the query SHALL match inside a single word of the node's own searchable text — with each level matching only its own identity: an artist root against its name, an album leaf against its album title and year, and a cached track item against its track title. A score SHALL determine only whether a node matches; artist, album, and track order SHALL remain settled order.

An artist root SHALL remain visible when it matches or when any descendant matches, and matching paths SHALL be force-expanded without overwriting persistent expansion. A match SHALL bring no other row with it: a matched album leaf SHALL NOT drag its track children into the projection, and a track row SHALL appear in the filtered projection only through its own title match. An empty query SHALL show the complete tree. Opening a filter SHALL retain an anchor to the selected node; clearing or dismissing it SHALL restore that node when it still exists and restore persistent expansion. The corpus SHALL be only the current settled tree; filtering SHALL start no full-library fetch.

#### Scenario: An artist name surfaces its root
- **WHEN** the debounced query fuzzy-matches an artist's name
- **THEN** that artist root remains visible and is expanded for the filter session
- **AND** its album leaves stay hidden unless they match on their own album title or year

#### Scenario: A matched album does not drag its tracks
- **WHEN** the debounced query matches an album leaf that has cached track items
- **THEN** that leaf and its artist root remain visible without its track rows
- **AND** a track row appears only when the query matches its own track title

#### Scenario: Empty query shows the tree
- **WHEN** the filter is open with an empty query
- **THEN** the complete settled tree remains visible
- **AND** no corpus-loading state or Service request starts

#### Scenario: Dismiss restores persistent state
- **WHEN** filtering force-expanded a persistently collapsed root and the user dismisses the filter
- **THEN** that root returns to its persistent collapsed state
- **AND** the pre-filter selected node is restored when it still exists

### Requirement: Tree input stays inside existing ownership boundaries
The mounted Music destination SHALL remain the sole event boundary. Keyboard precedence SHALL remain in the Keyboard Router, and Grouped Music SHALL translate only eligible local chords into tree operations. Pointer gestures SHALL resolve artist, album, or track targets only from geometry retained by the latest completed tree render. A pointer gesture that resolves any tree row, including a context click or double-click, SHALL focus the Library panel before its resolved action runs, so interacting with the tree focuses the Library panel exactly as interacting with any other library list does. A wheel gesture claimed inside the latest painted tree rectangle SHALL likewise focus Library even when movement clamps at a boundary. The tree SHALL receive no Service client, Player owner, `App`, credentials, raw screen coordinate for deferred resolution, or second shell cursor.

#### Scenario: Current-frame click selects an artist
- **WHEN** a click lands on an artist root painted in the latest completed frame
- **THEN** current-frame tree geometry resolves that root before local selection changes
- **AND** no shell hit map or compatibility path resolves the click

#### Scenario: Clicking the tree focuses the Library panel
- **WHEN** the user clicks a tree row while another panel holds focus
- **THEN** the Library panel becomes the focused panel
- **AND** the resolved row becomes the tree's selected node

#### Scenario: Stale geometry cannot claim input
- **WHEN** tree content or geometry is configured for a new frame before that frame finishes rendering
- **THEN** prior row geometry claims no pointer target

### Requirement: Tree double-click expands an expandable node and plays a track
A double-click on a Grouped Music tree node SHALL toggle the persistent expansion of an expandable node: an artist root, or an album leaf with cached track children. This SHALL apply both with and without the local tree filter active; filter-forced visibility remains in effect until filtering closes. An album leaf without cached track children SHALL claim the gesture without changing state. None of those gestures SHALL open a Hero. A double-click on a track item SHALL use the same album-track resolution and playback executor as the tree's Enter chord, after the same populated-queue confirmation gate. When the target queue that playback would replace is populated, that activation SHALL ask for confirmation before replacing it, regardless of whether the target Player owner is local or directly controlled; an empty target queue SHALL skip the queue-replacement confirmation, while the save/discard protection that guards an unsaved local queue change SHALL still apply on the local path as it does for every other queue-replacing flow. Grouped Music tree double-click SHALL NOT perform the ordinary album-leaf activation or open the Library Hero overlay.

#### Scenario: Double-click expands an artist root
- **WHEN** the user double-clicks an artist root
- **THEN** the root's expansion toggles and no Hero opens

#### Scenario: Double-click expands an album leaf
- **WHEN** the user double-clicks an album leaf
- **THEN** the leaf's expansion toggles and no Hero opens

#### Scenario: Double-click a track plays it
- **WHEN** the user double-clicks a track item
- **THEN** playback of that item is requested
- **AND** a populated queue prompts for confirmation first

#### Scenario: Double-click a track with an empty queue plays immediately
- **WHEN** the user double-clicks a track item while the queue is empty
- **THEN** playback starts immediately: the empty target queue skips the queue-replacement confirmation prompt, while the save/discard protection for an unsaved local queue change still applies as it does for every other queue-replacing flow

#### Scenario: Filtered double-click keeps tree semantics
- **WHEN** the local tree filter is active and the user double-clicks a visible artist root, album leaf, or track item
- **THEN** the same expand, claim, or play behavior runs against current-frame filtered tree geometry
- **AND** no flat Inline Search activation or Hero opening replaces it

### Requirement: The integrated tree presents a coherent tree-specific surface
The Grouped Music tree SHALL paint once in the Library panel browser slot at every Panel mode. At the repository's existing Wide geometry fixture and smallest supported non-Wide Library-panel fixture, it SHALL use semantic theme roles and present a readable, intentional tree surface: a distinguishable artist and album hierarchy with visible expansion state, a clear focused-node treatment inside the browser rectangle it is supplied, group-relative zebra rhythm, release-year metadata when it fits, focused-title marquee behavior, and a usable scrollbar. The tree's selected-row extent, zebra rhythm, and scrollbar focus behavior are its own and need not match the canonical flat media lists. Every visible node row SHALL retain its hierarchy/expansion glyph and at least one title cell; narrower content SHALL truncate or marquee rather than overrun the browser rectangle. Album leaves SHALL obtain semantic state through the canonical music collapse and SHALL remain visually unplayed with no played or resume decoration; artist roots SHALL likewise use ordinary grouping semantics. No base frame, fallback media list, or second tree painter SHALL underpaint or overpaint its rows.

The change SHALL be accepted only after automated checks at the existing Wide and smallest supported non-Wide Library-panel fixtures and the end-of-implementation manual PoC evaluation across Wide, Narrow, Mini, and Library Hero overlay states. That evaluation is where residual visual roughness is worked out. Review findings are fixed against this tree-specific contract. The dependency SHALL be removed and this change SHALL not merge only if the tree's core behavior or ownership cannot be delivered through the crate's supported interfaces; a parallel bespoke tree renderer is not an acceptance fallback at any point.

#### Scenario: Focused node carries the tree's selected-row treatment
- **WHEN** the Library panel is focused and the tree paints its selected artist root or album leaf
- **THEN** that node paints the tree's focused-node treatment across the browser rectangle, overriding its zebra treatment
- **AND** no destination-defined raw colour or marker substitutes for a theme role or the treatment

#### Scenario: Music playback history does not decorate tree rows
- **WHEN** an artist root or album leaf is painted and its underlying Music data carries played or resume fields
- **THEN** the tree paints ordinary Music row semantics without played or resume decoration

#### Scenario: PoC evaluation finds rough edges
- **WHEN** automated checks pass and the end-of-implementation manual PoC evaluation finds visual roughness in the tree's hierarchy, focused-node treatment, metadata, marquee, scrollbar, or narrow-width handling
- **THEN** that roughness is worked out against this tree-specific contract
- **AND** only a defect that the crate's supported interfaces cannot address removes the dependency rather than merging a bespoke parallel renderer

## Purpose

Provide a shallow, directly navigable artist-and-album tree for Grouped Music, including group-scoped actions, without changing other library lists. (In-place fuzzy filtering is deferred from this PoC per user decision 2026-09-20; Grouped Music keeps its pre-change Inline Search behavior.)

## ADDED Requirements

### Requirement: Grouped Music projects one stable shallow tree
The Grouped Music browser SHALL present each settled artist group as a focusable artist root with its settled albums as leaf children. It SHALL preserve settled artist and album order. Artist identity SHALL use stable Service identity when available so equal display names remain distinct, and SHALL use a deterministic fallback identity when the Service supplies none. Stable artist identity SHALL come only from `ArtistItems` pairs carried on album/item payloads; IDs obtained from an `/Artists` listing SHALL NOT be mixed into artist keys, artwork requests, or artist-track queries. Album leaves SHALL retain their existing stable album targets.

The tree SHALL have one owner for its selected node, expansion, viewport, and current-frame hit geometry. Ordinary settled-catalog replacement SHALL preserve the selected node and expansion state by stable identity. When the selected node survives, its prior viewport row SHALL be preserved when projection bounds permit; otherwise the viewport SHALL apply only the minimum scroll needed to keep it visible and clamp at projection bounds. A responsive presentation change SHALL reuse that owner and apply the same visibility rule rather than copying state into another control.

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

### Requirement: Artist roots and album leaves have distinct navigation
Up, Down, `j`, and `k` SHALL move across visible artist roots and album leaves. Right on a collapsed artist root SHALL expand it; Right on an already expanded artist root SHALL enter its artist-track Workspace, opening the Library Hero overlay first in non-Wide geometry. Left SHALL collapse a focused expanded artist root, or move a focused album leaf to its artist parent. Enter on an artist root SHALL toggle expansion. Enter on an album leaf SHALL retain the existing album activation behavior. Page navigation SHALL operate on the tree's visible-node viewport and SHALL NOT inherit Heading-based canonical-list group jumps.

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

#### Scenario: Album activation is preserved
- **WHEN** the user presses Enter on an album leaf
- **THEN** the existing album Hero and album-track Workspace behavior is invoked

### Requirement: Artist actions resolve visible album descendants
Play, enqueue, shuffle, and context actions invoked on an artist root SHALL resolve to that root's album leaves in settled display order. All settled child albums are in scope regardless of expansion. Artist identities SHALL never be emitted as playback, queue, or context effect targets.

#### Scenario: Action on a collapsed artist
- **WHEN** an artist root is collapsed and the user invokes enqueue
- **THEN** every settled child album is materialized in display order
- **AND** only album identities cross the effect boundary

### Requirement: Tree input stays inside existing ownership boundaries
The mounted Music destination SHALL remain the sole event boundary. Keyboard precedence SHALL remain in the Keyboard Router, and Grouped Music SHALL translate only eligible local chords into tree operations. Pointer gestures SHALL resolve artist or album targets only from geometry retained by the latest completed tree render. The tree SHALL receive no Service client, Player owner, `App`, credentials, raw screen coordinate for deferred resolution, or second shell cursor.

#### Scenario: Current-frame click selects an artist
- **WHEN** a click lands on an artist root painted in the latest completed frame
- **THEN** current-frame tree geometry resolves that root before local selection changes
- **AND** no shell hit map or compatibility path resolves the click

#### Scenario: Stale geometry cannot claim input
- **WHEN** tree content or geometry is configured for a new frame before that frame finishes rendering
- **THEN** prior row geometry claims no pointer target

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

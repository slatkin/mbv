## ADDED Requirements

### Requirement: The non-Wide library list is the Wide Browser-pane list

The non-Wide library list SHALL be the Wide Browser-pane list at the non-Wide pane width: it SHALL
resolve the same `LibraryPanel` list-box fill and the same `MainContentBox` stripe pair, and SHALL
carry no per-geometry body fill or scrollbar-column fill. Its list box SHALL be filled by the panel
that composes the surface, as the Wide Browser-pane list's is, so the Wide list policy is the only
one the non-Wide library uses. A list that carries no body fill SHALL keep resolving the scrollbar
column through the owning-surface identity of its selected-row surface, as before, so the Queue is
unaffected.

#### Scenario: The non-Wide library list uses the Wide Browser-pane policy

- **WHEN** a non-Wide library list renders in either focus state
- **THEN** its list box carries the `LibraryPanel` fill and its alternating rows carry the
  `MainContentBox` fill for that state
- **AND** no body fill or scrollbar-column fill is set on its paint policy

#### Scenario: Scrollbar column parity with the Wide Browser-pane list

- **WHEN** a non-Wide library list overflows and paints its scrollbar column
- **THEN** the column carries the same fill the Wide Browser-pane list's column carries
- **AND** it does not carry a library-body fill

#### Scenario: Lists without their own body fill are unchanged

- **WHEN** a Wide library list or the Queue list paints its scrollbar column
- **THEN** the column resolves the owning-surface identity of its selected-row surface
- **AND** its painted fill is unchanged from before this capability

## REMOVED Requirements

### Requirement: The non-Wide library list owns the surface under it

**Reason**: The non-Wide list no longer owns a body fill. The browser-pane composition paints the
list box for both geometries, so the list carries no body or scrollbar fill.

**Migration**: The panel that composes the browser pane paints the list box; the list uses the Wide
policy with no body override.

### Requirement: Non-Wide library lists use the Wide browser list's pair

**Reason**: Superseded by "The non-Wide library list is the Wide Browser-pane list", which states the
whole policy identity rather than only the stripe pair.

**Migration**: Resolve the non-Wide library list through the Wide Browser-pane policy; no
non-Wide-specific stripe or body pair remains.
## ADDED Requirements

### Requirement: Home composes canonical list controls
Home SHALL compose a persistent `InlineMediaBrowser` for the inline section and a persistent `WideMediaList` where the approved Wide arrangement requires a fixed one-column rail. Section identity SHALL remain keyed by `pref_key` and restored through `restore_section`. Home SHALL keep exactly one active section with one flat cursor and scroll position owned by the active control; only the active section's rows SHALL be projected into that control. Ordinary refresh SHALL preserve stable target and locally clamp without adopting parent cursor/scroll. Breakpoint or discrete navigation transitions SHALL use one `ViewportAnchor`, with no per-section cursor cache and no App-wide interaction mirror.

#### Scenario: Home refresh preserves section state
- **WHEN** the active Home section refreshes or the active variant changes
- **THEN** refresh preserves or clamps the control-owned stable target locally
- **AND** a variant transition performs one target/offset `ViewportAnchor` handoff
- **AND** `pref_key`/`restore_section`, images, and workspace effects remain shell/parent-owned.

### Requirement: Feeds projects structural rows
The Feeds Service/tab SHALL project FeedAgeGroup/date labels as non-selectable `Heading` rows and separators as non-selectable `Spacer` rows as canonical-list content. Only media `Item` rows SHALL enter selectable indexing. The subscription/group selector pills and the watched selector SHALL remain parent-owned chrome outside the canonical control and SHALL NOT be projected as canonical rows.

#### Scenario: Structural rows do not capture selection
- **WHEN** a user moves through a grouped Feeds list
- **THEN** cursor movement skips headings and spacers and activation resolves the selected FeedEntry target.

### Requirement: Canonical source of truth owns row presentation
Migrated Home and Feeds rows SHALL use the canonical row model and painter. The deferred #623 two-space row-indent correction SHALL be implemented at that source of truth, not by destination-specific offsets.

#### Scenario: Wide Feeds remains one column
- **WHEN** the Feeds Service/tab is rendered at an accepted Wide breakpoint
- **THEN** it uses one column with the accepted #623 framing/background and selected-row semantics.

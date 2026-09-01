## ADDED Requirements

### Requirement: Home composes canonical list controls
Home SHALL compose `InlineMediaBrowser` for inline sections and `WideMediaList` where the approved Wide arrangement requires a fixed one-column rail. Section identity SHALL remain keyed by `pref_key` and restored through `restore_section`; each section SHALL retain its own cursor and scroll.

#### Scenario: Home refresh preserves section state
- **WHEN** a Home section refreshes or the active variant changes
- **THEN** its selected target, cursor, and scroll are preserved or clamped by the canonical control, while `pref_key`/`restore_section`, images, and workspace effects remain shell/parent-owned.

### Requirement: Feeds projects structural rows
The Feeds Service/tab SHALL project group labels as non-selectable `Heading` rows and separators as non-selectable `Spacer` rows. Only media `Item` rows SHALL enter selectable indexing.

#### Scenario: Structural rows do not capture selection
- **WHEN** a user moves through a grouped Feeds list
- **THEN** cursor movement skips headings and spacers and activation resolves the selected FeedEntry target.

### Requirement: Canonical source of truth owns row presentation
Migrated Home and Feeds rows SHALL use the canonical row model and painter. The deferred #623 two-space row-indent correction SHALL be implemented at that source of truth, not by destination-specific offsets.

#### Scenario: Wide Feeds remains one column
- **WHEN** the Feeds Service/tab is rendered at an accepted Wide breakpoint
- **THEN** it uses one column with the accepted #623 framing/background and selected-row semantics.

### Requirement: Parent and embedded control ownership is explicit
The mounted parent SHALL own application effects, section/group state, images/workspaces, overlays, and mouse subscription; the embedded control SHALL own cursor, scroll, replacement geometry, and child hit regions. Keyboard routing remains centralized.

#### Scenario: Mouse target is translated once
- **WHEN** the parent receives a point in a list
- **THEN** it delegates to the embedded control's hit regions and translates the resulting typed target exactly once.

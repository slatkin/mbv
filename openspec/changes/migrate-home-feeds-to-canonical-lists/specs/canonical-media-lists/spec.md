## ADDED Requirements

### Requirement: Home composes canonical list controls
Home SHALL compose `InlineMediaBrowser` for the inline section and `WideMediaList` where the approved Wide arrangement requires a fixed one-column rail. Section identity SHALL remain keyed by `pref_key` and restored through `restore_section`. Home SHALL keep exactly one active section with one flat cursor and scroll position at a time; only the active section's rows SHALL be projected into the canonical control. The active-section cursor and scroll SHALL be carried through the control with a `ViewportAnchor` for refresh and breakpoint handoff, with no per-section cursor cache and no App-wide interaction mirror.

#### Scenario: Home refresh preserves section state
- **WHEN** the active Home section refreshes or the active variant changes
- **THEN** the single active-section selected target, cursor, and scroll are preserved or clamped by the canonical control via `ViewportAnchor`, while `pref_key`/`restore_section`, images, and workspace effects remain shell/parent-owned.

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

### Requirement: Parent and embedded control ownership is explicit
The mounted destination parent / AppComponent SHALL own the mouse subscription, raw gesture recognition and delivery, arbitration, blocking-overlay behavior, and `MouseGestureState`, in addition to application effects, section/group state, images/workspaces, and overlays. The embedded canonical control SHALL own only view-populated `HitRegions<Target>` and list-local updates, and it also retains cursor, scroll, and replacement geometry. The parent SHALL delegate point resolution to the child; there SHALL be no second gesture recognizer in the child. Keyboard resolution stays solely in `router.rs`/`key_policy.rs`.

#### Scenario: Mouse target is translated once
- **WHEN** the parent receives a point in a list
- **THEN** it delegates to the embedded control's hit regions and translates the resulting typed target exactly once.

# canonical-media-lists Specification

## Purpose

Provide reusable embedded TuiRealm list controls with one owner for list interaction and geometry across the first canonical media-list migration slice.

## ADDED Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, optional trailing text, and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. Heading and Spacer SHALL be excluded from selectable-target indexing. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

#### Scenario: Queue-like progress is presented safely
- **WHEN** a parent supplies active progress
- **THEN** the control receives only a bounded percentage
- **AND** playback and queue authority remain with the parent/shell

#### Scenario: Structural rows are displayed only
- **WHEN** a Heading or Spacer is rendered
- **THEN** it occupies display geometry
- **AND** it cannot be selected or activated

### Requirement: WideMediaList owns fixed-row mechanics
`WideMediaList<Target>` SHALL be an embedded plain TuiRealm `Component` that owns cursor, scroll, viewport, fixed-height one-column row placement, semantic painting delegation, scrollbar, movement, clamping, and view-populated `HitRegions<Target>`. It SHALL support Hero-on-left rails and later Queue fixed rows, but SHALL NOT implement Inline replacement or a non-hero two-column policy.

#### Scenario: Wide TV rail composes the control
- **WHEN** the TV surface is Hero-on-left
- **THEN** its right rail is painted and interacted with by one `WideMediaList`
- **AND** the parent retains workspace, hero, images, and effects

### Requirement: InlineMediaBrowser owns selected-row replacement
`InlineMediaBrowser<Target>` SHALL be an embedded plain TuiRealm `Component` owning one-column placement, selection visibility, variable-height selected-row replacement admission, ordinary-row fallback when replacement cannot fit, and matching parent/child hit geometry. It SHALL be distinct from Inline Search and SHALL not become a second mounted identity or router.

#### Scenario: A selected row is replaced
- **WHEN** the selected item fits the Inline presentation
- **THEN** its ordinary row is replaced once by the detail block
- **AND** there is no blank duplicate row and the parent target remains stable

### Requirement: Responsive handoff preserves an explicit anchor
At Wide↔Narrow transitions the parent SHALL hand off `ViewportAnchor { selected_target, selected_row_offset }`, with offset measured from viewport top to the selected ordinary row. The receiving control SHALL preserve the offset when possible and clamp it to its viewport otherwise. Ordinary refreshes SHALL preserve target and locally clamp without adopting shell cursor/scroll mirrors.

#### Scenario: TV re-anchors across breakpoints
- **WHEN** TV changes Wide→Narrow→Wide
- **THEN** characterization records the existing selected target, cursor, scroll, and screen-row offset
- **AND** replacement matches that evidence unless separately approved

### Requirement: Parent owns mouse delivery and child owns row hits
The mounted parent SHALL own the TuiRealm mouse subscription and `MouseGestureState`; the embedded control SHALL populate and resolve `HitRegions<Target>` from its latest view. The parent SHALL delegate list points and translate the result to typed destination requests. No child subscription, global hit map, second router, or duplicate row-coordinate path is permitted. Parent-owned pills, workspace controls, overlays, and keyboard precedence remain parent/router-owned.

#### Scenario: A list click resolves painted geometry
- **WHEN** a mouse gesture lands on a painted list row
- **THEN** the parent delegates to the embedded control's regions
- **AND** the stable target is returned without recomputing row coordinates

### Requirement: Named destinations compose without changing provider authority
The slice SHALL compose generic Emby catalogs, Movies, Emby homevideos/podcast libraries, narrow TV Series browsing, and Wide TV's right rail. Provider workspaces, images, effects, persistence, Service and Player authority, and typed message translation SHALL remain in their existing parents/shell. Non-hero two-column browsers SHALL retain their policy.

#### Scenario: One painter is active
- **WHEN** a listed destination is rendered at its applicable breakpoint
- **THEN** exactly one list painter runs
- **AND** the old loop is not run as an underpaint

### Requirement: Migration is evidence-gated
Before wiring, the implementation SHALL provide focused characterization for current TV Wide→Narrow→Wide handoff and re-home `render_plain_rows` without unapproved output changes. Visual correction and user live confirmation SHALL precede any UI test modification or addition. After confirmation, affected surfaces SHALL provide focused rendered evidence with representative metadata/state/image fixtures, manual/live Wide/Narrow evidence, one-painter evidence, and the 800-line file-size gate.

#### Scenario: Visual approval precedes tests
- **WHEN** the implementation changes a visual surface
- **THEN** the user confirms the live result first
- **AND** only afterward are UI tests added or modified

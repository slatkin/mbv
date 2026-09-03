# canonical-media-lists Specification

## Purpose

Provide reusable embedded TuiRealm list controls with one owner for list interaction and geometry across the first canonical media-list migration slice.

## Requirements

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
`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm `Component` that owns cursor, scroll, viewport, fixed-height one-column row placement, semantic painting delegation, scrollbar, movement, clamping, and internal row geometry for painting and scrolling. It SHALL support Hero-on-left rails and later Queue fixed rows, but SHALL NOT implement Inline replacement or a non-hero two-column policy. It SHALL express letter grouping through `MediaListRow::Heading`/`Spacer` rows. An applicable Wide Browser path SHALL delegate to this control and SHALL NOT reach `render_generic_movies_home_video_rows_with_ctx` or either painter it routes to (`render_letter_grouped_rows`, `render_plain_rows`); the absence of a `render_plain_rows` call alone SHALL NOT be accepted as compliance. It SHALL expose no mouse hit-resolution API; `restore-mouse-support` (#638) adds `HitRegions<Target>` later.

#### Scenario: Wide TV rail composes the control
- **WHEN** the TV surface is Hero-on-left
- **THEN** its right rail is painted and interacted with by one `WideMediaList`
- **AND** the parent retains workspace, hero, images, and effects

### Requirement: InlineMediaBrowser owns selected-row replacement
`InlineMediaBrowser<Target>` SHALL be a persistent embedded plain TuiRealm `Component` owning one-column placement, selection visibility, variable-height selected-row replacement admission, ordinary-row fallback when replacement cannot fit, and its internal row and replacement geometry for painting and scrolling. It SHALL be distinct from Inline Search, SHALL NOT be constructed during a render pass, and SHALL not become a second mounted identity or router. It SHALL expose no mouse hit-resolution API; `restore-mouse-support` (#638) adds `HitRegions<Target>` later.

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

### Requirement: Named destinations compose without changing provider authority
The slice SHALL compose persistent `WideMediaList` controls in the applicable Hero-on-left paths and persistent `InlineMediaBrowser` controls in the applicable Narrow paths for hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, and TV Series browsing. Non-hero two-column Emby catalogs SHALL keep their existing two-column arrangement policy and SHALL NOT be forced onto either canonical control. Provider workspaces, images, effects, persistence, Service and Player authority, and typed message translation SHALL remain in their existing parents/shell.

#### Scenario: One painter is active
- **WHEN** a listed destination is rendered at its applicable breakpoint
- **THEN** exactly one list painter runs
- **AND** the old loop is not run as an underpaint

### Requirement: Migration is accepted as one verified slice
The implementation, representative stateful and rendered tests, automated gates, review, and acceptance SHALL form one uninterrupted slice. There SHALL be no pre-test visual-approval checkpoint. Affected surfaces SHALL provide metadata/state/image-bearing rendered evidence, stateful target-and-anchor evidence, source-level one-painter evidence, manual/live Wide/Narrow evidence, and the 800-line file-size gate before acceptance. A visual defect found during review or acceptance SHALL be treated as a bug, fixed, and followed by rerunning the affected tests and gates.

#### Scenario: Evidence precedes acceptance
- **WHEN** the implementation changes a visual surface
- **THEN** representative tests and automated gates run before review and acceptance
- **AND** live Wide/Narrow review remains part of acceptance
- **AND** any defect found there is fixed before the slice is accepted

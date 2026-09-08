## MODIFIED Requirements

### Requirement: WideMediaList owns fixed-row mechanics

`WideMediaList<Target>` SHALL be a persistent embedded plain TuiRealm `Component` that owns cursor, scroll, viewport, fixed-height one-column row placement, semantic painting delegation, scrollbar, movement, clamping, and internal row geometry for painting, scrolling, and point resolution. Its component `view` SHALL be the list-row painting entry point used by destination parents. It SHALL support Wide hero rails and Queue fixed rows, but SHALL NOT implement Inline replacement or a non-hero two-column policy. It SHALL express letter grouping through `MediaListRow::Heading`/`Spacer` rows. An applicable Wide Browser path SHALL delegate to this control and SHALL NOT reach `render_generic_movies_home_video_rows_with_ctx` or either painter it routes to (`render_letter_grouped_rows`, `render_plain_rows`); the absence of a `render_plain_rows` call alone SHALL NOT be accepted as compliance.

#### Scenario: Wide TV rail composes the control

- **WHEN** the TV surface is Wide hero

- **THEN** its left rail is painted and interacted with by one persistent `WideMediaList`

- **AND** the parent retains workspace, hero, images, gesture recognition, and effects

#### Scenario: A Wide row is pointed

- **WHEN** the destination parent delegates a recognized pointer gesture inside the painted list rectangle

- **THEN** the control resolves the row from the same internal flow geometry its `view` painted

- **AND** it returns the stable opaque target without publishing a duplicate row map

### Requirement: InlineMediaBrowser owns selected-row replacement

`InlineMediaBrowser<Target>` SHALL be a persistent embedded plain TuiRealm `Component` owning one-column placement, selection visibility, variable-height selected-row replacement admission, ordinary-row fallback when replacement cannot fit, semantic painting delegation, and its internal row and replacement geometry for painting, scrolling, and point resolution. Its component `view` SHALL paint the ordinary row flow and reserve the admitted replacement region; the destination parent SHALL paint only its provider-owned detail payload in that region. It SHALL be distinct from Inline Search, SHALL NOT be constructed during a render pass, and SHALL NOT become a second mounted identity, subscription, focus target, gesture recognizer, or router.

#### Scenario: A selected row is replaced

- **WHEN** the selected item fits the Inline presentation

- **THEN** its ordinary row is replaced once by the detail block

- **AND** there is no blank duplicate row and the parent target remains stable

#### Scenario: An Inline row is pointed

- **WHEN** the destination parent delegates a recognized pointer gesture inside the painted Inline list rectangle

- **THEN** the control resolves ordinary and replacement targets from the same flow its `view` painted

- **AND** replacement continuation rows do not become duplicate targets

### Requirement: Responsive handoff preserves an explicit anchor

At Wide↔Normal transitions the destination parent SHALL identify the previously active embedded control, obtain exactly one `ViewportAnchor { selected_target, selected_row_offset }` from it, and apply that anchor once to the incoming control, with offset measured from viewport top to the selected ordinary row. Only the control painted for the current presentation SHALL receive movement or selection commands. Content MAY be projected to both controls, but ordinary refreshes SHALL preserve each control's own target and locally clamp without copying cursor or scroll between controls or adopting shell or parent mirrors.

#### Scenario: TV re-anchors across breakpoints

- **WHEN** TV changes Wide→Normal→Wide

- **THEN** each transition transfers the selected stable target and ordinary-row screen offset exactly once

- **AND** only the control painted at each presentation receives intervening movement

#### Scenario: Ordinary refresh does not synchronize controls

- **WHEN** refreshed content arrives without a breakpoint or discrete navigation transition

- **THEN** each persistent control preserves or clamps its own local state

- **AND** neither control copies cursor or scroll from its sibling, parent, or shell

### Requirement: Home composes canonical list controls

Home SHALL compose a persistent `InlineMediaBrowser` for the inline section and a persistent `WideMediaList` where the approved Wide arrangement requires a fixed one-column rail. Section identity SHALL remain keyed by `pref_key` and restored through `restore_section`. Home SHALL keep exactly one active section and one active embedded control; only that active control SHALL receive movement, selection, painting, and pointer-resolution delegation. Only the active section's rows SHALL be projected into the controls. Ordinary refresh SHALL preserve stable target and locally clamp without adopting parent cursor/scroll. Breakpoint or discrete navigation transitions SHALL use one `ViewportAnchor`, with no per-section cursor cache, lockstep control mutation, or App-wide interaction mirror.

#### Scenario: Home refresh preserves section state

- **WHEN** the active Home section refreshes

- **THEN** refresh preserves or clamps each control's stable target locally

- **AND** `pref_key`/`restore_section`, images, and workspace effects remain shell/parent-owned

#### Scenario: Home changes presentation

- **WHEN** Home changes between Wide and Inline presentation

- **THEN** the outgoing active control hands one target/offset `ViewportAnchor` to the incoming control

- **AND** later movement changes only the incoming active control

### Requirement: Named destinations compose without changing provider authority

The slice SHALL compose persistent `WideMediaList` controls in applicable Wide hero paths and persistent `InlineMediaBrowser` controls in applicable Normal paths for Home, hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, TV Series, grouped Music, Audiobookshelf Podcast and Book, Feeds, and Queue. No listed destination SHALL construct a canonical control during rendering or retain a parent-owned live cursor, scroll, row map, or ordinary-render writeback for a canonical list. Non-hero two-column Emby catalogs SHALL keep an explicitly isolated grid state and existing two-column arrangement policy and SHALL NOT be forced onto either canonical control. Inline Search, Search sidebar results, Playlists/open-playlist rows, Settings, and Sessions remain outside this capability. Provider workspaces, images, effects, persistence, Service and Player authority, mouse gesture recognition, and typed message translation SHALL remain in their existing parents/shell.

#### Scenario: One painter is active

- **WHEN** a listed destination is rendered at its applicable presentation

- **THEN** exactly one embedded control paints and owns row geometry for the canonical list rectangle

- **AND** the parent paints only separately owned chrome, hero/detail payload, or workspace regions

#### Scenario: Non-hero grid remains isolated

- **WHEN** a non-hero two-column Emby catalog is rendered

- **THEN** its grid-specific position and geometry remain outside the canonical controls

- **AND** canonical paths cannot read or write that grid state

### Requirement: Provider destinations compose canonical media controls

Grouped Music album browsing and Audiobookshelf Podcast show browsing and Book browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose persistent `WideMediaList` controls for Wide rails and persistent `InlineMediaBrowser` controls for Normal selected-row replacement where the arrangement permits. The controls SHALL remain embedded beneath the mounted destination component and SHALL own live list selection rather than being seeded from a parent selection during ordinary rendering. Provider workspaces, images, selectors, surname buckets, episode/chapter focus, effects, and typed intent translation remain parent-owned.

#### Scenario: Music groups retain provider authority

- **WHEN** a grouped Music album surface is rendered or navigated

- **THEN** the active canonical control owns album-row selection, scroll, painting, and row geometry

- **AND** grouping, track authority, images, and playback intents remain Music-owned

- **AND** no parent album cursor/scroll mirror or ordinary-render writeback runs

#### Scenario: Audiobookshelf shows compose without losing episodes

- **WHEN** a Podcast library is shown Wide or Normal

- **THEN** one persistent show-list control owns the active list presentation

- **AND** the selected show's episode workspace remains provider-owned, including episode filtering, images, and typed playback intents

#### Scenario: Audiobookshelf books compose without duplicate detail

- **WHEN** a Book library is shown Wide

- **THEN** one persistent book-list control paints ordinary fixed-height rows in the rail while the selected book's provider detail workspace renders separately

- **AND** no selected-row replacement, render-time list construction, or parent-owned book-list cursor/scroll is used in the Wide rail

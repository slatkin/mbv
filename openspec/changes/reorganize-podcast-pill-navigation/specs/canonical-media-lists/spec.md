## MODIFIED Requirements

### Requirement: Provider destinations compose canonical media controls

Grouped Music album and track browsing, Audiobookshelf Podcast episode browsing, and Audiobookshelf Book and chapter/audio-part browsing SHALL prepare provider-owned content as canonical selectable `Item`, non-selectable `Heading`, and `Spacer` rows and compose shared Wide or Inline presentations where their arrangements require them. The controls SHALL remain embedded beneath the mounted destination component. Provider detail workspaces, images, selectors, surname buckets, filters, effects, and typed intent translation SHALL remain parent-owned; every media-row flow's cursor, scroll, authoritative selected target, row-local behavior, and retained row geometry SHALL remain in its shared canonical owner.

#### Scenario: Music groups retain provider authority

- **WHEN** a grouped Music album or track surface is rendered or navigated
- **THEN** album and track rows use shared canonical ownership and delegation
- **AND** grouping, active-pane focus, images, content lookup, and playback intents remain Music-owned
- **AND** no track cursor mirror, second list painter, or App-owned interaction mirror runs.

#### Scenario: Audiobookshelf shows compose without losing episodes

- **WHEN** an Audiobookshelf podcast library is shown Wide or Normal
- **THEN** episode rows use shared canonical ownership and delegation
- **AND** the pill row's selection and filtering, active-pane focus, images, content lookup, and typed playback intents remain Audiobookshelf-podcast-owned
- **AND** episode rows are not reseeded or reselected during painting.

#### Scenario: Audiobookshelf books compose without duplicate detail

- **WHEN** a Book library is shown Wide or Normal
- **THEN** book and chapter/audio-part rows use shared canonical ownership and delegation
- **AND** surname buckets, active-pane focus, images, content lookup, and absolute chapter-seek intents remain Book-owned
- **AND** chapter/audio-part rows are not reseeded or reselected during painting.

### Requirement: Audiobookshelf geometry has complete breakpoint fallbacks

Audiobookshelf Book surfaces SHALL use the shared Wide hero or Inline arrangement at the established Wide/Normal breakpoints. Audiobookshelf Podcast surfaces SHALL use the shared Wide hero when it fits and their ordinary episode rows at every other geometry, because the podcast tab has no Inline hero. Both SHALL preserve the short-height fallback and hand off stable selected target and viewport anchor across breakpoint changes. Non-list repairs required to make the composition correct SHALL live in shared arrangements or the owning destination component, not a bespoke exception.

#### Scenario: Wide and short layouts are deterministic
- **WHEN** terminal width/height crosses the Wide threshold or the short-height guard
- **THEN** the surface selects the defined Wide, Normal, or short fallback arrangement
- **AND** the selected target, row offset, images, framing, and focus remain stable.

### Requirement: One shared owner supports list-local extension

Every in-scope logical media-row flow SHALL have exactly one shared owner for row content order, selectable-target indexing, cursor, scroll, authoritative selected-row identity, row-local interaction state, and row-local behavior. Its fixed-row presentation SHALL operate on that owner in Wide and non-Wide geometry rather than synchronize independent copies. A purely list-local state transition and row decoration SHALL be implementable in the shared canonical media-list subsystem without changing destination production code.

The in-scope flows SHALL be Queue slots; Home rows; generic Emby catalog rows; Movies and the Emby homevideos feed view; Grouped Music albums and tracks; TV series and episodes; Feeds entries; Audiobookshelf Podcast episode rows; and Audiobookshelf Book titles and chapter/audio-part rows.

Parent destinations SHALL retain Service content, stable-target-to-domain lookup, active pane and component focus, section/group/filter/bucket/season/scope chrome, Workspaces, loading, images, effects, persistence, and provider-specific typed intent translation. They SHALL NOT retain a second row cursor, row scroll, row-local membership or range state, row hit map, or authoritative selected-row identity.

#### Scenario: A list-local behavior has one implementation site

- **WHEN** a developer adds a purely list-local state transition and visual decoration
- **THEN** the production change is confined to the shared canonical media-list state/behavior owner and shared row painter
- **AND** no destination production file changes
- **AND** browser and provider-Workspace media rows receive the behavior through their existing composition

#### Scenario: Responsive presentation does not synchronize local state

- **WHEN** one logical list changes between Wide and non-Wide geometry
- **THEN** the same shared owner and fixed-row presentation remain active
- **AND** no cursor, scroll, membership, range, or other row-local state is copied between controls

#### Scenario: Parent authority remains outside the row owner

- **WHEN** a destination changes a section, group, filter, surname bucket, season, queue scope, focused pane, or Library Hero overlay state
- **THEN** the destination or owning Panel remains authoritative for that chrome, Workspace, or overlay state
- **AND** it projects the resulting media rows into the shared owner
- **AND** provider-specific effects remain typed destination intents

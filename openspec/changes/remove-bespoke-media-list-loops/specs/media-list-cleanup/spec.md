# Media-list cleanup

## ADDED Requirements

### Requirement: cleanup waits for all destination slices
The cleanup SHALL begin only after the canonical foundation lands and the
Home/Feeds, Music/Audiobookshelf, and Queue sibling destination slices are all
accepted. Their SHAs SHALL be recorded at implementation-issue time, not pinned
in this change. This cleanup SHALL be a distinct deletion/documentation-only PR
targeting `feat/migrate-tui-to-tuirealm` and SHALL make no visual corrections.

#### Scenario: destination work stays owned
- **WHEN** a destination or visual defect is found during cleanup
- **THEN** it is routed to that destination slice and is not fixed here

### Requirement: obsolete loops have one canonical replacement
The implementation SHALL delete each obsolete cross-family `render_*_rows`
loop and bespoke list painter only after staged ast-grep and grep proof of zero
production callers, and SHALL leave canonical controls as the sole list path
for each migrated destination. Non-hero two-column arrangements SHALL remain.

#### Scenario: no stale painter remains
- **WHEN** the cleanup is reviewed
- **THEN** every named obsolete loop/painter has zero production references and
each reachable destination has exactly one canonical body painter

### Requirement: obsolete interaction geometry is removed safely
The implementation SHALL delete old selection, scrolling, row-hit, and cursor
geometry only after zero production consumers are proven. Canonical controls
retain local viewport and hit-region ownership. Queue SHALL remain fixed-row-only.

#### Scenario: Queue stays fixed-row-only
- **WHEN** Queue is rendered at any supported breakpoint
- **THEN** it uses the fixed-row canonical control and does not gain inline hero,
Hero-on-left, or responsive handoff behavior

### Requirement: layout fields are removed only at zero references
`AppLayout::main` left/hero/selector/wide-family fields SHALL be removed only
when ast-grep/grep proves no production readers, writers, or geometry-dependent
callers. Test and documentation references SHALL be tracked separately.

#### Scenario: two-column policy survives
- **WHEN** a non-hero browser is rendered
- **THEN** its existing two-column arrangement remains unchanged

### Requirement: terminology and architecture stay coherent
Documentation SHALL reconcile ADR 0022, the interactive-surface ledger,
`CONTEXT.md`, and frontend guidance with final `WideMediaList` and
`InlineMediaBrowser` ownership and terminology, while preserving the Feeds
Service/tab and Emby homevideos feed-view distinction. No destination-family
work SHALL be folded into this change.

#### Scenario: ownership terminology is unambiguous
- **WHEN** the reconciled docs describe the migrated list surfaces
- **THEN** they name `WideMediaList` and `InlineMediaBrowser` owners consistently
  and distinguish both feed meanings

### Requirement: visual and staged verification gates are explicit
Before changing UI tests or stale UI docs, a user SHALL explicitly verify live
output at narrow 60x20 and Wide 120x40/140x30. The implementation plan SHALL
require ast-grep, grep, cargo checks, file-size, formatting, relevant tests, and
a final whole-tree zero-reference check after those updates.

#### Scenario: visual evidence covers breakpoints
- **WHEN** cleanup acceptance is sought
- **THEN** live evidence covers 60x20, 120x40, and 140x30 and confirms no
underpaint, geometry drift, or lost canonical list behavior

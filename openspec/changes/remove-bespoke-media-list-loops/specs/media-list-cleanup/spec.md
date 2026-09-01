# Media-list cleanup

## ADDED Requirements

### Requirement: obsolete loops have one canonical replacement
The implementation SHALL delete each obsolete cross-family `render_*_rows`
loop and bespoke list painter only after ast-grep and grep prove zero callers,
and SHALL leave the canonical media-list controls as the sole list path for
each migrated destination. Non-hero two-column arrangements SHALL remain.

#### Scenario: no stale painter remains
- **WHEN** the cleanup is reviewed
- **THEN** every named obsolete loop/painter has zero references and each
  reachable destination has exactly one canonical body painter

### Requirement: obsolete interaction geometry is removed
The implementation SHALL delete old selection, scrolling, row-hit, and cursor
geometry only when zero references are proven; canonical controls retain local
viewport and hit-region ownership. Queue SHALL remain fixed-row-only.

#### Scenario: Queue stays fixed-row-only
- **WHEN** Queue is rendered at any supported breakpoint
- **THEN** it uses the fixed-row canonical control and does not gain inline hero,
  Hero-on-left, or responsive handoff behavior

### Requirement: layout fields are removed only at zero references
`AppLayout::main` left/hero/selector/wide-family fields SHALL be removed only
when ast-grep/grep proves no consumers, writers, or geometry-dependent callers.

#### Scenario: two-column policy survives
- **WHEN** a non-hero browser is rendered
- **THEN** its existing two-column arrangement remains unchanged

### Requirement: terminology and architecture stay coherent
Documentation SHALL reconcile ADR 0022, the interactive-surface ledger,
`CONTEXT.md`, and frontend guidance with canonical list ownership, while
preserving the Feeds Service/homevideos distinction and recording this cleanup
as downstream of all four slices.

#### Scenario: no scope confusion
- **WHEN** the docs describe feeds or list cleanup
- **THEN** Feeds Service/tab and Emby homevideos feed view are named separately
  and no destination-family work is folded into this change

### Requirement: visual and verification gates are explicit
Before UI test edits, a user SHALL visually verify narrow 60x20 and Wide
120x40/140x30 output. The implementation plan SHALL require ast-grep, grep,
cargo checks, file-size, and fmt gates.

#### Scenario: visual evidence covers breakpoints
- **WHEN** cleanup acceptance is sought
- **THEN** live evidence covers 60x20, 120x40, and 140x30 and confirms no
  underpaint, geometry drift, or lost canonical list behavior

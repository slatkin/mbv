## Why

Inline Search currently paints as a separately mounted overlay whose area is inferred from destination geometry, while each destination owns the media list it displays. This split places search over the wrong Wide pane, leaves the input invisible on some destinations, and has regressed existing query, ordering, navigation, dismissal, and sizing requirements.

## What Changes

- Make Inline Search a shared embedded capability of the Interactive Component that owns each searchable Emby destination list.
- Give the destination sole responsibility for placing and painting the search input and filtered rows in Normal and Wide presentations.
- Preserve one search session, including query and navigation position, when a responsive transition changes the Interactive Component that owns the same destination.
- Restore the existing full-corpus fuzzy matching, score ordering, keyboard navigation, activation, dismissal, loading, and three-row input contracts.
- Remove the separately mounted Inline Search overlay, its paint-derived area selection, and Browser-only search-box projection.
- Keep full-library loading, recursive album index construction, navigation mutation, and activation effects in the shell.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `inline-library-search`: Require an open search session to remain visible, retain its query and navigation position, and continue filtering the same destination across Normal/Wide responsive transitions.

## Impact

- Interactive Components: Browser, Music workspace, TV workspace, shared embedded list/search controls, typed shell requests, and keyboard handling.
- Shell: Inline Search lifecycle/content projection, responsive owner handoff, full-library and recursive-album completion handling, and activation dispatch.
- Rendering: destination arrangements and the shared Inline Search input/results painter.
- Tests and architecture documentation: component buffer/input tests, shell `Application::tick()` integration tests, breakpoint-transition coverage, and the interactive-surface ledger.
- No new dependencies or public protocol changes.

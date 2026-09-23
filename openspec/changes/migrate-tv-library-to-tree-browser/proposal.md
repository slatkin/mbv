# Proposal

## Why

TV's show browser still uses a flat MediaList and a separate season/episode Workspace, although the shared TreeBrowser now supports nested destinations. Moving the show browser to the shared owner makes show → season → episode navigation available inline without replacing the existing Hero or duplicating tree mechanics in TV.

## What Changes

- Add optional non-selectable group headings to the shared TreeBrowser flow. Ungrouped Music continues to render and behave as before.
- Migrate TV's show-browsing list to a show → season → episode TreeBrowser; keep its existing alphabet groupings as non-selectable headings.
- Keep the TV Hero, season pills, and episode Workspace unchanged: inline tree episodes deliberately duplicate Workspace episodes.
- Keep Latest and Upcoming as flat episode lists and preserve their direct-play and Hero rules; do not force those modes through the hierarchy.
- Preserve TV content-mode pills, Inline Search, navigation effects, and stable selection across refresh and geometry changes.

## Capabilities

### New Capabilities

- `tv-tree-browsing`: TV show-mode hierarchy, duplicate Hero Workspace, and mode-specific browser behavior.

### Modified Capabilities

- `shared-list-components`: TreeBrowser can project structural group headings without giving them selection, expansion, marks, or actions.
- `canonical-media-lists`: TV show-mode browser rows use the tree rather than the flat media-list presentation; Latest/Upcoming and Hero Workspace episodes remain flat. A delta for this capability is still required before implementation.

## Impact

Shared TreeBrowser data, flow, painter, and pointer operations; TV content owner and shell projections for show/season/episode data; Library Panel list-slot integration; focused component, buffer, and mounted-tick tests. No new dependency or Service protocol. Planning requires incorporating the completed `rework-tv-library-pills` code and syncing its spec deltas before TV migration; its behavior is currently present in change artifacts but not this checkout's TV source. Do not overwrite that change's modes or selectors.

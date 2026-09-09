## Why

TV Series still duplicates Wide series-list position in its parent component and resolves series and episode pointer hits from re-derived positional geometry, including claiming blank/header space as the current selection. This violates the canonical-media-list ownership contract and prevents umbrella row 5.1 from closing after the accepted Browser-family repair.

## What Changes

- Make the persistent Wide series control the sole owner of series cursor/scroll by deleting `TvWorkspaceComponent`'s parent cursor mirror; pointer selection moves the control itself.
- Change series and episode hit payloads from positional ordinals to stable `String` targets, and resolve them only from each control's retained current-frame geometry; blank, header, and above-list space remain unclaimed.
- Stop TV shell click, double-click, and context-menu handling from writing `BrowseLevel::set_resting_cursor`; effects resolve the stable target emitted by the component.
- Fix Wide series wheel eligibility to use retained control geometry, and retire the TV-specific ordinal resolver if TV is its final caller.
- Re-prove Normal/Wide responsive handoff preserves both selected target and row offset in both directions.
- Add focused control tests and Wide plus Normal/Narrow `Application::tick()` integration coverage for painted-target navigation, retained hits, unclaimed geometry, wheel input, one-painter ownership, and breakpoint round-trips.
- Preserve TV-owned seasons, season pills, episode workspace content, the series detail box, images, effects, persistence, and existing typed keyboard/activation semantics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is an ownership refactor that conforms to the existing canonical-media-list, interactive-component-framework, and mouse-input requirements without changing observable requirements.

## Impact

- Affected area: TV Wide interactive component and hit types, TV shell mouse dispatch, the shared ordinal helper only if no other caller remains, responsive-handoff tests, and TV shell-tick/component tests.
- No API, dependency, persistence-format, provider-workspace, routing, mounting/focus, base-frame reservation, shared painter, or unrelated-destination change.

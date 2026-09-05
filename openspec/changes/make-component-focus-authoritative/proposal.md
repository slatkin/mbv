## Why

Mounted Interactive Components discard TuiRealm's `Attribute::Focus` notifications and instead receive duplicated focus booleans through content projection. Event-driven projections can therefore leave component input and focused styling stale after Panel focus moves between Library and Queue, as observed in the Music and Wide TV destinations.

## What Changes

- Make TuiRealm's mounted-component focus lifecycle authoritative for Interactive Component outer focus.
- Have focus-aware mounted components consume `Attribute::Focus` rather than receiving focus through content payloads.
- Keep component-private pane focus and selection intact while outer focus changes; derive focused painting from both states.
- Remove shell focus mirrors and focus-only content re-projection paths made obsolete by the authoritative lifecycle.
- Add live `Application::tick()` regression coverage for Library → Queue → Library focus transitions and focused/unfocused painting.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `interactive-component-framework`: Define TuiRealm focus notifications as the authoritative mounted-component focus boundary and prohibit content projection from carrying a second outer-focus truth.

## Impact

- Interactive Components under `src/app/components/` that gate input or presentation on outer focus.
- Shell destination and Queue synchronisation, content projection, and focus restoration around overlays.
- Music and TV Wide focused-pane rendering; latent stale-focus paths in other destinations are removed at the same ownership boundary.
- Shell `Application::tick()` integration tests and focused/unfocused render characterization coverage.
- No dependency, protocol, configuration, or Service behavior changes.

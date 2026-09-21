# Proposal

## Why

Home's Latest sections expose recently added content, but they do not tell the user which sections changed since the previous client launch or show when each item was added or published. Users must inspect every section and infer recency from ordering alone.

## What Changes

- Record a client-launch timestamp immediately at startup and compare Home Latest item timestamps against the interval since the previous launch.
- Mark each Home Latest pill containing a new item with an Iris `•`; never mark Continue, and clear a section's marker when that Latest pill is selected.
- Treat an already selected Latest pill as visited when its asynchronously loaded content arrives, so it never displays a marker while selected.
- Show each dated Latest item's provider date in the canonical right-aligned row gutter; leave Continue rows unchanged.
- Use provider timestamps as reported: Emby date-added timestamps and Audiobookshelf/Feed publication timestamps. Missing or invalid timestamps do not imply new content and do not render a date.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `home-latest-sections`: Define launch-relative new-content markers on Home Latest pills, visit-to-clear behavior, and provider-date gutters on Latest rows.

## Impact

- Home Latest loading and provider timestamp projection in the application shell.
- Home's Interactive Component state and Selector-row content projection.
- Shared pill-selector typed content and painter support for a semantic Iris marker.
- Existing canonical media-list date gutter projection and date formatting.
- A small per-user launch timestamp persisted atomically at startup; no Service API, queue, playback, daemon, or ctrl protocol changes.
- Coordination with `persist-tui-launch-state-on-exit`: the new-content cutoff has an intentionally different startup-write lifecycle and must not be folded into the exit-only TUI launch-location snapshot.

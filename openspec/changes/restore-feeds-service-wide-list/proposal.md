## Why

Issue #623 reports three defects in the **Feeds Service / Feeds tab Wide panel** (`render_feeds_content`), not the Emby homevideos feed view. At the Wide breakpoint the panel becomes a malformed two-column list, lacks the established rail surface treatment, and renders selected rows incorrectly.

## What Changes

- Keep the Feeds Service Wide media list one column at every Wide width, matching the Music and TV Wide list policy; do not change Narrow behavior unless a regression test demonstrates it is required.
- Restore the existing semantic surface/backdrop/hero-on-left list-panel background and border treatment for the right rail.
- Correct selected-row rendering so titles are not suppressed or duplicated, cell backgrounds do not drift across columns, and selection/played/active markers remain aligned to one-column rows.
- Add metadata-bearing render fixtures and focused buffer/geometry assertions covering all three defects at the 82-column threshold and a larger Wide size.

This is an independent prerequisite for the Feeds slice of `compose-canonical-media-lists`, stacked on `feat/migrate-tui-to-tuirealm` and PR #606. It is separate from accepted #634/#637, which fix the Emby homevideos feed view's Narrow inline expansion.

## Capabilities

### Modified Capabilities

- `right-panel-arrangements`: Feeds Service Wide content is a one-column hero-on-left rail with the established semantic surface treatment.
- `ui-design-system`: Feeds Service Wide rails use semantic surface/backdrop roles and preserve selected-row marker and background semantics.

## Impact

Docs/planning and the eventual Feeds render component/tests only. No provider, protocol, daemon, persistence, keyboard, or Emby homevideos feed-view changes.

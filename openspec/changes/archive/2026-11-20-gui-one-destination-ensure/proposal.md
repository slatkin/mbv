# Proposal

## Why

Each GUI Library destination decides for itself what loads when it shows: Music, Emby, Podcasts, Audiobooks, and Shows each have their own ensure function, and Feeds has none. The per-destination differences caused a real defect: the TV (Shows) destination never loaded its root list from the frame path, so a TV library restored at launch stayed empty until the tab was clicked. The narrow TV fix shipped with PR #1021. The cause remains: Audiobooks and Podcasts still start their root catalog load only from the open command, so they can fail the same way. See GitHub issue #1022.

## What Changes

- Give every GUI Library destination one ensure path. The frame path, the destination open command, and the keyboard router's destination steps all run it.
- The ensure path runs three steps in one fixed order for every destination: the root load, then the selection load, then the dependent loads. A destination states what each step does. A destination cannot skip a step. A destination with no work for a step states that explicitly.
- The Audiobooks and Podcasts catalog loads move into the root step. A restored Audiobooks or Podcasts tab then loads its catalog without a click, as Emby and Shows already do.
- The destination-specific open step keeps only what is a user action, not a load: forcing the Shows view mode to All and clearing a failed Emby root mark so an open retries it. It then runs the one ensure path.
- Every existing guard stays: one fetch at a time per library, no retry of a failed fetch from a frame, no second catalog chain beside the startup chain.
- The Library build pass makes one ensure call. It no longer names a per-destination ensure.
- **BREAKING (internal only)**: the per-destination public ensure functions on `App` become private steps. Their tests move to the one ensure path.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: adds one requirement. Every presented Library destination loads its root list, then its selection, then its dependent content, from the frame without a click, including a destination restored at launch.

## Impact

- `src/app/gui_library/` (new `ensure.rs`; changes in `store.rs`, `emby.rs`, `podcasts.rs`, `audiobooks.rs`, `shows_projection.rs`, `tv.rs`, `feeds.rs`, and the open step in `gui_library.rs`).
- `crates/mbv-gui/src/sections/library.rs`: the build pass calls the one ensure.
- Tests in `src/app/tests/gui_library*.rs` that call the per-destination ensures.
- `docs/architecture/gui-client.md`: one paragraph on the ensure order.
- No new dependency. No wire, config, or persisted-state change.

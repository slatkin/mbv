# Proposal

## Why

Invariant 11 (`docs/invariants/11-launch-state-catalog-boundary.md`, umbrella #810)
documents two properties of launch-tab restoration that no type enforces, and the
doc has already drifted from the code: the "one choke point" for
`emby_catalog_ready` is set at two further sites, the markers are never reset,
and a saved Service tab whose service is unconfigured or failing stays pending
for the whole session — then yanks the tab the moment that service connects
mid-session.

Five fields (`pending_launch_state`, `pending_launch_tab_resolved`,
`legacy_launch_tab`, `legacy_launch_migration_attempted`, `library_tab_pending`)
plus two readiness bools move together as an undocumented state machine. Encode
it so the two invariant properties become unrepresentable-when-broken, and fix
the mid-session tab jump.

## What Changes

- Replace the launch fields with one enum: `Pending(state)` →
  `TabSettled { state, tab }` → `Done`. `Pending → TabSettled` is reachable only
  through a single `select_tab()` that assigns **and** settles the tab.
  The destination re-anchor accepts only `TabSettled` and acts only while the
  active tab still equals the recorded `tab`; otherwise it drops to `Done`.
- Delete `emby_catalog_ready` and `audiobookshelf_catalog_ready`. The one
  production writer of each catalog (`rebuild_library_tabs_from_views`,
  `apply_audiobookshelf_catalog`) resolves a saved Service identity at its end,
  so only code that just built the catalog can resolve one.
- **Expiry:** a saved Service tab resolves to Home immediately when its service
  is not configured, and is dropped on that service's first failure. Launch
  restoration never moves the tab after the startup outcome is known.
- **BREAKING (one-time):** delete legacy selected-tab migration
  (`legacy_launch_tab`, `legacy_launch_migration_attempted`,
  `library_tab_pending`, the numeric fallback branch). A user who has not
  launched since the snapshot landed (`e8bb5e154`, 2026-09-22) loses one tab
  restore.
- Rewrite `docs/invariants/11-…md` to the residual only: `App::tab` still has
  several production writers and no type stops a new one skipping settle.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tui-launch-state`: launch restoration SHALL resolve a saved Service tab only
  against that service's outcome at startup — Home when the service is not
  configured or fails — and SHALL NOT move the tab after that outcome is
  known; the destination re-anchor SHALL apply only to the tab it was resolved
  for.

## Impact

- `src/app/dispatch/library/cw_library_tab.rs`, `load.rs`,
  `src/app/dispatch/run_loop/drains.rs`,
  `src/app/dispatch/session/emby_service_completion.rs`,
  `src/app/shell/library_panel.rs`, `src/app/shell/run.rs`,
  `src/app/state/app_struct.rs`, `construct.rs`.
- Tests: `lifecycle_launch_restore.rs`, `lifecycle_launch_migration.rs`
  (migration tests deleted), `tick_integration.rs:84`, `panel_focus.rs:102`.
- No persisted-format change; `tui_launch_state.json` is unchanged.

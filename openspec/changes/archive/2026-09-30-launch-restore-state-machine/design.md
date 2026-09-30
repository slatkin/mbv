# Design

## Context

See `proposal.md` for motivation. Launch restoration today is spread over
`App` fields that are only ever assigned together:

| Field | Role |
|---|---|
| `pending_launch_state: Option<TuiLaunchState>` | the loaded snapshot |
| `pending_launch_tab_resolved: bool` | tab consumed, pill/item still pending |
| `legacy_launch_tab`, `legacy_launch_migration_attempted` | selected-tab pref migration (8 days old, `e8bb5e154`) |
| `library_tab_pending: usize` | always `0` in production (`construct.rs:163`); only tests set it |
| `emby_catalog_ready`, `audiobookshelf_catalog_ready` | "catalog arrived" markers |

`resolve_library_tab_pending` runs on every sync pass (`shell/run.rs:24`) and
`reanchor_pending_launch_destination` (`shell/library_panel.rs:221`) applies the
snapshot's pill, item and focus to whatever `active_library_key()` is at that
moment. The only stop on the process is an explicit tab move
(`apply_tab_position`).

Nothing stored can tell "catalog not arrived" from "arrived and empty":
`libs` is empty in both cases, and `ServiceState::Ready` precedes the catalog on
a daemon attach (the original bug).

## Goals / Non-Goals

**Goals:**
- Make "selected but not settled" and "resolved before the catalog is live"
  unreachable rather than documented.
- Restoration never moves the tab once the owning Service's startup outcome is
  known.
- Delete state instead of adding it: no new flag, no reset site.

**Non-Goals:**
- No change to `tui_launch_state.json` or to component-side `launch_selector` /
  `reanchor_launch_state`.
- No `App::tab` accessor or single-writer type (kept as the documented residual).
- No `Catalog<T>` wrapper around `libs` / `audiobookshelf_libraries`.

## Decisions

### D1 — One enum replaces the launch fields

```rust
enum LaunchRestore {
    Pending(TuiLaunchState),
    TabSettled { state: TuiLaunchState, tab: TabSelection },
    Done,
}
```

- `App::build` starts `Done` when there is no snapshot, else `Pending`.
- `Pending → TabSettled` happens only through `select_tab()` (D5).
- `TabSettled → Done` when the destination accepts the re-anchor
  (`reanchor_launch_state` returns true), or when `self.tab != tab`.
- Any state `→ Done` on an explicit tab move (`apply_tab_position`).
- Expiry (D3) is itself a resolution: `Pending → TabSettled { tab: Home }`
  through `select_tab`, so the re-anchor still applies the saved Panel focus
  once before the state reaches `Done`.
- `reanchor_pending_launch_destination` matches only `TabSettled`, so
  `pending_launch_tab_resolved == true` with no snapshot cannot exist, and the
  re-anchor cannot apply to a tab it was not resolved for.

*Alternative:* keep the fields and add debug asserts — rejected; asserts do not
run on the failing path and leave the state machine undocumented.

### D2 — Resolve when the catalog arrives; delete both readiness bools

Each catalog has one production writer: `rebuild_library_tabs_from_views`
(`load.rs:280`) and `apply_audiobookshelf_catalog` (`drains.rs:107`). Each ends by
calling `resolve_launch_service_tab(kind)`, which matches a `Pending`
snapshot whose `TabIdentity::ServiceLibrary.kind` is `kind` and runs
`select_tab`. The sync-pass call keeps handling only `Home` and `Feeds`
(`has_feeds_subscriptions` is not catalog-dependent).

Only code that just built the catalog can resolve a Service identity, so the
"marker set on the wrong path / before populated / never reset" failure modes
have no representation. `emby_service_completion.rs:57,212` (redundant sets) and
the reset gap disappear with the field.

`fetch_home` runs synchronously from about eight sites; the added call is a
no-op once the state is not `Pending`.

*Alternatives:* `Catalog<T>` around the vectors (~560 compile-forced edits for
two readers that care); `ServiceState::Ready` (wrong moment); a private-constructor
`CatalogReady` enum (still a stored bool).

### D3 — Expiry follows the Service's startup outcome, not a timer

- Not ready at build: a snapshot naming a Service that is unconfigured or
  configured without its credential is treated as gone at the first resolution
  pass — it settles Home exactly like the gone-tab fallback
  (`select_tab(Home)`, landing in `TabSettled { tab: Home }`) and never waits
  for that Service.
- Failure: the first failed startup outcome for a Service calls
  `expire_launch_service(kind)`, which resolves a `Pending` snapshot naming
  that Service the same way — Home settle, `TabSettled { tab: Home }`. The
  outcome sites are exactly:
  - **Emby:** startup completion `Err` and setup completion `Err`
    (`transition_emby_failure` / `apply_emby_setup_completion_inner`),
    startup-worker disconnect (`handle_emby_startup_worker_disconnect`, the
    terminal Unavailable / Needs-authentication path with no `Err` completion),
    and any later `handle_emby_runtime_failure` — which also covers a
    `fetch_home` failure on a daemon attach.
  - **Audiobookshelf:** validation completion `Err`
    (`apply_audiobookshelf_completion` — the catalog worker never starts on
    this path, so the catalog-`Err` hooks below do not cover it), catalog
    completion `Err` including the non-auth `Err(_) => {}` arm (`drains.rs`),
    startup/validation-worker disconnect
    (`handle_audiobookshelf_worker_disconnect`), and catalog-worker
    disconnect: `drain_audiobookshelf_catalog_event` receiving
    `Err(Disconnected)` without a completion, routed through the same
    generation-checked handler using the generation retained on
    `AudiobookshelfCatalogReceiver` (a new field, set at spawn — the
    receiver carries only `rx` today). Without it a silently dead catalog
    worker leaves the snapshot `Pending` forever, and a later user re-setup
    then yanks the tab.

  Expiry hooks sit behind the existing generation-acceptance checks, so a stale
  completion neither resolves nor expires the current snapshot.
- A later connection then finds `TabSettled` or `Done`, which
  `resolve_launch_service_tab` never matches, and does nothing (spec: no
  mid-session tab move). This mirrors the rule already stated for
  `pending_navigate_tab_switch`: a later drain must never yank the tab.
- Expiry is a resolution, not a discard: the snapshot survives one re-anchor
  pass so the saved Panel focus is still applied to the Home fallback
  (`tui-launch-state`'s Queue-focus requirement; `settle_tab_selection`'s Home
  arm touches no focus), after which the re-anchor's normal accept-or-mismatch
  rule moves the state to `Done`. Saved Service pill/item identities match
  nothing on Home; Home's re-anchor ignores them and selects its first item.

### D4 — Delete legacy migration

Delete `legacy_launch_tab`, `legacy_launch_migration_attempted`,
`library_tab_pending`, `migrate_legacy_launch_state`, `legacy_tab_identity`,
`legacy_position_for_key`, `legacy_identities`, and the numeric fallback in
`resolve_library_tab_pending`. This removes the only reader
(`legacy_tab_identity`) that needed both catalogs' readiness, so D2 needs no
per-catalog state. Cost: one lost tab restore for a user who has not launched
since 2026-09-22. `library_position_state` stays (other consumers); only its
use here goes.

### D5 — `select_tab()` is the one assign-and-settle

`fn select_tab(&mut self, tab: TabSelection)` assigns `self.tab` and calls
`settle_tab_selection()`. `apply_tab_position` and the `Pending → TabSettled`
step both use it. `TabSettled.tab` records `self.tab` *after* settling, because
`settle_tab_selection` can normalise a stale index to Home.

*Alternative:* make `App::tab` private behind an accessor — deferred, see
Non-Goals.

## Risks / Trade-offs

- [Some startup-outcome path is missed and a snapshot stays `Pending`] → D3
  enumerates every site where a Service outcome becomes known (Emby startup
  Ok/Err, setup Ok/Err, worker disconnect, later runtime failure incl.
  `fetch_home` on daemon attach; Audiobookshelf validation Ok/Err, catalog
  Ok/Err, worker disconnect; the configured-without-credential start), and the
  tasks carry a hermetic test per path plus the end-to-end fails-then-connects
  scenario per Service.
- [`TabSelection` is index-based; a catalog rebuild between settle and re-anchor
  could make `self.tab == tab` compare a different library] → window is one sync
  pass; accepted, re-anchor still resolves pill/item against current content.
- [Deleting migration loses one restore] → accepted by the user (2026-09-30).
- [`App::tab` still has ~5 production writers] → stays documented as invariant
  11's residual.

## Migration Plan

Single change, no persisted-format change; rollback is a revert. Invariant 11 is
rewritten to the residual in the same change.

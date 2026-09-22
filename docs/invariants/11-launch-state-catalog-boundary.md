# Invariant 11 — Every live-catalog path marks the catalog ready

**Scope:** launch-tab resolution (`src/app/cw_library_tab_actions.rs`), the
`emby_catalog_ready` / `audiobookshelf_catalog_ready` gates, and the two
places live Emby views become `App::libs`
(`apply_emby_bootstrap`, `fetch_home`).

## The invariant

`resolve_library_tab_pending` may resolve a saved `TabIdentity` only after the
owning catalog is live: `resolve_service_tab` returns `None` while
`emby_catalog_ready` / `audiobookshelf_catalog_ready` is false. Therefore
every code path that actually delivers the live catalog must set its ready
flag, and must set it only once `App::libs` / `App::audiobookshelf_libraries`
holds that catalog (a ready flag over an empty catalog resolves a saved id to
the first-guaranteed-tab fallback and consumes the one-shot intent).

Emby has two such paths: the startup worker's `apply_emby_bootstrap` for a
plain local launch, and `fetch_home`'s view rebuild for a local-daemon/remote
attach, which has a live client at construction and never runs the worker.
`rebuild_library_tabs_from_views` is the single choke point both go through,
so it owns `emby_catalog_ready = true`.

## Why it matters

The launch snapshot stores opaque Service ids, not tab indices. If a path
never marks the catalog ready, `resolve_library_tab_pending` returns early on
every tick and the one-shot pending state is never consumed: the TUI silently
stays on Home instead of restoring the saved tab, even though the snapshot was
written correctly at exit.

## How the code maintains it today

`App::build` sets both ready flags false. `apply_emby_bootstrap` (plain local
launch) and `rebuild_library_tabs_from_views` (reached by `fetch_home` on a
local-daemon/remote attach) both rebuild `libs` from live views;
`rebuild_library_tabs_from_views` sets `emby_catalog_ready` after the rebuild.
Audiobookshelf completion sets `audiobookshelf_catalog_ready` in the run-loop
drain, after its libraries are assigned.

## What breaks if it is violated

- A local-daemon/remote launch restores the queue but never the tab, and the
  pending launch state leaks for the rest of the session.
- Marking ready before the catalog is populated silently downgrades a saved
  Service tab to Home.

Regression: `app::tests_lifecycle::
home_view_rebuild_marks_the_catalog_ready_and_resolves_the_launch_tab`.

# Invariant 11 — Launch restoration needs a live catalog and a loaded tab

**Scope:** launch-tab resolution (`src/app/dispatch/library/cw_library_tab.rs`), the
`emby_catalog_ready` / `audiobookshelf_catalog_ready` markers, and every place
live Emby views become `App::libs` (`apply_emby_bootstrap`, `fetch_home`).

## The invariant

Launch-state restoration has two prerequisites, and both must hold on *every*
launch path, not just the plain local one.

1. **The catalog is live.** `resolve_service_tab` returns `None` until the
   owning catalog has arrived, so the marker that says so must be set where
   the catalog is actually applied. `App::build` starts both markers false;
   `rebuild_library_tabs_from_views` — the one choke point both Emby catalog
   paths go through — sets `emby_catalog_ready` after the rebuild. It must not
   be set by a specific *attach* path: a plain local launch reaches the catalog
   through the Emby startup worker's `apply_emby_bootstrap`, while a
   local-daemon/remote attach has a live client at construction and never runs
   that worker, reaching the catalog through `fetch_home` instead.

2. **The resolved tab is settled, not merely selected.** Assigning
   `self.tab` alone leaves the destination's `nav_stack` empty, so the panel
   owner is handed no content and the restored tab paints blank. The restore
   path must run the same settle step as a user tab switch
   (`settle_tab_selection`: stale-destination fallback, image dims, panel
   focus, library activation, tab-bar visibility, prefs), because the launch
   snapshot names a tab and a pill/item, never a browse position.

The pending snapshot is deliberately *not* consumed by settling: the pill/item
re-anchor still needs it.

## Why it matters

The launch snapshot stores opaque Service ids, not tab indices. If the marker
is never set on a path, `resolve_library_tab_pending` returns early forever,
the one-shot pending state leaks, and the TUI silently stays on Home. If the
tab is selected without activation, the tab bar shows the right tab over an
empty panel. Both look identical to "the feature doesn't work".

## How the code maintains it today

`App::build` sets both ready markers false. `rebuild_library_tabs_from_views`
rebuilds `libs` from live views and then sets `emby_catalog_ready`.
Audiobookshelf completion sets `audiobookshelf_catalog_ready` in the run-loop
drain, after its libraries are assigned. `resolve_library_tab_pending` assigns
`self.tab` and calls `settle_tab_selection()`, which is the same function
`apply_tab_position` calls after user tab movement.

## What breaks if it is violated

- A local-daemon/remote launch restores the queue but never the tab, and the
  pending launch state leaks for the rest of the session.
- Marking ready before the catalog is populated downgrades a saved Service tab
  to Home.
- Selecting the restored tab without activating it paints a blank panel.

Regression: `app::tests_lifecycle::
restored_launch_tab_loads_its_library_content_not_just_the_tab`.

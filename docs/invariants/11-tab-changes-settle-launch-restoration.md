# Invariant 11 — Tab changes settle launch restoration

Umbrella: #810.

## The invariant

`App::tab` has several production writers. Every tab change that occurs while
launch restoration is pending must pass through `select_tab` so the selected
tab is settled before a destination re-anchor. No type currently prevents a
new writer from assigning `App::tab` directly and skipping that settle step.
This remains a caller discipline, not a type-enforced boundary.

## Why it matters

Selecting a tab without settling it can leave its destination inactive and the
panel blank. A writer that bypasses `select_tab` can also leave restoration
silently abandoned: the re-anchor's recorded-tab guard detects the mismatch
and moves `LaunchRestore` to `Done` without applying the saved destination.

## How the code maintains it today

`LaunchRestore` encodes the restoration lifecycle: a snapshot moves from
`Pending` to `TabSettled { state, tab }` through `select_tab`, then to `Done`
when an explicit tab move abandons the launch intent, after the destination
accepts its re-anchor, or when the active tab no longer matches the recorded
tab. The state records which tab was settled, and the re-anchor checks that the
active tab still matches it.

`LaunchRestore` keeps the pending snapshot and settled destination together,
and each identity resolves at its own boundary. Home and Feeds identities, plus
unavailable-at-startup Service identities, resolve on the sync pass
(`resolve_launch_tab_on_sync`); a live Service identity resolves when its
catalog arrives; and a Service startup failure or disconnect expires the
matching pending identity to Home. Each resolution uses `select_tab`,
preserving the destination re-anchor pass; readiness-marker state is not used.

A failed Service state is written only through `fail_emby_service` and
`fail_audiobookshelf_service`, which pair the state write with
`expire_launch_service`, so a failure cannot leave the snapshot Pending. The
residual is the Audiobookshelf catalog failure in `drains.rs` that does not
change Service state: it calls `expire_launch_service` directly.

## Global UI-state reset ordering

`Model::reset_ui_state` (row 5.1, #745) is the deliberate bypass of this
invariant: it abandons launch intent instead of settling it. Its ordering is
load-bearing and is not a free choice:

1. `abandon_pending_ui_intents` runs first. It sets `LaunchRestore::Done` and
   clears the pending navigate/series/track/overlay/queue-reanchor intents and
   the shell-owned selection/gesture/search intent.
2. Only then is Home selected through the existing settled-tab path,
   `set_library_tab(0)`. `apply_tab_position` marks the move `Done` before it
   calls `select_tab`.
3. Layout/geometry defaults are restored (Panel mode and focus, widths,
   artwork versus visualizer).
4. Shell browse roots return to their default fields without refetch or cache
   invalidation, and each retained Audiobookshelf podcast state clears its
   committed show pill so the lazy fan-out scope matches the owner's default
   `State(All)` pill.
5. Transient overlays are dismissed through the existing lifecycle helpers.
6. Mounted owners and Queue-local presentation reset
   (`LibraryPanel::reset_presentation`, `QueueComponent::reset_presentation`).
7. Saved presentation state is cleared last, after every live write, so a
   later sync cannot recreate it. The live reset stays applied if that clear
   fails.

Done-before-`select_tab` is what makes step 1 matter. If the snapshot were
still `Pending` when Home was selected, `select_tab` would wrap it into
`TabSettled`; a live Service identity resolving when its catalog arrives would
then re-settle the snapshot the reset discarded. With `launch_restore` already
`Done`, that catalog arrival has no pending identity to resolve.

One completion guard backs this up. A root-level `Loaded` whose
`LevelFetchKey` no longer matches the reset root is dropped by
`drop_stale_root_completion` in
`src/app/dispatch/library/event/browse_loads.rs` (#745). The root's parent id
survives a reset, so the parent-match check alone would accept a browse issued
before the reset and reinstate its discarded sort/filter/scope; the fetch-key
comparison rejects it. If that dropped completion was the unfiltered root's
only in-flight load, the guard re-issues it under the reset fields so the
default root still completes.

## What remains unenforced

`App::tab` still has several production writers, and no accessor or single-writer
type prevents a future writer from bypassing `select_tab` and its settle step.

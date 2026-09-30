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

## What remains unenforced

`App::tab` still has several production writers, and no accessor or single-writer
type prevents a future writer from bypassing `select_tab` and its settle step.

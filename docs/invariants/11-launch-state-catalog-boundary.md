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
panel blank. Skipping the settle step can also leave `LaunchRestore` in a state
that does not describe the selected tab, so the pending destination may be
re-anchored against the wrong tab.

## How the code maintains it today

`LaunchRestore` encodes the restoration lifecycle: a snapshot moves from
`Pending` to `TabSettled { state, tab }` through `select_tab`, then to `Done`
after the destination accepts its re-anchor or the active tab no longer
matches the recorded tab. The state records which tab was settled, and the
re-anchor checks that the active tab still matches it.

Launch restoration's other two properties are enforced: `LaunchRestore` keeps
the pending snapshot and settled destination together, and Service-tab identity
is resolved only when that Service's catalog arrives. Catalog arrival is the
resolution boundary; readiness-marker state is not used.

## What remains unenforced

`App::tab` still has several production writers, and no accessor or single-writer
type prevents a future writer from bypassing `select_tab` and its settle step.

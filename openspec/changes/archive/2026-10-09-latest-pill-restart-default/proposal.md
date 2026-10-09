# Proposal

## Why

A restart currently reopens whatever main Selector pill the user left active on the exit tab (a letter range, a podcast show, a Feeds filter, a TV mode). The user wants every Latest-bearing library to greet a restart on its `Latest` pill, with every other pill selection staying session memory. Per user decisions (2026-10-09): Grouped Music and Audiobookshelf book libraries keep persisting their pill (no Latest pill there); the selected item is never restored across a restart (always the top row); the last tab keeps restoring.

## What Changes

- Launch snapshots record the destination's persisted pill scope instead of the live pill: the `Latest` pill for every destination whose selector offers one (Emby libraries with a painted selector, TV libraries, podcast libraries, Feeds); the selected group for Grouped Music and the selected bucket for Audiobookshelf book libraries stay as today; Home keeps its fixed Continue scope.
- Launch snapshots stop recording the selected library item. Restoration always lands on the first selectable row of the restored scope. Existing reanchor item handling remains as legacy-decode for snapshots written by older versions.
- Selector restore paths honor only the persisted scope: legacy letter/group/show/filter selectors in an old snapshot decode but resolve to the destination's default (the Home precedent).
- The legacy per-library position document stays a load-time migration reader only; its letter-filter, TV content mode, and feed-group fields are cleared at load so a stale file cannot resurrect a pill from an old run. The document remains never-written.
- Mid-session behavior is unchanged: owners retain pills across tab switches, and the in-memory position map still restores browse positions on tab re-entry.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tui-launch-state` — snapshot content (pill scope, no item) and restoration rules.
- `destination-latest-modes` — Latest is the restart selection for every Latest-bearing destination; other pills are session-only.
- `tv-library-content-modes` — the TV content mode restores within a session only; restart resolves the count-dependent default.
- `audiobookshelf-podcast-library-ui` — a podcast library starts on `Latest` after every restart.

## Impact

- `crates/mbv-components`: `launch_snapshot`/`launch_selector`/`reanchor_launch_state` for Emby library, TV, podcast, Feeds, book, music, and Home owners.
- `src/app/state/construct.rs`: clears pill fields on the legacy position document at load.
- Tests: component snapshot tests, launch-restore lifecycle tests, shell run tests.
- No config-type, protocol, or queue changes; `SelectorIdentity` variants stay decodable for old snapshots.

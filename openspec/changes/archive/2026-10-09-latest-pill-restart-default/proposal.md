# Proposal

## Why

A restart currently reopens whatever main Selector pill the user left active on the exit tab (a letter range, a podcast show, a Feeds filter, a TV mode). The user wants every Latest-bearing library to greet a restart on its `Latest` pill, with every other pill selection staying session memory. Per user decisions (2026-10-09): Grouped Music and Audiobookshelf book libraries keep persisting their pill (no Latest pill there); the selected item is never restored across a restart (always the top row); the last tab keeps restoring.

## What Changes

- Launch snapshots record the destination's persisted pill scope instead of the live pill: the `Latest` pill for every destination whose selector offers one (Emby libraries with a painted selector, TV libraries, podcast libraries, Feeds); the selected group for Grouped Music and the selected bucket for Audiobookshelf book libraries stay as today; Home keeps its fixed Continue scope.
- Launch snapshots stop recording the selected library item. Restoration always lands on the first selectable row of the restored scope. The snapshot tuple shape stays decodable so snapshots written by older versions still parse, but a saved item, letter range, show, watched filter, or other non-scope pill SHALL NOT restore (`tui-launch-state`: "A saved library item, letter range, show, watched filter, or other non-scope pill in a snapshot written by an older version SHALL NOT restore"; "the first selectable item in the restored scope's list is selected and the saved item SHALL NOT restore").
- Selector restore paths honor only the persisted scope: legacy letter/group/show/filter selectors in an old snapshot decode but resolve to the destination's default (the Home precedent). On every restart, every podcast library starts on `Latest`, whether or not it was the selected destination at orderly exit (`audiobookshelf-podcast-library-ui/spec.md:123`).
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
- `src/app/dispatch/action.rs:270` (with `src/app/dispatch/library/load.rs`, `src/app/shell/messages/navigation.rs`, `src/app/shell/overlays/sidebars.rs`, `src/app/shell/settings.rs`): F4 toggles the Playlists sidebar like F2/F3 (function-key parity; commit `7d239e821`).
- `crates/mbv-render/src/components/chrome_tabs.rs:186` with `crates/mbv-ui-model/src/ui_util.rs:16` (`is_home_icon_title`): the selected icon-only Home tab paints no eighth-block runs and keeps the Iris active colour (2026-10-09 user rule; commits `ca8fdab33` + `ba48238c8`).
- `src/app/state/projection/chrome_status.rs:466`: a shuffle-sourced queue paints no source pill (2026-10-09 user rule; commit `4d2f4d38b`).
- Tests: component snapshot tests, launch-restore lifecycle tests, shell run tests, the F4 toggle test (`src/app/tests/tick_integration.rs`), and the chrome-tabs/ui-util tests.
- No config-type, protocol, or queue changes; `SelectorIdentity` variants stay decodable for old snapshots.

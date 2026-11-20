# Proposal

## Why

The GUI Playlists screen takes the Library panel's slot, but it looks better in its narrow form, with the button strip above the tree. It does not need a wide view. The Queue column has the right width for it, and the Library panel can stay visible beside it.

## What Changes

- The Playlists control in the top bar toggles what the Queue column shows: the queue list, or the Playlists view (the existing narrow arrangement of the Playlists screen, moved as it is).
- While the Playlists view shows, the playback panel stays above it. Only the now-playing title block under the art changes: the Playlists view's title block (the picked row's title and detail line) takes its place. The view's button strip and the playlist tree follow under it. The Queue footer stays under the view.
- The Queue column keeps every behaviour it has now. In the queue-fold state the view takes the folded line's place. In the compact strip state the view shows under the strip. A Playlists press while the column is collapsed to its rail expands the column and shows the view.
- Play, Shuffle, and Add to queue close the view, so the queue shows again. Play and Shuffle close it when the queue replacement commits; a cancelled replacement prompt leaves it open. Add to queue closes it at once. This applies to the button strip, the right-click menu, and the item double-click.
- The Library panel stays visible while the view shows. A press on a destination (Home or a tab) leaves the view open. The top bar marks the active destination and the Playlists control together.
- In the Mini arrangement, opening the view shows the Queue column. A Playlists press while Mini shows the Library brings the Queue column back and leaves the view open.
- The Library panel's keys work while the view shows, as they do with the queue list.
- **BREAKING (GUI layout)**: the Wide arrangement of the Playlists screen, with the tree beside the hero and its own resize divider, is removed. The Playlists screen no longer takes the Library panel's slot.
- The stale Playlists text in the `gui-settings-screen` spec is corrected: Playlists left the Settings screen in commit c35ddb043 without a spec change.

Decisions made with the user (2026-11-20): the existing narrow design moves without redesign; a destination press leaves the view open; a Playlists press on the collapsed rail expands the column; the Queue column is not changed as a concept, only what it displays.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: adds the requirement for the Playlists view in the Queue column (toggle, placement, title replacement, close on Play/Shuffle/Add, Library panel kept, Mini and rail behaviour, actions). The Service-mark header requirement drops its Playlists screen reference. The tree double-click requirement names the Playlists view.
- `gui-settings-screen`: the sidebar, open-action, and keyboard requirements drop Playlists; the "Playlists offers the TUI's playlist actions" requirement moves to `gui-client`.

## Impact

- `crates/mbv-gui/src/window/app.rs`: the Library slot no longer mounts the Playlists screen; the Mini view logic and the top bar props change.
- `crates/mbv-gui/src/window/components/panel.rs`: the Queue column paints the Playlists view in place of the queue list.
- `crates/mbv-gui/src/window/components/playlists_panel.rs`: the Wide arrangement, its divider, and its width props go.
- `crates/mbv-gui/src/window/components/queue_playback_panel.rs`: the now-playing title block can be left out.
- `crates/mbv-gui/src/window/components/library_top_bar.rs`: a destination press no longer closes the view; the Playlists press goes through a callback from `App`.
- `crates/mbv-gui/src/window/library/keys.rs`: the Playlists gate keeps only the rename field step.
- `src/app/gui_playlists.rs`, `src/app/gui_playlists/commands.rs`: the toggle expands a collapsed column; Add to queue closes the view.
- Docs: `docs/architecture/gui-client.md`, `CONTEXT.md`.
- The TUI Client, the ctrl protocol, persistence, and dependencies do not change.

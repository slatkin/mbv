# Design

## Context

See proposal.md for the motivation and the specs for the required behaviour.

Today `App` (`crates/mbv-gui/src/window/app.rs`) mounts `PlaylistsPanel` in the Library panel's slot while the `playlists` section is present, and `panel_shows` counts that section. `painted_view` forces the Mini view to Library while the section is present. The Queue column is `Panel` (`window/components/panel.rs`). In the Expanded and ExpandedWithoutQueue states, `Panel` paints `QueuePlaybackPanel`, then `QueuePanel` or `FoldedQueue` in a `min-h-0 flex-1` slot, then `QueueFooter`. The CompactStrip state paints `CompactStrip` alone, and the Rail state paints `CollapsedRail`.

`PlaylistsPanel` has two arrangements. Wide puts the tree beside the hero, with its own `PanelDivider` on the list width. Every other arrangement puts the hero (title, detail line, and the picked row's control buttons) above the tree. The window owns the pick, the right-click menu, and the rename draft. The core owns whether the view is open (`App::gui_playlists_open`), the saved list, and the open playlist.

Facts that the design depends on:

- Play and Shuffle already close the view on commit. `replace_queue_from_playlist` uses `ReplacementExecutor::PlaylistsSidebar`. On commit with no raised prompt, that executor raises `DismissSidebar(Playlists)`, and the GUI's `apply_pending_overlay_request` (`core_thread/settings.rs`) maps it to `close_gui_playlists()`. A cancelled prompt raises nothing. The item double-click and the right-click menu send the same `Action`/`ItemAction` commands, so they take the same path.
- Add to queue (`enqueue_playlist_items`) leaves the view open. The TUI sidebar keeps that behaviour.
- The `playlists_open` key reaches the GUI as `ToggleSidebar(Playlists)` and calls `toggle_gui_playlists()`, the same core method as the top bar's `Toggle` command.
- The `panel` section (collapse and fold toggles) is not gated. It is built every tick, so a core change to `panel_collapsed` reaches the window with no extra mark.
- Queue keys act only with `KeyTarget::Queue`, which comes from the `data-pointer-zone="queue"` marker. Only `QueuePanel` and `QueueFooter` carry that marker. The folded state already unmounts `QueuePanel`, so the core already handles a Queue column without a mounted queue list.
- The key router (`window/library/keys.rs`) returns every key to the core while the `playlists` section is present, except the rename field's keys, because today the Library panel is not painted then.

## Goals / Non-Goals

**Goals:**

- Move the existing narrow Playlists arrangement into the Queue column without changing how it looks.
- Keep every existing Queue column state and toggle as it is.

**Non-Goals:**

- No restyle of the hero, its buttons, or the tree.
- No keyboard navigation for the Playlists tree. The tree had none before this change.
- No TUI change. The TUI Playlists sidebar keeps its behaviour, including Add to queue that leaves the sidebar open.
- No change to Esc. Esc does not close the view, as before.

## Decisions

### D1. `Panel` chooses between the queue list and the Playlists view

`Panel` reads `sections.playlists`. When the section is present:

- Expanded and ExpandedWithoutQueue: `QueuePlaybackPanel { now_playing_title: false }`, then `PlaylistsPanel {}` in the `min-h-0 flex-1` slot in place of `QueuePanel` or `FoldedQueue`, then `QueueFooter` as now.
- CompactStrip: `CompactStrip`, then `PlaylistsPanel {}` in a `min-h-0 flex-1` slot under it.
- Rail: `CollapsedRail` as now. D4 makes sure that an open action never leaves the view behind the rail.

Alternative considered: a separate column component that wraps `Panel`. Rejected, because the panel-state logic would then be in two places.

### D2. The now-playing title block can be left out

`QueuePlaybackPanel` gets one prop, `now_playing_title: bool`, with `#[props(default = true)]`. While an item plays, the art section always paints. The title `div` paints only when the prop is true. The block reports no layout region, so leaving it out changes no hit map. The stopped-state control row and the player menu do not change. Every existing caller keeps the default.

The narrow `PlaylistsPanel` already paints the hero (title, detail line, buttons) first and the tree under it. When it is mounted right after `QueuePlaybackPanel`, the hero sits directly under the art, in the place of the now-playing title.

### D3. `PlaylistsPanel` keeps only its narrow arrangement

Remove the `arrangement`, `list_width`, and `library_column` props, the Wide branch, its `PanelDivider`, the `PanelWidthsStore` handles, and `data-playlists-arrangement`. The root is the existing narrow column (`relative flex h-full min-h-0 min-w-0 flex-1 flex-col`), with the hero (`flex-none`) above the tree (`flex-1`). The pick, the menu, the rename field, the classes, and the colours do not change. The root carries no `data-pointer-zone`, so a key with the pointer over the view resolves `KeyTarget::None`, and no queue row key acts (spec: *Queue row keys do not act on the hidden queue list*).

### D4. The core toggle expands a collapsed column

In `src/app/gui_playlists.rs`:

- `open_gui_playlists` also sets `panel_collapsed = false`.
- `toggle_gui_playlists`: when the view is open and the column is not collapsed, close the view. When the view is open and the column is collapsed, set `panel_collapsed = false` and keep the view open with no reload. When the view is closed, open it.

The press and the `playlists_open` key both reach this one method, so the rail behaves the same for both. Alternative considered: the window expands the rail before it sends `Toggle`. Rejected, because the key path does not pass through the window.

### D5. Add to queue closes the view, in the GUI command arm only

In `apply_gui_playlists_command` (`src/app/gui_playlists/commands.rs`), the `Action` and `ItemAction` arms call `close_gui_playlists()` after the shared action when the action is `MusicTreeAction::Enqueue`. The shared `playlist_action` and `playlist_row_action` stay unchanged, so the TUI keeps its sidebar open. Play and Shuffle need no change (see Context).

### D6. `App` no longer gives the Library slot to Playlists

In `window/app.rs`:

- Remove the `PlaylistsPanel` branch from the Library slot. `panel_shows` becomes `has_library_list`.
- `LibraryTopBar` gets `current_tab: header.current_tab` and `home_active: header.home_active` with no Playlists override. `playlists_active` stays.
- Remove `painted_view`. The Mini view is `mini_view.cloned()`.

### D7. Mini: open shows the Queue column; the press brings it back

Two window-side pieces, in a new child module `window/app/playlists.rs`, keep `app.rs` under 800 lines:

- An open edge. An effect reads `sections.playlists`. When the section goes from absent to present, the effect sets `mini_view` to `MiniView::Queue`. The previous openness lives in a `use_hook` cell that no render reads. The effect writes only `mini_view`, which it does not read. This covers the press and the key.
- The press. `LibraryTopBar` gets `#[props(default)] on_playlists_pressed: Option<Callback<()>>`. When it is set, the Playlists control calls it instead of sending `Toggle`. `App`'s callback runs this rule: when the arrangement is Mini, `mini_view` is Library, and the view is open, set `mini_view` to Queue and send nothing. In every other case, send `GuiPlaylistsCommand::Toggle`.

### D8. A destination press no longer closes the view

`open_destination` in `library_top_bar.rs` stops sending `GuiPlaylistsCommand::Close`. `destination_pressed` does not change: in Mini a destination press still shows the Library.

### D9. The key router keeps only the rename step for Playlists

In `route_library_key`, the Playlists gate keeps its rename field step and drops `return KeyOutcome::Core(target)`. With no rename draft, routing falls through to the Library steps, as it does with the queue list. Settings and Playlists never show together, so `settings.ui.text_field` can only be the rename draft while the section is present.

### D10. Docs and the domain glossary

- `docs/architecture/gui-client.md`: rewrite the Playlists screen paragraph for the Playlists view in the Queue column (toggle, placement, title replacement, close rules, Mini and rail). Change "the Playlists screen" in the Ctrl+K routing step to "the Playlists view".
- `CONTEXT.md`: add the term *Playlists view* (the GUI Queue column's alternative to the queue list; it is not the TUI's Playlists sidebar), with an *Avoid* line for "Playlists screen". Remove Playlists from the Settings screen's section list. Change "Home and the Playlists screen show a text title" to "Home shows a text title".

### D11. Tests

No tests are added. Tests that assert removed behaviour are deleted: `mini_paints_the_playlists_screen_in_the_library_slot` in `window/app/tests/header.rs` asserts `painted_view`, which D6 removes. Every other test that fails to compile against a changed signature is updated to the new signature, and its contract stays as it is.

## Risks / Trade-offs

- [The `playlists_open` key in Mini, while the Library shows and the view is open, closes the view, which the user cannot see.] → The core does not know the window's Mini view, so the press rule in D7 cannot apply to the key. A second key press opens the view, and the open edge shows the Queue column. The spec states the Mini rule for the press only.
- [The Queue footer keeps its Queue pointer zone. A key with the pointer over the footer still reaches the queue's selected row while the view shows.] → This is the footer's behaviour today. The spec covers keys over the view only.
- [Rename, Refresh, and Delete in a narrow 400 px column wrap the hero buttons to a second line.] → The user decided to move the existing design as it is.
- [`app.rs` is 788 lines.] → D7 puts the new code in `window/app/playlists.rs`, and D6 removes `painted_view` and the Playlists branch.

## Migration Plan

This is a GUI-only layout change with no persisted state, so no migration is needed. The Wide list width that the Playlists screen shared with the Library panel stays in use by the Library panel. Rollback is a revert of the change's commits.

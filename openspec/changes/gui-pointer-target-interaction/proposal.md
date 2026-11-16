# Proposal

## Why

The GUI Client still uses the TUI's focus model. One sticky Panel focus (Library or Queue) decides where keys go. A Tab walk moves focus across panes. Each Library list keeps a keyboard cursor, and that cursor is separate from the item that the Hero shows. A GUI has a pointer, and it has no use for these rules: the user looks at and points at a list, but the keys go to the panel that they clicked last. This change replaces the TUI focus model in the GUI with a pointer-target model. The thing under the pointer receives the keys. The Library has no selection, and the Queue is a ledger with one persistent active row.

## What Changes

- **BREAKING (GUI only)**: The GUI Client has no Panel focus. Each key goes to its pointer target: the Library row or the Library column under the pointer, the Queue list box under the pointer, or (with no target) only the global chords. A press, a Tab, or a crossing does not store a focus.
- **BREAKING (GUI only)**: The Library lists have no keyboard cursor and no selection. Enter, `n`, and `a` act on the hovered row. The arrow, Page, Home, and End keys scroll the list under the pointer. A press on a row still opens it in the Hero, and the open item keeps its marker. The Hero tracklist and the Workspace tables have no cursor. A single press on one of their rows does nothing, and a double-click or the hover play button plays the row.
- The Tab pane walk is removed. The pill row has no keyboard focus, so Space is always the core's play/pause. Tab and Shift+Tab reach the core's keyboard router.
- The Queue is a ledger. Its active row (the Queue cursor) always shows, whatever the pointer does. A click and the arrow keys move the active row. Hover does not move it. Keys over the Queue list box act on the active row. The now-playing row, the hovered row, the marked rows, and the active row each show differently.
- Mini shows one panel, chosen by a Mini view, not by focus. A segmented Library | Queue switch at the top of the panel that shows changes the view. The GUI launch location saves the Mini view in place of Panel focus.
- A text field (the Search field, the Library filter, the Queue filter) keeps the keys while it is open, wherever the pointer is. Esc closes the open Library menu, the Hero overlay, or the Search results wherever the pointer is.
- The `panel_right` and `panel_left` actions become TUI-only. In the GUI their routed command has no effect.
- The window keeps no copy of the core's focus. The frame's focus section is removed.
- The TUI Client does not change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: replaces the focus requirements (Exactly one focus is visible, Panel focus by press and Tab, the window keys, the Library keys) with the pointer-target model. It changes the Mini arrangement to the Mini view, the launch location to save the Mini view, the Music list, the letter rail, the collection pills, the Status bar hints, and the interaction-state split, so that they name no Library cursor and no Panel focus. It adds the Queue ledger requirement.
- `client-interaction-core`: the focus-stack requirement states that the GUI Client presents no Panel focus. The Client core sets the stack's Panel focus from each key's pointer target.

## Impact

- `crates/mbv-gui/src/window/launch.rs`: the launch copy publishes the pointer target from Blitz's hover node after each window event and each frame.
- `crates/mbv-gui/src/window/library/`: the key router and every destination's keys (`keys.rs`, `keys_shared.rs`, `emby_keys.rs`, `shows_keys.rs`, `audiobooks_keys.rs`, `podcasts_keys.rs`, `feeds_keys.rs`), the per-destination cursor state and its painting, and the pill focus (`pill_list.rs`).
- `crates/mbv-gui/src/window/components/`: the Library lists, the Hero tracklist, the Workspace table, the Queue panel row paint, the Library top bar and the Queue header (the Mini switch), and the Library Status bar.
- `crates/mbv-gui/src/window/app.rs`, `arrangement.rs`: Mini picks its panel from the Mini view.
- `crates/mbv-gui/src/core_thread.rs`, `core_thread/tick_host.rs`, `core_thread/launch_restore.rs`, `src/window/channel.rs`, `src/bridge/`: the key carries its target. The focus section, `GuiFocus`, and the `TakeFocus`/`TakeQueueFocus` commands are removed. A `SetMiniView` command and a Mini view in the panel toggles are added.
- `src/app/gui_library.rs`, `src/app/gui_library/launch.rs`, `src/app/gui_library/item_menu.rs`: the core sets the Panel-focus adapter itself and holds the Mini view.
- `crates/mbv-config/src/gui_launch_state.rs`: `mini_view` replaces `panel_focus`, and older files still load.
- `crates/mbv-keybinds/src/registry.rs`: the `panel_right` and `panel_left` scope.
- Docs: a new ADR (the GUI derives its key target from the pointer; FocusStack is an internal adapter for the GUI), `docs/architecture/gui-client.md`, and `CONTEXT.md` (a *Pointer target* and *Mini view* term; Panel focus names a TUI concept). At archive, the `gui-client` spec's Purpose line drops "keyboard-first".
- No new dependency.

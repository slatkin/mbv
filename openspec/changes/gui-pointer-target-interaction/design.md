# Design

## Context

See proposal.md (Why) for the motivation, and the `gui-client` and `client-interaction-core` deltas for the required behavior. This section lists only the current facts that shape the approach.

- **Two focus layers.** Panel focus (Library or Queue) lives in the core's `FocusStack` and in `App`'s stored panel focus. The core moves both together (`CoreThread::take_panel_focus`). The window reads it from the frame's `focus` section (`GuiFocus`) in three places: the Mini panel pick (`window/app.rs`), the Library key gate and the Tab walk (`window/library/keys.rs`), and the Queue cursor bar (`queue_panel.rs`, `bars`). Pane focus (`LibraryPane`: Pills, List, Hero) is window state in `LibraryUiState.pane`.
- **Library cursors.** Each destination keeps a keyboard cursor in window state: the Music list (`ui.cursor`, apart from the pick), `ui.track_cursor` (Hero tracklist), Home (`home_ui.cursor`, which settles the core's Home selection), Shows (list and Workspace `cursor`), Emby, Audiobooks, Podcasts, and Feeds. The per-destination key modules (`keys.rs`, `emby_keys.rs`, `shows_keys.rs`, `audiobooks_keys.rs`, `podcasts_keys.rs`, `feeds_keys.rs`, `keys_shared.rs`) act on the cursor row id. The pill rows keep a `focused_pill` (`pill_list.rs`, `shows.rs`, `podcasts.rs`).
- **Key path.** The root `main` holds the only `keydown` handler. `route_library_key` decides `Handled`, `Text`, or `Core`. A `Core` key goes through `CoreLink::key` to `CoreThread::route_key`, which hands the Queue leaf to `App::route_gui_input` only while `FocusStack::focused()` is `Queue`.
- **Hover.** The core resolves Queue row hover from its last pointer position against the hit map, and it re-resolves after each layout report (`CoreThread::apply_layout`). The Library has no Rust-side hover. It paints hover through the CSS `:hover` veil on its `button` rows.
- **Blitz hover after a scroll.** In blitz-dom 0.3.0-beta.4, `BaseDocument::resolve` calls `refresh_hover`, which re-resolves the hovered node from the last pointer position after layout or scroll. It sets the `:hover` state but dispatches no `pointerenter` or `pointerleave` event (`set_hover_to` only records the node). So a Dioxus `onpointerenter` handler goes stale after a wheel scroll under a still pointer, while `BaseDocument::get_hover_node_id` stays correct.
- **Launch location.** `mbv_config::GuiLaunchState.panel_focus: LaunchPanelFocus` (`library` or `queue`, default `library`) is saved at orderly exit (`src/app/gui_library/launch.rs`) and restored by `core_thread/launch_restore.rs`. The struct does not deny unknown fields.
- **Destination steps in the core.** The router's `NextLibraryTab`, `PreviousLibraryTab`, and `SetLibraryTab` reach `App::dispatch_navigation_command` (`src/app/dispatch/action.rs`), which runs the TUI's `library_tab_next`, `library_tab_prev`, and `set_library_tab`. Those skip the GUI's open step (`open_gui_active_destination`, `src/app/gui_library.rs`) and its presented-destination check (`presents_open_position`). Only the `OpenDestination` command runs both. Both `RouterOutcome::Command` and `RouterOutcome::PrefixDispatch` go through `App::apply_gui_router_command`.
- **Mounted surfaces.** `CoreThread::new` mounts `Queue`, `QueuePlaybackPanel`, `StatusBarPanel`, and `Library` in the `FocusStack`.
- The item menu builder (`App::gui_library_item_menu`) returns no menu unless Panel focus is Library.

## Goals / Non-Goals

**Goals:**

- One resolver for "what is under the pointer" that the key router reads, that is correct after a scroll, and that matches the painted `:hover`.
- Remove Panel focus, pane focus, Library cursors, and pill focus from the window. No window code reads a focus fact.
- Keep `FocusStack` and the shared keyboard router unchanged for the TUI. The GUI sets the stack's Panel focus per key, as an internal adapter.

**Non-Goals:**

- The Settings screen's sidebar and content cursors, its Esc ladder, prefix mode, the footer hint line, and the `[keys]` sharing. They stay as they are (later changes).
- The Queue's own keyboard model (cursor movement, marks, Visual mode). The Queue cursor stays as it is and is now named the active row.
- Native DOM focus (Tab between controls). The root-pinned DOM focus (`restore_root_focus`, invariant 33) stays.
- Any TUI change.

## Decisions

### D1. The pointer target comes from Blitz's hover node

After each window event, the launch copy (`window/launch.rs`, `MbvApplication::window_event`) reads `BaseDocument::get_hover_node_id`. It walks up the parent chain and builds a `PointerTarget`. It writes the value into a window store only when the value changed. A redraw is a window event, and `refresh_hover` runs inside its resolve, so the target follows a scroll. A pointer leave clears the hovered node, so the target becomes empty.

DOM markers name the targets. Each value is a fixed token, except the row id:

- `data-pointer-zone`: `library` on the Library column root, and `queue` on the Queue list box (the queue header, the filter, the rows, and the footer band; not the player block).
- `data-pointer-list`: the list kind on each list scroller: `music`, `home`, `shows`, `emby`, `audiobooks`, `podcasts`, `feeds` (the destination lists), and `tracklist` and `workspace` (inside the Hero).
- `data-pointer-row`: the row's id on each list row. The id is the same `row.id()` string that the cursor used, so each destination's existing id lookups (`rows_of`, `home_rows`, and the others) resolve it. The Audiobooks chapter rows have no id string (their cursor was a chapter index, `cursor: Option<usize>`): they carry the chapter index, and the Enter path parses it back.

```
PointerTarget {
    zone: Option<PointerZone>,        // Library | Queue
    list: Option<PointerList>,        // Music | Home | ... | Tracklist | Workspace
    row:  Option<String>,             // the hovered row's id
}
```

The walk is a pure function over the chain of attribute lookups (`pointer_target(chain)`), so a unit test covers it without Blitz. The launch copy is its only caller.

The store is a Dioxus `Signal<PointerTarget>` that the root provides as context. The launch copy writes it from the vdom runtime, in the same way that `apply_viewport_size` writes the viewport signal. Readers use `peek` only. The key router reads it at key time, and no component subscribes to it, so a hover change re-renders nothing. The hover paint stays the CSS `:hover` veil. Both read the same Blitz hover node, so the look and the key target always agree.

Alternatives considered:

- *Per-row `onpointerenter`/`onpointerleave`*: these events do not fire on a scroll under a still pointer (see Context), so Enter would act on a row that is no longer under the pointer.
- *Geometry derivation* (last pointer position, list rect, scroll offset, and row tops): this is exact only for the virtual lists, and it repeats hit testing that Blitz already does.
- *The core's hit map*: the Library reports no regions, and adding every Library row to the `LayoutReport` would put window-local interaction state back into the core (ADR 0036).

### D2. The window key router routes by pointer target

`route_library_key` keeps its early steps: a core overlay or prefix sends the key to the core with the zone target; the Queue filter's text entry sends it to the core as `Core(KeyTarget::Queue)` wherever the pointer is, so the keys that the filter does not consume still reach the `QueueLeaf`, as today; the Settings frame sends it to its router; `Ctrl+K`; the Search field; the Library filter. After those, the order is:

1. Esc closes the open Library row menu, then the item menu, then the Narrow Hero overlay, then the carried Search results, wherever the pointer is. With none open, Esc goes to the core.
2. The digits `1` to `7` open a destination while the Library panel shows.
3. With `zone == Library`, the active destination's keys run against the hovered row. `/` opens the list filter. Enter, `n`, and `a` act on `row` when `list` is the destination's list, the `tracklist`, or the `workspace`. With no row they are `Handled` and do nothing. The arrow, Page, Home, and End keys scroll the destination list when `list` is that list (D6). Every other key goes on to step 4.
4. Every other key goes to the core with its target: `KeyTarget::Queue` when `zone == Queue`, otherwise `KeyTarget::None`.

The window does not take Tab or Shift+Tab. They reach the core's `next_library_tab` and `previous_library_tab` (already `AllClients`). In the GUI, `App::apply_gui_router_command` intercepts `NavigationIntent::NextLibraryTab`, `PreviousLibraryTab`, and `SetLibraryTab`, in the same way that it intercepts `MoveQueueRow`. Next and Previous step through the presented destinations in tab order (Home first) and wrap. `SetLibraryTab(index)` acts as `OpenDestination { position: index }`. Each opens through the `OpenDestination` path (`presents_open_position`, `set_library_tab`, `open_gui_active_destination`), and never opens one that the GUI does not present.

A tab press also resets window state: `LibraryState::open_destination` (`window/library/state.rs`) sets `hero_dismissed`, clears `item_menu_open`, and clears `row_menu`. The core cannot reach that state. So the reset moves out of `open_destination` into `LibraryState::reset_for_destination`, and the window runs it when the carried `GuiLibraryList.current_tab` changes from the last value that it saw (one check where the Library panel reads the `library_list` section; the first value seen only sets the baseline). `open_destination` then only sends the command. A tab press, a window digit, a router step, and the launch restore's open all change `current_tab`, so they share one reset, and a router open looks exactly as a tab press does: in Narrow the new destination shows its list and no Hero overlay, and an open row menu or item menu closes. The TUI path does not change. `KeyOutcome::Core` becomes `KeyOutcome::Core(KeyTarget)`, so the forwarder sends the target with the key.

### D3. The core sets Panel focus per key, as an adapter

`CoreInput::Key` carries `KeyTarget`, and `CoreLink::key(key, target)` sends it. `CoreThread::route_key(key, target)` first calls `take_panel_focus` with `Queue` for `KeyTarget::Queue` and `Library` for `KeyTarget::None`. It skips the call when the value is already in effect. With Library Panel focus, `focused()` is `Library` (the core mounts it). `route_key` builds no leaf for `Library`, and the only Library-gated chord is `panel_left`, which D3 makes a no-op, so only the leafless global and transport chords act. That is the "global chords only" behavior. Do not unmount `Library`, and do not assert `UiRoot` in tests. The leaf is the `QueueLeaf` only for `KeyTarget::Queue` with no open overlay, which is the existing `focused() == Queue` check after the adapter write.

Each `GuiLibraryCommand` that the core applies first sets Library Panel focus through the same adapter. The item menu builder's Library gate then holds for a menu that the window asked for, with no change to the builder.

`PanelIntent::FocusPanel` (`panel_right`, `panel_left`) becomes a documented no-op in `App::apply_gui_router_command`, in the same way that `MoveQueueRow` is intercepted there. In `mbv-keybinds` the two actions become `ActionScope::TuiOnly`, so the GUI Settings key list no longer shows them.

The App-side effect of a Queue row click (`handle_mouse_single_click_queue` sets Queue focus) stays. The next key overwrites it.

Alternative considered: the core finds the target from its own `pointer_at` against its hit map. It holds no Library geometry, and two resolvers could disagree at a seam. The window has the one resolver (D1), so it sends the target.

### D4. The window keeps no focus

The frame's `focus` section is removed: `bridge/frame/focus.rs` (`GuiFocus`, `GuiSurface`, `GuiPanel`, and the overlay, modal, and popup mirrors), the `focus` slot in `src/sections.rs`, `bridge/sections.rs`, and the `Sections.focus` signal in `window/sections.rs`. The core still holds its `FocusStack`. `GuiLibraryCommand::TakeFocus` and `TakeQueueFocus` are removed with their senders (the press handler in `library_panel.rs` and the Tab walk), and so is the core's startup `take_panel_focus(PanelFocus::Queue)`. `LibraryPane`, `LibraryUiState.pane`, the pane lists (`PANES`, `HOME_PANES`, and the others), `active_library_panes`, `focus_library_pane`, and `move_library_pane` are removed.

### D5. Mini shows only the Queue panel

In the Mini arrangement the window shows only the Queue panel. The pick is one function in `window/app.rs`, `mini_shows_queue(arrangement) -> bool`, which returns `true` for Mini. The two visibility lines read it: `queue_visible = !panel_shows || arrangement != Mini || mini_shows_queue(arrangement)`, and the reverse for the Library. A later change adds a tabbed Mini view; it replaces only this function. This change adds no Mini view state, no switch, and no command. No key and no press changes the Mini panel. The pick lands in the same task as the per-key Panel-focus adapter (D3): after D3 Panel focus changes with every key, so a pick that still read `GuiFocus.panel` would flip Mini to the Library on any key with no target.

Launch location: `GuiLaunchState.panel_focus` is removed. `GUI_LAUNCH_STATE_VERSION` stays 1. The struct does not deny unknown fields, so a file that an earlier build saved still loads and its `panel_focus` is ignored. `LaunchPanelFocus` stays for the TUI launch state. The save writes no Panel focus, and the restore sets none: `apply_launch_panel_focus`, `launch_focus_taken`, and `note_launch_focus_taken` are removed, with the reset in `core_thread/settings.rs`.

Alternative considered: a Mini view in the core with a segmented switch. It is out of scope; a later change designs the tabbed Mini view.

### D6. Library lists lose the cursor; the keys scroll

The Music list keeps the pick, which is the core's Hero item. A track press picks its album and sets no cursor. The marker on the picked album stays. Home, Emby, Shows, Audiobooks, Podcasts, and Feeds keep their core select command on press (`HomeSelect`, `EmbySelect`, and the others), which already paints their "Hero item" marker. They lose the cursor and the settle-on-cursor-move path (`settle_home_selection_after` and its siblings). The filter narrows the rows and never selects. The letter rail press scrolls only. The Hero tracklist (`track_cursor`) and the Workspace tables (`cursor`, and the `press_*` cursor setters) lose the cursor. A single press on their rows does nothing. The double-click and the hover play button stay.

The scroll keys write the destination's `ListScroll.scroll_top`, which the virtual list already mirrors into the element (`library_virtual_list`). Up and Down move by the height of the row at the top edge. Page Up and Page Down move by the viewport height. Home and End go to the ends. The value is clamped to `[0, total − viewport]`. One function, `scroll_step(rows, view, ListMove) -> f32`, beside `list_window` in `window/library/list.rs`, serves every destination.

The pill rows lose `focused_pill`, `move_pill_focus`, and the Space and Enter pill selection. `request_*_pill_select` stays for presses.

The Library Status bar drops its `tab pane` hint pair.

### D7. The Queue paints its active row at all times

`queue_panel.rs` drops the `focused` input. `bars` becomes "marked, or the cursor slot". The four row states are distinct: the hovered row paints the `--canvas-hover-veil` (the veil that every hovered `button` already paints) in place of the `selected` fill, which it shares today. The active row paints the `selected` fill and a 4 px leading accent edge, the same edge as the Library's picked marker. A marked row paints the `selected` fill without the edge. The playing row keeps its play glyph. The cursor stays `Surface` state in `mbv-surfaces`, and only the name in the GUI docs changes to "active row".

### D8. Documentation

A new ADR, `docs/adr/0040-gui-key-target-follows-the-pointer.md`, records D1 to D3: the GUI Client has no Panel focus, the window resolves the pointer target from Blitz's hover node, and the core sets `FocusStack`'s Panel focus per key as an adapter. ADR 0035 stands for the TUI. An amendment to ADR 0036 notes that the window content no longer holds a cursor or focus inside the Library panel, and points at ADR 0040. `CONTEXT.md` gets the term *Pointer target*, and the *Panel focus* entry notes that the GUI Client has none. `docs/architecture/gui-client.md` replaces its key-order lists, the Library interaction table, and the Mini text.

## Risks / Trade-offs

- [A key in the same frame as a scroll uses the target from before the scroll] → A redraw follows each scroll, and `refresh_hover` runs in it. The window is stale for at most the time to one frame. A key that falls in that gap acts on the row that showed the `:hover` look when the key went down, which is what the user saw.
- [blitz-dom is a pre-release (0.3.0-beta.4), and `get_hover_node_id` or `refresh_hover` can change] → The walk is isolated in one pure function plus one call site in the launch copy, which already tracks the dioxus-native source. A unit test pins the walk, and the existing launch tests cover the call site.
- [The Library has no keyboard-only browse path apart from Search and the digits] → This is the accepted product direction of the change.
- [In Mini the Library does not show, and no key or press shows it] → Accepted until the later tabbed Mini view. A wider window shows the Library.
- [A press-to-focus habit: a user clicks the Queue and then types with the pointer resting over the Library] → The keys go to the Library, by design. The Queue's active row still shows, so the state stays visible.
- [Large test churn in `window/library/*_keys/tests.rs` and `keys/tests/panes.rs`] → Tasks rewrite one destination at a time. The Tab-walk and cursor-movement tests are deleted with their behavior. They are not kept or ported.
- [Files grow past 800 lines while cursor code is replaced] → The change removes more than it adds. Each task checks its touched files against the limit.

## Migration Plan

No data migration: an earlier launch file loads, and its `panel_focus` is ignored. Rollback is a revert. An older build that reads a file saved by this change finds no `panel_focus` key and falls back to Library focus, which is harmless.

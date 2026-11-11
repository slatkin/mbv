# Design

## Context

See `proposal.md` (Why) and the two delta specs for behavior. This section gives only the current state that shapes the approach.

- **F2, F3, and F4 resolve in the shared router but do nothing in the GUI.** The router turns `settings_open`, `sessions_open`, and `playlists_open` into `NavigationIntent::ToggleSettings`, `OpenSessions`, and `OpenPlaylists`. `App::dispatch` turns each into `App::request_sidebar_toggle`, which sets `App::pending_overlay`. Only the TUI shell drains `pending_overlay`, so in the GUI the request stays set and nothing opens. At first start, `construct/remote.rs` calls `App::open_services_settings` when `should_open_services` holds. That sets `pending_overlay` to `OpenSidebar(Settings)` and `settings_destination` to `Services`. The GUI ignores this too.
- **The settings model is in `App`, but much of the effect code is in the TUI `Model`.** `App::handle_settings_activate` runs every toggle and cycle, and every refusal, live effect, and deferred save. Setup forms (`App::setup.emby_setup_form`, `audiobookshelf_setup_form`) and their workers are in `App`. These parts run only in the TUI `Model`:
  - the multiselect item lists and commits (`shell/overlays/menus.rs`: `open_multiselect`, `commit_my_languages`, `commit_library_list_selection`);
  - the library routes editor (`open_library_routes`, `enter_device_stage`, `commit_device_selection`);
  - feed add, edit, and remove (`shell/feeds_manage.rs`, with the add worker's channel in `Model::feeds_manage`);
  - the playlist actions (`shell/playlists.rs`: `playlist_action`, `playlist_row_action`, and the rename and delete openers).
- **Keys are read-only in both Clients.** `mbv_keybinds::load(&RawKeybinds)` is the only validator. It rejects reserved chords and chords that cannot be parsed. It rejects a configured router chord that equals any other action's effective chord, configured or default, and prefix and prefix-namespace collisions. The GUI reads `config.keybinds` again on each key (`App::route_gui_input`). The TUI copies it once into `Model::keybinds`. `save_config_settings_at` writes `[keys]` from the in-memory `Config` on every settings save.
- **The GUI pattern for a GUI-owned surface exists.** ADR 0036 put the Library panel's interaction state in the window content. The core holds the data selection and runs actions through a fourth bridge command, `library(GuiLibraryCommand)`, which `App::apply_gui_library_command` applies (`src/app/gui_library.rs`).
- **There is no design canvas for Settings.** The archived `align-gui-with-design-canvas` canvas has only Main, Collapsed, and Menus.

## Goals / Non-Goals

**Goals:**

- One implementation of each settings effect, called by both Clients. The TUI's behaviour and look do not change.
- The Keys editor checks an edit with `load`, the validator that the config load already uses, and with no second rule set.
- A frame carries only the open section's data, and never a secret.

**Non-Goals:**

- No live reload of `[keys]` in a running TUI. It reads the file at its next start.
- No session-specific options in Sessions beyond selecting a target.
- No new Settings design canvas. The screen paints with the GUI's existing tokens and shared parts.
- No fix for the TUI's Log out row. It is filed as a separate bug.
- No guard against binding a chord that the GUI window content takes before the core (`Tab`, `1` to `7`, `/`, `Ctrl+K`) or that a leaf takes first. `load` accepts these chords today, and the editor applies the same rules.
- No protection against two Clients that save other settings at the same time. Only `[keys]` gets this protection (D8).

## Decisions

### D1. The window content owns the interaction state, and the core owns the open section (new ADR 0037)

The front end holds the sidebar cursor, the keyboard position (sidebar or content), each list's cursor and scroll, form drafts, choice marks before apply, the capture state, and in Mini the pane that shows. The core holds whether the screen is open, the selected section, the section's data, the form progress and errors, and every action.

The open section is the core's for two reasons. The open actions are rebindable `[keys]` actions that only the core router resolves. Some sections also start loads when they open: Playlists loads the playlist list, and Library loads the library list. A reload keeps the open section and drops the rest, which matches the `gui-settings-screen` spec.

*Alternatives.* Surface state in `mbv-surfaces`, which the issue proposed, is rejected for ADR 0036's reasons. There is also no shared TUI surface to share with, because the TUI keeps its sidebar. A front-end-only open flag is rejected because the window would need to know the user's `[keys]` chords.

ADR 0037, "The GUI Settings screen owns its interaction state", records the decision and points to ADR 0036. It does not amend ADR 0036.

### D2. The GUI tick host drains the sidebar requests

`ClientTickHost` gets no new hook. The GUI core thread takes `App::pending_overlay` once per tick, after input routing, and matches it exhaustively:

- `ToggleSidebar(Settings)` toggles the F2 group. `ToggleSidebar(Sessions)` and `ToggleSidebar(Playlists)` toggle their own sections. The toggle rule is in the spec: if the screen is closed, open it at the action's section. If the screen shows that action's group, close it. Otherwise show the action's section.
- `OpenSidebar(Settings)` opens the screen at the section that `settings_destination` names. `Services` opens Services, and `Main` opens Services. `Keys` opens Keys. `Panel` cannot occur in the GUI, so it maps to Services. This is how the first-start open reaches the GUI with no new code in `construct`.
- `DismissSidebar(Playlists)` closes the screen while it shows Playlists. `ReplacementExecutor::PlaylistsSidebar` raises this request after a playlist play, so the GUI closes the screen and the replaced queue shows, as the TUI closes its sidebar.
- Every other variant is a TUI overlay that the GUI does not present. Each one logs `gui.overlay_request.undelivered` at debug level and is dropped, with one documented arm for each variant and no wildcard.

This drain also fixes a stale-request hazard. `run_replacement` reads `pending_overlay.is_none()`, and the GUI never cleared that field before.

The state is `App::gui_settings: Option<GuiSettingsState>`, with `section: GuiSettingsSection` and the per-section caches of D9. Opening pushes `ComponentId::Overlay(OverlayId::Settings)` onto the core `FocusStack`, and closing pops it. The GUI has no `GuiSurfaceLeaf` for that id, so a key routes with no leaf. Global and transport actions fire. A queue-scoped gate such as `ClearQueuePrompt` reads a focus that is not the Queue and stays closed. Panel focus returns when the id is popped. Settings is not added to `is_blocking_overlay`.

### D3. Sections and scope declarations

`GuiSettingsSection` (in `src/app/gui_settings/`) is the closed list `Services, Playback, Display, Session, Library, Queue, Mpv, Feeds, Actions, Keys, Sessions, Playlists`, in sidebar order. Each F2 section's rows come from `SETTING_SECTIONS` in `mbv-ui-model`, filtered by D4. Services, Keys, and Panel are navigation rows in the TUI. In the GUI, Services and Keys are sidebar entries, and Panel is hidden. A section with no shown row is not listed.

### D4. Each setting and each key action declares its client scope

`mbv-ui-model` gets `pub enum SettingScope { AllClients, TuiOnly, Inert }` and `SettingKey::scope(self) -> SettingScope`, an exhaustive match with no wildcard:

- `TuiOnly`: `Panel`, `PanelSide`, `PanelWidth`, `PanelWidthExpanded`, `PanelReserve`, `PanelGutterTop`, `PanelGutterBottom`, `PanelGutterLeft`, `PanelGutterRight`, `PanelAccent`, `PanelAccentColor`, `PanelAccentWidth`, `ImageProtocol`, `MouseSupport`.
- `Inert`: `LogOut`. No Client acts on it. The separate bug owns it.
- `AllClients`: every other key.

The GUI shows only `AllClients` rows. The TUI ignores the scope.

`KeybindAction` gets a declared field, `scope: ActionScope { AllClients, TuiOnly }`, in the one `KEYBIND_ACTIONS` table. An action is `TuiOnly` when its routed command has no effect in the GUI Client: the command is a `Command::Shell`, which `apply_router_command` leaves undelivered, or an `App` command whose state the GUI does not present. These actions are `TuiOnly`:

- `help_open`, `twitch_url_open`, and `f5_refresh` (shell commands);
- `search_open` (the TUI Search sidebar; the GUI's Search is `Ctrl+K` in the window content);
- `panel_mode_cycle_x` (Panel mode is TUI state);
- `ctrl_l_force_clear` (a terminal redraw);
- `visualizer` and `hide_visual_slot` (the GUI paints no visualizer and no hidden visual slot).

Every other action is `AllClients`. The Keys section lists only `AllClients` actions. A collision with a `TuiOnly` action is still refused, because the file is shared and `load` checks every action. The refusal names the action, so the user can find it in the TUI.

### D5. A fifth bridge command, `settings(GuiSettingsCommand)`

**Grant rule (live finding).** Tauri's ACL sits between `invoke` and the command. A command that `generate_handler!` registers but `build.rs`'s app manifest does not declare, or that `capabilities/default.json` does not grant, is refused before the core sees it. The front end's fire-and-forget invoke then logs only to the webview console. The first build of this change shipped `settings` in neither file, so every Settings action died silently, by pointer and by key. `crates/mbv-gui/tests/capabilities.rs` now reads the command list from `generate_handler!` in `src/main.rs` and asserts that each command is declared and granted once. A new command cannot repeat the miss, because no list in the test names one.

`src/main.rs` registers `settings`. It only pushes `CoreInput::Settings`, as `library` does. `src/bridge/settings.rs` translates the wire form into `mbv::GuiSettingsCommand`, and `App::apply_gui_settings_command` (`src/app/gui_settings.rs`) applies each variant through the shared `App` method. Identities are stable, never row positions. A command built from a stale frame logs at debug level and changes nothing, as the `library` arms do.

| Variant | Carries | Applies through |
|---|---|---|
| `close` | none | close the screen (D2) |
| `selectSection` | `section` | set `section`, then start the section's load (D9) |
| `activate` | `setting` (the `SettingKey` name) | `App::handle_settings_activate`, refused for a key whose scope is not `AllClients` |
| `applyChoices` | `kind`, `selected` (choice ids) | `App::commit_settings_choices` (D7) |
| `assignRoute` | `library` (lowercased name), `endpoint` or none | `App::assign_library_route` (D7) |
| `routeDevices` | `library` (lowercased name) | `App::route_devices(library)` (D7); the candidates are cached in `GuiSettingsState` and projected until the section closes |
| `embySetup` | `serverUrl`, `username`, `password` | fill `EmbySetupForm`, then `App::submit_emby_setup` |
| `audiobookshelfSetup` | `serverUrl`, `apiKey` | fill the form, then `App::submit_audiobookshelf_setup` |
| `serviceAction` | `ServiceActionIntent` name | `App::route_service_action`, except `ManageFeeds`, which selects the Feeds section: routing it would raise `OpenFeedsManage`, which the D2 drain drops |
| `cancelSetup` | `service` | `App::cancel_emby_setup` or `App::cancel_audiobookshelf_setup` |
| `feedAdd` | `name`, `url`, `kind` | `App::begin_feed_add` (D7) |
| `feedEdit` | `url` (identity), `name`, `kind` | `App::edit_feed` (D7) |
| `feedRemove` | `url` | `App::request_feed_removal` (confirm modal) |
| `keysCapture` | `target`, `key` (a `DomKey`) | D6 |
| `keysReset` | `action` | D6 |
| `keysClear` | `target` (a prefix-namespace chord or the prefix) | D6 |
| `selectTarget` | `GuiTargetKey` | `App::select_chooser_target` |
| `playlistOpen`, `playlistBack`, `playlistRefresh` | `id` or none | the moved playlist methods (D7) |
| `playlistAction`, `playlistItemAction` | `id` and the item `id`, plus `play`, `shuffle`, or `enqueue` | the moved playlist methods (D7) |
| `playlistRename` | `id`, `name` | `App::spawn_rename_playlist` |
| `playlistDelete` | `id` | the TUI's `ConfirmAction::DeletePlaylist` prompt |
| `resetUiState` | none | D10 |

Feeds are addressed by URL, because the config list has no other stable identity and the TUI's index can move.

### D6. The Keys editor checks an edit by running `load`

`mbv-keybinds` gets four pure functions on `Keybinds`, and none of them checks anything:

- `with_router(action, chord) -> Keybinds` stores the override under the action's declared section. A chord equal to the action's only default chord removes the override.
- `with_prefix_assignment(action, Option<Chord>) -> Keybinds`.
- `with_prefix(Option<Chord>) -> Keybinds`.
- `without_router(action) -> Keybinds`.

`Keybinds::to_raw() -> RawKeybinds` is the inverse of `load`. The candidate is accepted only when `load(&candidate.to_raw())` returns `Ok`. The loaded value then replaces `config.keybinds`. This keeps one rule set, as the `configurable-keybinds` delta requires. `KeybindsError`'s `Display` already names the entry and the other action for each collision kind, and the frame carries that text as the refusal.

Capture flow. The front end sends the raw `DomKey` of the press. `src/bridge/keys.rs` translates it to a `KeyEvent`, as for every key. `KeyChord::new` normalizes it, and a new `KeyChord::to_keybinds_chord(self) -> Option<mbv_keybinds::Chord>` (the inverse of `from_keybinds_chord` in `src/app/input/resolver.rs`) gives the configured chord. `None` is refused because the chord grammar cannot name the key. The window never forms a chord string, so the two Clients cannot disagree on a chord's spelling. Esc ends the capture in the front end and is never sent. The front end shows the capture until the next frame answers it.

Persistence. `mbv-config` gets `ConfigSection::Keys`, which `save_config_section` writes through the existing `write_keys_section`. `save_config_settings_at` stops calling `write_keys_section`, so a settings save leaves `[keys]` as the file has it (D8). An accepted edit saves at once, with no debounce, because the spec says that the file holds the edit when the next key fires.

*Alternative.* A separate editor-time validator in the GUI is rejected because it would be a second rule set. A free-text chord field is rejected because a typed spelling can differ from the router's normalization.

### D7. Shared effects move from `Model` to `App`

Each move keeps the TUI's behaviour. The TUI shell keeps only its component work (mount, stage, cursor) and calls the moved method:

- **Choice lists.** `App::settings_choices(kind) -> Result<Vec<SettingsChoice>, ..>` builds the items that `open_multiselect` builds today. `App::commit_settings_choices(kind, selected)` runs the two commits and returns the refetched Home content for a library kind. The TUI assigns that content through `Model::assign_home_content`, and the GUI assigns it to the Home store that its Library reads.
- **Library routes.** `App::route_libraries()`, `App::route_devices(library)`, and `App::assign_library_route(library, endpoint)` take the bodies of `open_library_routes`, `enter_device_stage`, and `commit_device_selection` without the component calls. The refusal for a device with no endpoint stays in `assign_library_route`.
- **Feeds.** The add worker's channel and its pending id move from `Model::feeds_manage` to an `App` field. `App::begin_feed_add`, `App::edit_feed`, `App::request_feed_removal`, and `App::drain_feed_add_results` replace the `Model` methods. The `drain_feed_add_results` hook of `ClientTickHost` already exists in both hosts, and both now call the `App` drain. The TUI host then updates the component's stage from the returned outcome. The GUI frame shows the pending add from the `App` field.
- **Playlists.** `playlist_action`, `playlist_row_action`, `shuffle_queue_from_playlist`, `replace_queue_from_playlist`, and `enqueue_playlist_items` move to `App`, keyed by playlist id and item id instead of the TUI's index. The TUI shell resolves its index to the id before it calls.

### D8. A settings save leaves `[keys]` as the file has it

`save_config_settings_at` no longer writes `[keys]`, and only `ConfigSection::Keys` writes it. Today every full save writes the in-memory bindings, so a TUI that loaded its bindings before a GUI edit would put back its old copy at its next F2 change. Neither Client edits bindings through a full save, so removing the write loses nothing. The existing tests that expect a full save to round-trip `[keys]` move to the section save.

### D9. The frame carries the open section only

`GuiFrame.settings: Option<GuiSettingsFrame>` is `None` while the screen is closed. While it is open, it carries `section`, `sections` (the shown list from D3 and D4), and `content`, a tagged union with one variant for each section:

- **F2 rows**: each row's `SettingKey` name, its kind (toggle, cycle, choice list, or action), its value text or flag, and, for a choice list, its choices once loaded. Every F2 section except Library uses this variant.
- **Library**: the section's choice rows (`HiddenLibraries`, `FeedViewLibraries`) with their choices once loaded, plus `routes`: the routable libraries with each one's assigned device, the selected library, and its device candidates once `routeDevices` runs for it. The `LibraryRoutes` F2 row is not projected in the GUI: its TUI popup's work lives inline in this section, and its `activate` would raise `OpenLibraryRoutes`, which the D2 drain drops.
- **Services**: each Service's state label, its action labels and intents, and the setup form's `busy`, `error`, and server URL. The form never carries a password or an API key. The same rule covers logs: a file-backed debug log, the GUI Client log included, never carries a setup secret — a password, an API key, or a Service credential — such as through a `Debug` formatting of a command that carries one.
- **Feeds**: the subscriptions (name, URL, kind) and the pending-add flag.
- **Keys**: the prefix chord, then one row for each `AllClients` action. Each row has the id, the label, the section, the effective router chords, the prefix-namespace chord, the changed-from-default flag, and the prefix-addressable flag. The frame also carries the last refusal (action and text) until the next edit.
- **Sessions**: the `GuiTargetChoice` list that the chooser uses.
- **Playlists**: the list (id, name, item count, loaded flag), the open playlist and its items, and the loading flags.

The section's load starts on `selectSection` and when the screen opens:

- Playlists runs `App::spawn_load_playlists`.
- Library runs `App::route_libraries` and `App::settings_choices` for its two library kinds. Selecting a library in the routes list sends `routeDevices`, which runs `App::route_devices(library)`; the candidates are cached until the section closes.
- Playback runs `App::settings_choices(MyLanguages)`.

The results are cached in `GuiSettingsState` until the screen closes. The status line paints from the frame's existing `status` field (the shared Status bar projection). So the screen shows toasts and the prefix-armed state with no new projection.

### D10. Reset UI State in the GUI

`App::reset_gui_ui_state` (with the core-thread parts in `CoreThread`) does the following, in this order:

1. It abandons the pending GUI launch restore and selects Home through `set_library_tab(0)`.
2. It clears every destination's kept collection and pick in the GUI Library store.
3. It resets the Queue Surface state's cursor, marks, and viewport, and sets the collapse and fold toggles off.
4. It closes the overlays and the Settings screen.
5. It deletes the GUI launch-location file through a new `mbv_config::clear_gui_launch_state`.
6. It increases the frame's `uiResetEpoch` counter.

When the front end sees a new epoch value, it resets its Library interaction stores and its panel widths, and removes `mbv.gui.panelWidths` from local storage. It does not call `mbv_config::clear_saved_ui_presentation_state`, which is the TUI's. The TUI's `Model::reset_ui_state` does not change.

### D11. The front end

`ui/src/lib/settings/` holds the store and the key router. `ui/src/lib/components/Settings*.svelte` holds the screen, the sidebar, one component for each section, and the status line. `App.svelte` mounts `SettingsScreen` in place of the two columns while `frame.settings` is present.

- **Fill.** `SettingsScreen`'s root is a flex item of `App.svelte`'s row container, so it carries `flex-1 min-w-0`. Without them it took its content's width (631 px of a 1400 px window in the live finding), and the content pane shrank to a strip. jsdom has no layout, so no test can measure this. The user's live run is the layout gate. No front-end test runs in a real browser.
- **Pointer contract.** Every control is a native `button` or `input` whose press runs the same store function as the keyboard path, so the two paths share one effect. A press moves the store's cursor and pane to the pressed item. Hover feedback paints for each control that can act, and an unavailable control is a disabled `button`, so a press on it never runs its handler. The window forwards each press to the core as well (`pointer`). The Queue and Library panels retract their reported regions on unmount, so while the screen is open a press resolves to no core surface. A core test pins this.
- **Choice kind.** Each projected choice-list row carries its apply kind in the frame. The front end reads that kind and does not derive it again from the setting name (the open review finding on `choicesKindOf`).
- **Widths.** The sidebar takes the Queue column's expanded width: the dragged or default width, whatever the collapse toggle, because the 72 px rail cannot hold the list. No divider shows. Mini is the existing `libraryArrangement` threshold, measured as the window width minus that width.
- **Paint.** The screen paints with the existing `app.css` tokens and the shared Library parts (rows, list headers, scrollers, buttons). Icons are Lucide only.
- **Key routing order.** This is added to `routeLibraryKey`'s chain while `frame.settings` is present, and the Library steps do not run:
  1. A core overlay or prefix mode takes the key, as today.
  2. An active capture takes every key. Esc cancels the capture, and any other key sends `keysCapture`.
  3. A focused text field takes the key. Esc cancels the form, and Enter submits it.
  4. The screen keys run. Up, Down, Page Up, Page Down, Home, and End move in the focused pane. Tab and Shift+Tab switch between the sidebar and the content. Enter opens the sidebar entry or activates the content row. Space activates a focused toggle or choice mark. Esc follows the spec's ladder.
  5. Every other key goes to the core, so Space on the sidebar plays or pauses.

### D12. No new dependency

The change adds no crate and no npm package. It uses what the workspace and `ui/package.json` already pin: `toml` and `serde` on the Rust side, and `svelte` 5.57.3, `bits-ui` 2.20.6 (the switch, checkbox, and text-input primitives), `@lucide/svelte` 1.70.0, and `tailwindcss` 4.3.4 in the front end. No upgrade is part of this change.

## Risks / Trade-offs

- **[Risk] Opening Library, routes, or a device list stalls the GUI's frames for one Emby request.** `get_views` and `fetch_sessions_blocking` are blocking calls, and the TUI makes them on its UI thread today. → They run only on an explicit section open or route edit, the results are cached until the screen closes (D9), and moving them to a worker would change the TUI too, so it stays out of scope.
- **[Risk] A user binds a chord that the GUI window takes first.** Examples are `1` to `7` or `/`, or a TUI leaf key. `load` accepts it, but the window content consumes it before the core. → This is listed as a non-goal. The Keys row still shows the chord, so the user sees what they set.
- **[Risk] A retired action id stays in the file longer.** A full save used to drop it. → `load` already ignores it with one warning, and the next Keys edit rewrites the table.
- **[Risk] The `Model` to `App` moves change TUI code paths.** → Each move is a separate task with no behaviour change, and the existing TUI tests for multiselect, routes, feeds, and playlists stay green before the GUI calls the moved method.
- **[Trade-off] The sidebar ignores a collapsed Queue panel.** Its width is the expanded Queue column's width. → The rail width cannot hold section names, and closing the screen restores the collapsed state.

## Migration Plan

No data migration. `[keys]` keeps its file shape, and old configurations load unchanged. The Owner handshake refuses a mixed build, so an old binary that still writes `[keys]` on a full save cannot run beside this one. Rollback is a revert of the change. A `[keys]` table that the Keys editor wrote is valid input to the old loader.

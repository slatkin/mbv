# Proposal

## Why

The GUI Client has no settings. F2, F3, and F4 do nothing in it: the core router resolves them, but only the TUI shell drains the sidebar request they raise. Issue #988 asked for the TUI's F2 sidebar at parity. The GUI no longer keeps parity with the TUI (`docs/architecture/gui-client.md`, ADR 0036), and a terminal sidebar of rows is a poor fit for a window. The GUI gets one Settings screen instead. It holds what the TUI spreads over three sidebars, and it makes key bindings editable for the first time.

## What Changes

- **One GUI Settings screen.** While it is open, the Queue column becomes the Settings sidebar and the Library column becomes the section content. No player shows while the screen is open. Playback continues, and the core's global and transport keys still work while no text field or chord capture holds the keyboard.
- **Opening and closing.** The `settings_open` action (F2 by default) opens the screen at the first F2 section. `sessions_open` (F3) opens it at Sessions, and `playlists_open` (F4) opens it at Playlists. Esc from the sidebar, or the same action again, closes it and restores the panels as they were. At first start with no Remote Service configured and no feed subscriptions, the screen opens at Services, as the TUI opens its Services view. The header's playback-target chooser does not change.
- **One sidebar entry for each F2 section.** Services, Playback, Display, Session, Library, Queue, Mpv, Feeds, Actions, and Keys each get their own section screen, followed by Sessions and Playlists.
- **TUI-only settings are hidden.** The GUI hides the settings that have no effect on it: the Panel page (pinwin geometry, reserve, and accent), Image protocol, and Mouse support. It also hides the key actions that have no effect in the GUI. Each setting and each key action declares this once, beside its own declaration.
- **Log out is hidden.** The TUI's Log out row sets a flag that nothing reads, so it does nothing. The GUI does not show it. Fixing or removing the TUI row is a separate bug, not this change.
- **Keys becomes an editor.** You capture a new chord for an action, set or clear its prefix-namespace chord, set or clear the prefix chord, and reset an action to its default. The editor checks every edit with the same rules the config load applies. An edit that collides with another action's chord is refused, and the message names that action. A valid edit is saved to `[keys]` at once and takes effect in the GUI at once. A TUI uses it at its next start.
- **A settings save no longer writes `[keys]`.** Only the Keys editor writes it. Without this, a running TUI's next settings save would put back the bindings it loaded at start.
- **Reset UI State has a GUI counterpart.** In the GUI it resets the GUI's own presentation state: the Library returns to Home with no saved picks, the Queue's cursor, marks, and scroll and the collapse and fold toggles return to defaults, the panel widths return to defaults, and the GUI launch location is cleared. Media, settings, queue contents, and playback do not change. The TUI's saved state is not touched.
- **Sessions is basic.** It lists the same playback targets as the header chooser and selects one. Later work adds session-specific options.
- **Playlists is a temporary home with the full TUI set.** It lists the saved playlists, opens one to its items, plays, shuffles, or adds a playlist or one item, renames, deletes, and refreshes.
- **Shared effects move from the TUI shell to `App`.** The setting-choice lists (Hidden libraries, Feed view libraries, My languages), the library routes editor, feed add, edit, and remove, and the playlist actions run today only in the TUI `Model`. They move to `App` so both Clients run one implementation. The TUI's behaviour does not change.
- **Colour by data type across the whole GUI (user-authorized scope).** Weight and size carry hierarchy, and colour carries data type: time, date, count, section label, secondary title, and role each get one colour. A repeated time or year in a list is dimmed. This applies to the Queue, the Library, and Settings, following the colour pass on the three design canvases (`canvas/colour-by-type.diff`). The TUI does not change.
- **A new ADR** extends ADR 0036's rule to the Settings screen: the GUI window content owns the screen's interaction state, and the Client core owns the open section, the data, and the actions.

## Capabilities

### New Capabilities

- `gui-settings-screen`: the GUI Client's Settings screen: its arrangement, opening and closing, sidebar sections, the settings it shows and hides, Service setup, feed subscriptions, the Keys editor, Sessions, Playlists, the GUI's Reset UI State, keyboard reach, and the ownership of its state.

### Modified Capabilities

- `configurable-keybinds`: bindings become editable from the GUI Client's Keys editor, with the load-time rules applied to each edit. A settings save leaves `[keys]` as the file has it, and only the Keys editor writes it.

## Impact

- `crates/mbv-gui/`: a new frame section (`GuiSettingsFrame`), a fifth bridge command (`settings`), a `CoreInput::Settings` variant, the core thread's drain of the sidebar requests that F2, F3, and F4 raise, and the Settings focus in the core's `FocusStack`. In `ui/`: the Settings screen component and its sections, its key router, and its arrangement in `App.svelte`.
- `src/app/`: a new `gui_settings` module of `App` methods for the bridge command. The TUI `Model` effects listed above move to `App`, and the TUI shell calls the moved methods.
- `crates/mbv-ui-model/`: a declared client scope for each `SettingKey`.
- `crates/mbv-keybinds/`: a declared client scope for each `KeybindAction`, and a conversion from compiled bindings back to the raw `[keys]` form so that an edit is checked by `load`.
- `crates/mbv-config/`: `ConfigSection::Keys`, a full settings save that leaves `[keys]` as the file has it, and a clear for the GUI launch location.
- `src/app/input/resolver.rs`: the inverse of `KeyChord::from_keybinds_chord`, so a captured key becomes a configured chord.
- Docs: a new ADR, `docs/architecture/gui-client.md` (a Settings screen section), `docs/architecture/interactive-surface-ledger.md` (a GUI Settings screen row), and `CONTEXT.md` (the *Settings screen* term).
- Colour pass: `mbv-ui-model` (a `MetaKind` for typed segments), `mbv-render`'s shared Hero meta-row builder (typed segments; the TUI joins them to the same text), the GUI Library and Queue frame DTOs, and the Queue, Library, and Settings Svelte components.
- No new dependency.

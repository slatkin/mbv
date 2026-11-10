# Proposal: align-gui-library-keys

## Why

The user rule says: "all shortcut keys should match unless specifically authorised." The GUI Library panel hardcodes every key it takes in the window content (`crates/mbv-gui/ui/src/lib/library/keys.ts` and its per-destination siblings); none of them reads `[keys]`. Ten of the chords it takes differ from the TUI binding for the same action, several GUI-only keys collide with TUI chords through the GUI-only core literals in the key policy, and three search and menu chords that the TUI answers have no GUI path at all. Two chord sets are also duplicated in code instead of shared: the confirm modal's key set (`src/app/gui_client_state/overlay.rs:203-233` re-derives the shared confirm chord set and drops Shift+D) and the confirm button's hint parser (`src/app/gui_client_state/confirm.rs:46`).

## What Changes

- The GUI Library panel takes the TUI's chord for every action the TUI binds, and reads the configured chord from `mbv-keybinds` where the action is a registry action, so user-configured `[keys]` apply. Window keys that the front end hardcoded (`Tab`, `Shift+Tab`, the digits, `Ctrl+K`) stop shadowing their registry actions.
- GUI-only keys that collide with a TUI chord in the same context (pane cycle on `Tab`, pill select on `Space`/`Alt+Space`/`Enter`, playback-target chooser on `r`, queue fold on `q`, panel collapse on `z`, deep seek on `[`/`]`) move off the TUI's chords to authorised new defaults, declared as `KeyGate::GuiClient` registry actions so a user can rebind them. The user approved all eight proposals as written on 2026-11-10; `design.md` D3 records each authorisation.
- GUI-only actions without a TUI key get authorised keys: play next (`n` kept), the Home row's add (`a` kept) and play next (`n`), and the focused Collection pill's select key (`l`).
- The TUI chords that the GUI never answered gain a GUI path: the search-result `Ctrl+P`/`Ctrl+S`/`Ctrl+A` family, the Search field's `Tab`/`BackTab` type-filter cycle, and `.` for the item context menu.
- The Search field's visible hint names the configured `search_open` chord read from the keybinds, not the `Ctrl K` literal.
- The confirm chord set and the confirm button hint parser move into one shared module (`mbv-ui-model::confirm`), which the TUI's Confirm component, the GUI overlay's key branch, and the GUI's button press all use, so Shift+D answers the dirty-playlist prompt in the GUI again.
- `Esc` with nothing panel-owned open reaches the core, so the leaf back and the deferred stop work under Library Panel focus as in the TUI.

## Capabilities

### New Capabilities

None. The alignment lands in existing capabilities.

### Modified Capabilities

- `gui-client`: the keyboard-matching requirement gains the Library panel's chord alignment, the search and confirm chord scenarios, and the hint-from-keybinds rule.
- `configurable-keybinds`: a GUI-only action's default chord may not shadow a TUI surface's own chord, and the deep-seek and GUI-literal chords follow that rule.

## Impact

- `crates/mbv-gui/ui/src/lib/library/keys.ts` and its five per-destination siblings: most hardcoded chords go, the shared chord table replaces them, and the take/forward routing shrinks to genuinely GUI-owned keys.
- `crates/mbv-gui/src/bridge/`: the frame carries the configured chords the front end must match, and `bridge/keys.rs` names any new chord keys.
- `crates/mbv-keybinds/src/registry.rs`, `src/app/input/key_policy.rs`, `src/app/input/key_policy/commands.rs`: the deep-seek chord defaults move, the GUI-literal entries become registry-backed or drop, and the new GUI-only actions follow the *Add a keybind action* recipe (`docs/recipes.md`).
- `crates/mbv-ui-model/src/confirm.rs`: gains the shared confirm chord set and hint parser; `crates/mbv-components/src/confirm.rs`, `src/app/gui_client_state/overlay.rs`, and `src/app/gui_client_state/confirm.rs` switch to it.
- `crates/mbv-gui/ui/src/lib/components/LibraryTopBar.svelte`: the Search hint binds to the carried chord instead of a literal.
- `docs/recipes.md`: the keybind recipe gains the GUI aside this change exercises.
- No Owner-side, ctrl, network, or TUI-behaviour change. The TUI's chords and defaults stay unchanged.

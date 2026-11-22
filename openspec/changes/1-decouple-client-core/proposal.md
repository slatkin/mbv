# Proposal: 1-decouple-client-core

## Why

The TUI Client and the GUI Client are not separated in the code. The GUI program links the root `mbv` package, and that package is the TUI. About 14k lines of GUI-only code (`src/app/gui_*`) are in the TUI package as `impl App` blocks, with `gui_*` fields on the shared `App` and GUI variants in the shared event enum. Shared state carries TUI types (ratatui `Span`, `Rect`, terminal-cell anchors), and shared leaf crates (`mbv-theme`, `mbv-images`) carry ratatui and crossterm. The core branches on which Client runs (`App::gui_client`, `ClientKind`). Because of this, agents mix up the two Clients when they read instructions and code.

This change is the first of three sequential changes:

1. `1-decouple-client-core` (this change): the shared code loses its TUI types and its GUI state, and names no Client. Everything stays in the root package.
2. `2-split-tui-gui-clients`: the core, the TUI, and the GUI become separate crates.
3. `3-add-owner-program`: the Owner process runs from its own `mbv-owner` program.

This change has value alone: after it, shared code names no toolkit and no Client, and GUI state lives only in GUI code. It also makes change 2 possible, because the crate split needs a core that holds no Client code.

## What Changes

- Add a TUI-only leaf crate, `mbv-tui-support`. `mbv-theme` and `mbv-images` lose their ratatui and crossterm parts to it.
- Move the toolkit-free logic and data that the core or the GUI uses from `mbv-render` and `mbv-components` down to `mbv-ui-model`. GUI code then imports neither TUI crate.
- Move the TUI-only code in shared modules under the TUI shell (`src/app/shell/`): the ratatui projections, the terminal infrastructure and image protocol, the crossterm key edge, the tab strip, `AppLayout`, and the terminal size.
- Shared calls and shared state lose terminal cells. A context menu opens "at the pointer that the Client recorded", and each Client places it.
- A neutral `ClientCapabilities` flag set replaces `App::gui_client` and `ClientKind`. The core reads a capability, never a Client kind.
- GUI state leaves `App`: the `gui_*` fields go into `GuiClientState`, held by a new `GuiCore { app, gui }`. The GUI `impl App` blocks become `impl GuiCore` blocks. The three GUI events leave the shared event enum. The core calls no GUI method.
- The router gets a `pub` snapshot entry, so the GUI routing entry reads no router field.
- Docs: a new ADR, a *Client capability* term in `CONTEXT.md`, and updated architecture docs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `client-interaction-core`: shared Client state uses neutral values, and the core names no Client.

## Impact

- Code: `src/app/` (shell, dispatch, state, input, infra, GUI modules), `src/gui.rs` (new, `GuiCore`), `crates/mbv-gui`.
- Crates: new `mbv-tui-support`. Changed `mbv-theme`, `mbv-images`, `mbv-ui-model`, `mbv-render`, `mbv-components`, `mbv-gui`.
- Dependencies: no new external dependency. `ratatui` and `ratatui-image` move from `mbv-theme` and `mbv-images` to `mbv-tui-support` at their current versions.
- Packaging: none.
- Users: no change to keys, screens, commands, or prefs.
- Docs: `CONTEXT.md`, `docs/architecture/gui-client.md`, `docs/architecture/tui-frontend.md`, and a new ADR.

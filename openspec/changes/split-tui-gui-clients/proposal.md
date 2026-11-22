# Proposal: split-tui-gui-clients

## Why

This is change 2 of 3. It starts after `decouple-client-core` is merged. After that change, the shared code holds no TUI type and no GUI state, and names no Client. But the boundary is still a convention: the TUI shell and the GUI modules sit inside `crate::app` and reach `App` internals directly, the GUI program still links the root `mbv` package (and with it ratatui, crossterm, and TuiRealm), and every GUI edit recompiles the TUI. Agents still find both Clients in one `src/` tree. The archived `extract-client-core` change (design D2) left this step for later.

This change makes the boundary a compile error: the Client core, the TUI, and the GUI become separate crates, and no Client crate can name the other. The third change, `add-owner-program`, then moves the Owner process out of the `mbv` program.

## What Changes

- Move the shared helpers that later crates need into leaf crates: launch helpers and the pinwin request runner into `mbv-desktop`, the Owner lock into `mbv-daemon`, the log-level parser into `mbv-core`, and the image disk cache into `mbv-config`.
- Build the `pub` access surface of `App` that each Client needs, and prove it with a boundary rehearsal inside the root package: the shell moves to `crate::tui` and the GUI modules to `crate::gui`, so the compiler rejects every private access before any crate exists.
- Add a `mbv-client-core` crate. It holds `App`, dispatch, neutral state and events, the shared keyboard router, the Client tick, and Owner find, start, and attach. It has no terminal library, no terminal UI framework, and no GUI toolkit in its dependency graph.
- Move the TUI into a `mbv-tui` crate (program name stays `mbv`): the shell, the crossterm key edge, the TUI projections, the pinned panel, and, until `add-owner-program`, the Owner host behind the hidden `--__local-daemon` flag. `mbv-components` and `mbv-render` stay TUI-only crates.
- Move all GUI-only code into `mbv-gui`, which depends on `mbv-client-core` instead of the root package.
- The root package becomes a virtual workspace. No Client code stays in the root `src/`.
- Narrow every item that the rehearsal widened and no Client needs, and give neutral names to the last `Gui*` items in the core.
- Docs: a new ADR, one agent guide for each Client crate, a root `AGENTS.md` that describes the shared core, and updated `CONTEXT.md` and architecture docs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `client-interaction-core`: the Client core is its own crate with no Client toolkit, and no Client imports another Client's code.
- `gui-client`: the GUI program links no terminal UI library.

## Impact

- Code: almost all of root `src/` moves into `crates/mbv-client-core`, `crates/mbv-tui`, and `crates/mbv-gui`. The root `Cargo.toml` becomes a virtual workspace.
- Crates: new `mbv-client-core`, `mbv-tui`. Changed `mbv-gui`, `mbv-desktop`, `mbv-daemon`, `mbv-core`, `mbv-config`.
- Dependencies: no new external dependency. Existing workspace dependencies move between crates at their current versions.
- Packaging: the `.deb` metadata moves to `crates/mbv-tui/Cargo.toml`; CI and the `Makefile` use `-p mbv-tui`. The installed programs do not change (`mbv`, `mbv-gui`).
- Users: no change to keys, screens, or commands. `mbv`, `mbv --pin`, `mbv -q`, `mbv --toggle`, and `mbv-gui` act as before.
- Docs: `AGENTS.md`, `CONTEXT.md`, `docs/architecture/tui-frontend.md`, `docs/architecture/gui-client.md`, `docs/architecture/interactive-tui-component-map.md`, `docs/recipes.md`, and a new ADR.

# Proposal: split-tui-gui-clients

## Why

The TUI Client and the GUI Client are not separated in the code. The GUI program links the root `mbv` package, and that package is the TUI. About 14k lines of GUI-only code (`src/app/gui_*`) are in the TUI package as `impl App` blocks, with `gui_*` fields on the shared `App` and GUI variants in the shared event enum. Shared state carries TUI types (ratatui `Span`, `Rect`, terminal-cell anchors). The GUI also starts the TUI program (`mbv --__local-daemon`) to host the Owner process. Because of this, agents mix up the two Clients when they read instructions and code. The archived `extract-client-core` change (design D2) left this step for later.

## What Changes

- Add a `mbv-client-core` crate. It holds the code that both Clients use: `App`, dispatch, neutral state and events, the shared keyboard router, the Client tick, and Owner find, start, and attach. It has no terminal library, no terminal UI framework, and no GUI toolkit in its dependency graph.
- Move the TUI into a `mbv-tui` crate (program name stays `mbv`): the shell, the crossterm key edge, the TUI projections, the terminal image protocol, the ratatui colour conversion, and the pinned panel. `mbv-components` and `mbv-render` stay TUI-only crates.
- Move all GUI-only code into `mbv-gui`: `gui_library`, `gui_settings`, `gui_playlists`, `gui_client_state`, and the GUI routing entry. `App` loses its `gui_*` fields and its GUI event variants.
- No Client crate depends on the other Client crate. Code that both Clients use lives in `mbv-client-core` or a shared leaf crate, and both import it.
- Shared signatures use neutral types only. Terminal-cell anchors and terminal sizes leave the core.
- `mbv-theme` and `mbv-images` lose their ratatui and crossterm parts to a new TUI-only leaf crate, `mbv-tui-support`.
- **BREAKING (packaging)**: add a separate `mbv-owner` program that hosts the per-user Owner process. Both Clients start it. The `mbv` program no longer hosts the Owner, and the hidden `--__local-daemon` flag is removed. Every release artifact installs `mbv-owner` beside `mbv` and `mbv-gui`.
- The Owner launches TUI Clients for Pin swap, Unpin, and Open from the `mbv` program of its own build, also after a package upgrade replaces that file.
- The root package becomes a virtual workspace. No Client code stays in the root `src/`.
- Docs: a new ADR, one agent guide for each Client crate, a root `AGENTS.md` that describes the shared core, and updated `CONTEXT.md` and architecture docs.

## Capabilities

### New Capabilities

- `owner-program`: the separate program that hosts the per-user Owner process, how Clients find and start it, and how it launches TUI Clients of its own build.

### Modified Capabilities

- `client-interaction-core`: the Client core is its own crate with no Client toolkit, and no Client imports another Client's code.
- `gui-client`: the GUI program starts the Owner from the Owner program, ships beside it, and links no terminal UI library.
- `local-daemon-single-instance`: a restart from the mismatch prompt starts the fresh Owner from the Owner program.
- `local-daemon-stay-alive`: the local daemon runs from the Owner program.
- `pin-swap`: launches start the `mbv` program of the Owner's own build.
- `pinned-launch`: the Pin flag rules no longer name the removed `--__local-daemon` flag.

## Impact

- Code: almost all of root `src/` moves into `crates/mbv-client-core`, `crates/mbv-tui`, `crates/mbv-gui`, and `crates/mbv-owner`. The root `Cargo.toml` becomes a virtual workspace.
- Crates: new `mbv-client-core`, `mbv-tui`, `mbv-tui-support`, `mbv-owner`. Changed `mbv-gui`, `mbv-theme`, `mbv-images`, `mbv-desktop`.
- Dependencies: no new external dependency. Existing workspace dependencies move between crates at their current versions.
- Packaging: `PKGBUILD`, `PKGBUILD-git`, the `.deb` metadata, the tarball step in `.github/workflows/build.yml`, and the `Makefile` add `mbv-owner` and follow the new package names.
- Users: no change to keys, screens, or commands. `mbv`, `mbv --pin`, `mbv -q`, `mbv --toggle`, and `mbv-gui` act as before. An Owner from the old build is stopped through the existing mismatch prompt.
- Docs: `AGENTS.md`, `CONTEXT.md`, `docs/architecture/tui-frontend.md`, `docs/architecture/gui-client.md`, `docs/architecture/owner-client-operation-split.md`, `docs/recipes.md`, and a new ADR.

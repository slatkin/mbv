# Proposal: 3-add-owner-program

## Why

This is change 3 of 3. It starts after `2-split-tui-gui-clients` is merged. After that change, the Client core, the TUI, and the GUI are separate crates. But the per-user Owner process still runs as `mbv --__local-daemon`, from the TUI crate `mbv-tui`. So the Owner process links the whole terminal UI, and the GUI Client must find and start the TUI program to host its Owner: the GUI still depends on the TUI at run time. `CONTEXT.md` and the agent guides must still explain that the TUI binary is also the Owner, which is the confusion the split removes elsewhere.

## What Changes

- **BREAKING (packaging)**: add a separate `mbv-owner` program that hosts the per-user Owner process. It links no Client crate. Both Clients start it from beside their own program, else from `PATH`. The `mbv` program no longer hosts the Owner, and the hidden `--__local-daemon` flag is removed. Every release artifact installs `mbv-owner` beside `mbv` and `mbv-gui`.
- The Owner launches TUI Clients for Pin swap, Unpin, and Open from the `mbv` program of its own build. It holds the sibling `mbv` file open from start, so a launch after a package upgrade still starts the Owner's own build.
- Docs: a new ADR, and updated `CONTEXT.md`, agent guides, and the Owner and Client operation split doc.

## Capabilities

### New Capabilities

- `owner-program`: the separate program that hosts the per-user Owner process, how Clients find and start it, and how it launches TUI Clients of its own build.

### Modified Capabilities

- `gui-client`: the GUI program starts the Owner from the Owner program and ships beside it.
- `local-daemon-single-instance`: a restart from the mismatch prompt starts the fresh Owner from the Owner program.
- `local-daemon-stay-alive`: the local daemon runs from the Owner program.
- `pin-swap`: launches start the `mbv` program of the Owner's own build.
- `pinned-launch`: the Pin flag rules no longer name the removed `--__local-daemon` flag.

## Impact

- Code: `crates/mbv-tui/src/local_daemon.rs` and the Owner hooks of `crates/mbv-tui/src/{main,startup}.rs` move into a new `crates/mbv-owner`. The Owner-program lookup moves from `crates/mbv-gui` into `mbv-client-core` (`owner_link`).
- Crates: new `mbv-owner`. Changed `mbv-tui`, `mbv-client-core`, `mbv-gui`.
- Dependencies: no new external dependency.
- Packaging: `PKGBUILD`, `PKGBUILD-git`, the `.deb` metadata in `crates/mbv-tui/Cargo.toml`, the tarball step in `.github/workflows/build.yml`, and the `Makefile` add `mbv-owner`.
- Users: no change to keys, screens, or commands. An Owner from the old build is stopped through the existing mismatch prompt.
- Docs: `AGENTS.md`, `crates/mbv-tui/AGENTS.md`, `CONTEXT.md`, `docs/architecture/owner-client-operation-split.md`, and a new ADR.

# Design: add-owner-program

## Context

See proposal.md (Why). This is change 3 of 3 (`decouple-client-core`, `split-tui-gui-clients`, `add-owner-program`). The decision numbers are the numbers of the original single-change plan, kept in all three designs so that cross-references stay true. This design holds D3 (the Owner rows), D8, D9, D10 (the Owner part), and D11 (the Owner part).

The state after `split-tui-gui-clients` that shapes the approach:

- The Owner process is `mbv --__local-daemon`. `crates/mbv-tui/src/startup.rs` takes the hidden flag and calls `run_local_daemon_main` in `crates/mbv-tui/src/local_daemon.rs`, which also holds the swap launch and the tray hooks. `crates/mbv-tui/src/main.rs` installs the allocator; `startup.rs` installs the signal handlers and the panic hook, whose crash writer makes crossterm calls.
- The shared helpers the Owner needs already sit in leaf crates: `launch_exe`, `detach`, `notify`, `toggle_panel`, and the pinwin request runner in `mbv-desktop` (`launch`); the Owner lock in `mbv-daemon` (`owner_lock`); `parse_log_level_arg` in `mbv_core::applog`; the image disk cache in `mbv-config`.
- A TUI Client starts the Owner through `owner_link::start_owner` and `spawn_detached` (`mbv-client-core`), which run its own executable with `--__local-daemon` (`local_daemon_args`). The GUI resolves the sibling `mbv` program (`owner_program` in `crates/mbv-gui/src/attach.rs`) and starts it the same way.
- The Owner's Pin swap, Unpin, and Open launch `/proc/<owner pid>/exe`, which is the TUI because the Owner and the TUI are one file. That keeps the Owner's own build after a package upgrade replaces the file.
- The attach gate compares `app_version` and `wire_id` only (`BuildInfo::differs_for_gate`). `build_id` is display only.

## Goals / Non-Goals

**Goals:**

- The Owner process runs from a program that links no Client crate, no terminal UI, and no GUI toolkit.
- Both Clients find and start the Owner by one rule, in the core.
- Pin swap, Unpin, and Open keep starting a TUI of the Owner's own build, also after a package upgrade.

**Non-Goals:**

- No change to `mbvd`, the ctrl protocol, the wire schema, the Owner's persisted state, sockets, or lock path.
- No change to keys, screens, or commands.

## Decisions

### D3 (Owner rows). Placement

| Today | Goes to |
|---|---|
| `crates/mbv-tui/src/local_daemon.rs`: `run_local_daemon_main`, swap launch, tray hooks | `mbv-owner` (it calls `mbv_config::load_config` directly) |
| `crates/mbv-tui/src/main.rs`: the allocator setup | `mbv-tui` keeps it; `mbv-owner` installs the same settings |
| `crates/mbv-tui/src/startup.rs`: `install_signal_handlers`, `install_panic_hook`, `write_crash_log` | `mbv-tui` keeps them; `mbv-owner` gets its own copy of the hooks with a crash writer that makes no terminal call (the Owner holds no terminal) |
| `crates/mbv-tui/src/startup.rs`: the `--__local-daemon` branch | deleted |
| `crates/mbv-gui/src/attach.rs`: `owner_program` | `mbv-client-core` (`owner_link`), looking for `mbv-owner` |
| `mbv-client-core` `owner_link`: `local_daemon_args` | builds the `mbv-owner` arguments (`--log-level` only) |

### D8. The Owner program

`crates/mbv-owner` builds the `mbv-owner` binary. Its dependencies are `mbv-daemon`, `mbv-desktop`, `mbv-config`, `mbv-ctrl`, `mbv-core`, `mimalloc`, `nix`, and `libc`, and no Client crate. `main` installs the same allocator settings and fatal-signal handler as the `mbv` binary does, then runs today's `run_local_daemon_main`. It takes `--log-level` as today's hidden mode does, and prints its own usage. It has no other argument. Run with no Owner lock free, it exits as today's hidden mode does.

A Client starts the Owner (`owner_link::start_owner`) from the sibling `mbv-owner` of its own executable, else from `mbv-owner` on `PATH` (the rule that the GUI's `owner_program` uses today, moved to the core and used by both Clients). The `owner_exe` option and `App::set_owner_exe` stay for tests and for the GUI's restart path.

The hidden `--__local-daemon` flag is removed from `mbv`.

```
  crates/mbv-owner (bin "mbv-owner") --> mbv-daemon, mbv-desktop, mbv-config, mbv-ctrl, mbv-core
  (no dependency on mbv-client-core, mbv-tui, or mbv-gui)
```

### D9. The Owner launches TUI Clients of its own build

At start, `mbv-owner` opens the sibling `mbv` program file (`std::fs::File::open`, so `O_CLOEXEC`) and keeps the `File` for its lifetime, with the path and the `(dev, ino)` it opened. For Pin swap, Unpin, and Open:

- If the path still names the same `(dev, ino)`, launch the path.
- Else, launch `/proc/<owner pid>/fd/<n>`. This names the held inode from any process of the same user, also from the terminal emulator that Unpin starts. This is the same `/proc` access rule that `launch_exe` relies on today for `/proc/<pid>/exe`.
- If no sibling `mbv` existed at start, send the desktop notification that names the missing program and start nothing (`pin-swap` delta).

A TUI started this way keeps today's `launch_exe` behaviour for its own `--pin` relaunch.

Alternative considered: launch the sibling `mbv` path every time. Rejected: after a package upgrade it starts a newer TUI build, which the attach gate refuses.

### D10 (Owner part). Packaging and build

- `PKGBUILD`, `PKGBUILD-git`, the tarball step in `.github/workflows/build.yml`, and the `.deb` assets in `crates/mbv-tui/Cargo.toml` install `mbv-owner` beside `mbv` and `mbv-gui`.
- `Makefile` GUI targets build `-p mbv-tui -p mbv-owner -p mbv-gui`.
- The unsafe-site audit comment in `build.yml` updates the paths of the moved FFI sites.

### D11 (Owner part). Dependencies

No new external dependency, and no version change. `mimalloc` 0.1 goes to `mbv-owner` as well as `mbv-tui` (both binaries keep the allocator). `nix` 0.31 and `libc` 0.2 go with the moved FFI sites.

## Risks / Trade-offs

- [The held `mbv` file pins an old inode on disk until the Owner exits] → Same cost as today, where the Owner's own running file pins its inode.
- [`/proc/<pid>/fd/<n>` needs the same-user `/proc` access that `/proc/<pid>/exe` needs today] → No new access rule. Hardened kernels that block it today already block the current path.
- [Splitting the Owner from the TUI creates the held-file need of D9] → Accepted: the gain is an Owner with no Client code and a GUI that does not need the TUI program at run time.
- [A dev build that runs `mbv-gui` alone finds no fresh `mbv-owner`] → The `Makefile` GUI targets build all three. The mismatch prompt still catches a stale sibling.
- [A packager installs `mbv` and `mbv-gui` but forgets `mbv-owner`] → Every artifact in this repo installs all three (task 2.1); a Client that finds no `mbv-owner` reports the failure to start the Owner, as today when the program is missing.

## Migration Plan

- An Owner from an earlier release runs as `mbv --__local-daemon`. A Client from this release finds it, the gate sees a different `app_version`, and the mismatch prompt offers restart. Restart stops it and starts `mbv-owner`. No state migration: the Owner's persisted state, sockets, and lock path do not change.
- Rollback: install the earlier package. Its `mbv` finds the newer `mbv-owner` Owner and offers restart in the same way.

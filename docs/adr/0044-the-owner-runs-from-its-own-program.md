---
status: accepted
---

# The Owner Runs From Its Own Program

**Amended (2026-11-25):** the GUI Client is now the `mbv` program and the TUI
Client is `mbv-tui`. The Owner holds two sibling programs from start, each
with the held-file rule below: `mbv-tui` for Pin swap and Unpin, and `mbv`
for Open, which now starts a GUI Client window. Where this record says the
sibling `mbv` program, read the Client program that the launch starts. The
`pin-swap` and `owner-program` specs hold the current launch rules.

The per-user Owner process runs from its own program, `mbv-owner`, in the
crate `crates/mbv-owner`. The `mbv` and `mbv-gui` programs never host the
Owner process. This completes the split
[ADR 0043](0043-clients-are-separate-crates-over-one-client-core.md) began:
the Owner process links no Client crate, no terminal UI library, and no GUI
toolkit, and the GUI Client needs no TUI program at run time. ADR 0043's
rules stand.

## Decision

- **`crates/mbv-owner` builds the `mbv-owner` binary (design D8).** Its
  dependencies are `mbv-daemon`, `mbv-desktop`, `mbv-config`, `mbv-ctrl`,
  `mbv-core`, `mimalloc`, `nix`, `libc`, and `tracing`, and no Client crate.
  Its `main` installs the allocator settings and the fatal-signal handling
  the `mbv` binary installs, then runs the Owner run loop that the TUI crate
  hosted before. It takes `--log-level` and prints its own usage. It has no
  other argument.
- **A Client starts the Owner program beside itself (design D8).** A Client
  that finds no Owner process starts `mbv-owner` from the directory of its
  own program, else the `mbv-owner` name on `PATH`. The rule lives one time,
  in `mbv-client-core` `owner_link`, and both Clients use it.
- **The Owner launches TUI Clients of its own build (design D9).** At start,
  `mbv-owner` opens the sibling `mbv` program file and holds the `File`, its
  path, and its `(dev, ino)` for its lifetime. Pin swap, Unpin, and Open
  launch the path while it still names that inode, else
  `/proc/<owner pid>/fd/<n>`, which names the held inode from any process of
  the same user. When no sibling `mbv` existed at start, the Owner sends a
  desktop notification that names the missing program and starts nothing.
- **Every release artifact installs `mbv-owner` beside `mbv` and
  `mbv-gui`.** All three programs come from one build, so the sibling lookup
  and the sibling `mbv` launch find their files.

## Considered options

- **Keep the Owner in the `mbv` binary behind the hidden
  `--__local-daemon` flag (rejected).** The Owner process then links the
  whole terminal UI, and the GUI Client must find and start the TUI program
  to host its Owner. That is the confusion the Client split removes
  elsewhere.
- **Launch the sibling `mbv` path on every Owner action (rejected).** After
  a package upgrade the path names a newer TUI build, which the attach gate
  refuses. The held file pins the Owner's own build. When the Owner and the
  TUI were one file, the running program file pinned it the same way.
- **One Owner lookup path per Client (rejected).** Two lookup rules drift.
  One rule in the core serves both Clients.

## Consequences

- The dependency graph proves the boundary. `cargo tree` for `mbv-owner`
  names no Client crate, no terminal UI library, and no GUI toolkit.
- The Owner and the TUI are separate files, so the Owner needs the held
  sibling `mbv` file that the one-file design never needed.
- A package upgrade while an Owner runs breaks no launch: the held file
  keeps the Owner's own build, and the attach gate refuses a newer one.
- `mbvd`, the ctrl protocol, the wire schema, the sockets, and the lock path
  do not change. Related decisions:
  [ADR 0015](0015-local-daemon-for-stay-alive.md) names the local daemon for
  stay-alive, [ADR 0030](0030-owner-process-is-the-only-local-player-owner.md)
  keeps the Owner process the only local Player owner, and
  [ADR 0033](0033-pinwin-is-an-external-app-host.md) keeps the pinned panel
  host external.

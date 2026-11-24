---
status: accepted
---

# The Clients Are Separate Crates Over One Client Core

`mbv-client-core` holds the Client core. The TUI Client and the GUI Client are
the separate crates `mbv-tui` and `mbv-gui`, and each links the core. No Client
crate names the other, and the core names no terminal or window toolkit, so the
boundary is a compile error instead of a review convention. This amends
[ADR 0035](0035-client-core-owns-focus-tuirealm-is-the-tui-adapter.md) and
[ADR 0039](0039-gui-client-renders-with-dioxus-native-on-blitz.md): the GUI
Client links the core, not the TUI. Their rules stand.

## Decision

- **The root manifest is a virtual workspace (design D1).** It holds
  `[workspace]`, `[workspace.package]`, `[workspace.dependencies]`, and
  `[workspace.lints]`, and no package. No Client code sits in a root `src/`.
  The workspace states `resolver = "3"` explicitly; a virtual root otherwise
  falls back to resolver 1. `default-members` lists every crate except
  `mbv-gui`, including `mbv-client-core` and `mbv-tui`. Until the Owner
  program moves out, `mbv-tui` also hosts the Owner process behind the hidden
  `--__local-daemon` flag.
- **`mbv-client-core` holds the shared Client core (design D2).** It holds
  `App`, dispatch, the neutral state and events, the one Keyboard Router, the
  Client tick, the Client-side Owner find, start, and attach, and the Client
  presentation projections. It carries no `ratatui`, `crossterm`, `tuirealm`,
  `ratatui-image`, `tui-scrollbar`, `mbv-components`, `mbv-render`,
  `mbv-tui-support`, Dioxus, or Blitz. Each function, type, field, and test
  follows one placement rule: both Clients reach it, or its behaviour is
  Owner, Service, queue, or playback behaviour, so it goes to the core; only
  one Client reaches it and it is presentation or Client-local state, so it
  goes to that Client; or both Clients need it with no `App` dependency, so it
  goes to the lowest shared leaf crate that fits.
- **Each Client is its own crate.** `mbv-tui` (binary `mbv`) holds the
  TuiRealm shell, the TUI projections, the terminal key edge, the pinned
  panel, the Owner-restart prompt, and, until the Owner program moves out, the
  Owner host. `mbv-gui` (binary `mbv-gui`) holds the window, the core thread,
  `GuiCore`, and every GUI-only module. `mbv-components`, `mbv-render`, and
  `mbv-tui-support` stay TUI-only crates, and neither Client links them
  through the other.
- **The access surface lands before any crate move (design D4b).** Every
  crate move is a path-only move, so the surface each Client needs is built
  while everything is one crate, and the compiler proves it before the crates
  exist. A production read is a `pub` read method, or an existing core method
  that already does the work. A production write is first a `pub` core method
  that names the intent and holds the logic the Client runs today; a plain
  setter only where the Client alone owns the decision. No `&mut` getter hands
  out a field. Core items use `pub` or `pub(in crate::app)`, never
  `pub(crate)`. A test that asserts only core behaviour moves to the core with
  its private access and gets no seam; a test that needs the shell `Model` or
  the GUI state sets core state through a `test`-feature fixture seam, asserts
  through a production read, and acts through a production entry point or a
  named `test`-feature action seam. A private method never becomes `pub` only
  for a test.
- **The widening is transient and recorded (design D4b rule 6).** The commits
  that widen an item record the widened items and their base and end commits
  in the change's task note. After the moves, every widened item that no
  non-test target of `mbv-tui` or `mbv-gui` names is narrowed again, and every
  `test`-feature seam stays `pub`. A widening that only a test needs is not
  kept.
- **The boundary is rehearsed inside one package first (design D4c).**
  Before any new Client crate exists, the root package takes the module shape
  of the final crates: `crate::app` holds exactly the future core,
  `crate::tui` holds the shell and every TUI-staying test, and `crate::gui`
  holds `GuiCore` and the GUI modules. Because `App` fields and the
  not-yet-widened core items are `pub(in crate::app)`, the compiler then
  rejects every access that would fail across a crate boundary. A rehearsal
  move is one `git mv` plus path edits, so it cannot compile in slices: if it
  leaves more than 40 compile errors, undo it, add a conversion slice for the
  largest error class, and try again.
- **The test surface crosses the boundary by a feature, not by `cfg(test)`
  (design D4e).** A Client's test links the core compiled without `cfg(test)`,
  so a core guard that keeps a test off a real system — MPRIS, the image
  cache, the Owner lock and socket — or that adds a test-only item turns off
  for those tests. Every such guard is `cfg(any(test, feature = "test"))`, and
  a Client's test build enables the core's `test` feature. The core's fixtures,
  the fixture and seam prelude, and the `*_for_test` actions are `pub use`
  behind that gate and `#[doc(hidden)]`, so a Client names
  `mbv_client_core::tests` and the seam statics exactly as the one-crate tests
  named `crate::app::tests`.
  - **Accepted deviation: a self dev-dependency.** `mbv-gui` and
    `mbv-client-core` each take a self dev-dependency with the `test` feature
    (`mbv-gui = { path = ".", features = ["test"] }` and the same shape for
    the core). A package's own `tests/` binary and bench link its library,
    which Cargo compiles without `cfg(test)`, so the self-dependency is the
    only way a package enables its own feature for those targets. A
    library-only build carries no `feature = "test"`, so every seam stays out
    of the normal build.
- **The placements the implementation settled (row 2.1).**
  - The realized `impl App` split deleted `ExitKind`. It carried
    `Model::launch_state_snapshot()` in its `Quit` variant, so it could not
    move to the core. The shell `Model` decides `swapped_out` and saves the
    launch snapshot itself, then calls `App::teardown(quit_timeout,
    swapped_out)`; `client_exit_teardown` stays the GUI Client's shell-free
    entry.
  - The hero projection splits across the seam. The core fetches, decodes, and
    caches. The TUI owns the projection policy — the fit rule, the Wide versus
    overlay verdict, and the Resizing/Ready decision — in
    `crates/mbv-tui/src/tui/hero_image_projection.rs`, because the fit rule is
    terminal geometry and the cover-fit encoding is `ratatui-image`'s. The
    core gained the neutral cache write `App::clear_image_cover_box`, and the
    cover-fit encoding stays in the TUI's image-protocol module.
  - `focus_ring_arg` stays core-side beside `size_arg` and
    `collapsed_reserve`. All three are argv-grammar siblings of the
    pinned-setting requests, and a TUI-side copy would duplicate that grammar.
  - The terminal image protocol store stays on the shell `Model`:
    `ImageProtocols` is the TUI edge's, not `App`'s. The core pushes protocol
    facts and holds neutral queues — the completed fetches to encode and the
    keys to clear — that the shell drains each tick in `drain_protocol_work`.
    The prune splits there too: the core decides which keys to clear through
    `App::clear_image_cover_box`, and the shell executes by pruning its store
    against the cache. `mbv-tui-support::protocol` gained pixel tags so a
    staged encode never shadows newer pixels under one cache generation.
- **One key routing site for both Clients.** The Keyboard Router stays in the
  core's `app::input`. Each Client mounts it: the TUI through its key edge,
  and the GUI through the router's neutral entry, `resolve_router_outcome`.
  A GUI key the window itself consumes never reaches the router (ADR 0036).

## Context

After `1-decouple-client-core`, the shared code named no Client and held no
GUI state, but the boundary was a convention. The TUI shell and the GUI
modules sat inside `crate::app` and reached `App` internals directly, the GUI
program linked the root `mbv` package and with it `ratatui`, `crossterm`, and
TuiRealm, and every GUI edit recompiled the TUI. An agent that worked on one
Client read both. A first attempt at the split failed in one step with 94
library and 697 test errors, because it moved the crates before the access
surface existed.

The rehearsal answers that failure: build the surface while one crate, let the
compiler reject the cross-boundary accesses, and only then move paths. The
costs are an `App` access surface that mirrors more of the core than a purist
API would, and a test-sidecar feature that every Client's test build enables.
Both are bounded. The surface is narrowed after the move to the items a
Client's non-test code names, and the feature adds no code to a normal
build.

## Considered options

- **Keep the root package as the TUI and move only the core out (rejected).**
  The root `src/` would still read as "the app", which is the confusion this
  decision removes.
- **Separate repositories (rejected).** One repository, by the user's
  decision; the shared core and the leaf crates want one workspace and one
  lockfile.
- **Rely on review and `rg` checks without a rehearsal (rejected).** That is
  the first attempt. An `rg` pattern passes while a binding name or a path
  form it does not know still reaches a private item; the compiler at the
  rehearsal is the gate.
- **Give each Client its own copy of the shared rules (rejected).** The rules
  would drift, and a fix would land twice. The core holds each interaction
  rule once (ADR 0042).
- **A `cfg(test)`-only seam surface (rejected).** It turns off for a Client
  test, and the build stays green while a real system call runs (design D4e).
- **Widen `App` fields to `pub` (rejected).** A field is never `pub`, also not
  under the `test` feature; a Client reads and writes through methods.

## Consequences

- Cargo holds the boundary. A core call to a toolkit, a Client call into the
  other Client, or a core name of a Client is a missing dependency or a
  compile error, never a review finding.
- A third Client links `mbv-client-core`, builds its own `ClientCapabilities`,
  passes `None` where it has no library stores, and implements
  `drain_client_events` as a no-op. It changes no core code.
- A GUI edit recompiles no TUI target, and a TUI edit recompiles no GUI
  target. The core compiles once for both.
- The lints split with the manifests. `mbv-tui` carries the terminal-write
  restriction for its binary, and the core inherits the workspace's library
  side.
- The test layout follows the crates: public-API tests in each crate's
  `tests/<name>.rs` binary, private-item tests in `src/`. The
  self-dependency is the accepted cost of reaching a `test`-feature seam from
  a crate's own test target, and the `test` feature appears only under
  dev-dependencies and those self-dependencies.
- Documentation splits with the code. The root `AGENTS.md` holds the
  workspace map and the shared core, and each Client crate has its own guide.
- The installed programs, the Owner, its sockets, its lock path, and the
  prefs file do not change. Related decisions:
  [ADR 0035](0035-client-core-owns-focus-tuirealm-is-the-tui-adapter.md) keeps
  the core's focus authority,
  [ADR 0039](0039-gui-client-renders-with-dioxus-native-on-blitz.md) keeps the
  GUI's renderer in the same process as its core thread, and
  [ADR 0042](0042-the-client-core-names-no-client.md) keeps the core free of
  Client names.

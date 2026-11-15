# mbv Agent Guide

mbv is a Rust terminal media client for Emby, Audiobookshelf, and Feeds. It embeds mpv. Every local TUI is a Client of the per-user Owner process. That process is the only local Player-owner host. Stay-alive controls whether that process outlives its Clients. Packaged `mbvd` stays a separate Player owner. `mbvd` is a headless server daemon (a systemd unit, no desktop session). It never has a tray, a pinned panel, or any Client functionality. Connection to it still leaves a Local process on the user machine, and that process owns those (`CONTEXT.md`: *mbvd*, *Local process*).

## Quick Overview

- **Language**: Rust (stock edition-2024 `cargo fmt`, max-width-100)
- **Services**: Emby, Audiobookshelf, Feeds
- **Playback**: embedded mpv; Google Cast receivers and Emby Sessions are output projections of the Owner
- **Clients**: the TUI (TuiRealm) and the GUI Client (Dioxus Native over Blitz, ADR 0039)
- **Player owners**: the per-user Owner process (local) and the packaged `mbvd` daemon
- **Async**: synchronous first; `tokio` is edge-only

## Workspace Structure

```
src/
├── main.rs                  # Thin binary entry; calls startup
├── startup.rs               # The one entry main calls: args, config, pinned panel launch, Owner link, TUI Client run
├── lib.rs                   # Library target: shell App, Client core tick, keyboard router, Client-side Owner find/start/attach
├── app.rs + app/            # App module: Client core, dispatch, GUI client state, projections, playback presentation
│   ├── shell/               # Interactive shell + TuiRealm Model: App, mount/focus, runtime lifecycle,
│   │                        #   projections, dispatch, effects; single tick + draw path (run.rs, draw.rs)
│   └── input/               # The ONLY keyboard routing site (see Notes for Agents)
├── config.rs                # Re-exports mbv-config; adds app-side config helpers
├── owner_link.rs (+owner_link/)       # Client-side Owner find, start, attach (lib.rs re-exports it)
├── owner_restart.rs (+owner_restart/) # Owner build mismatch at startup
├── single_instance.rs       # Single instance: flock + socket (ADR 0006)
├── launch.rs (+launch/)     # The binary path a launcher starts
├── pin.rs (+pin/)           # Only pinwin caller: builds every pinwin argv and runs the program
└── local_daemon.rs          # Local-daemon bootstrap

crates/
├── mbv-surfaces/        # Surface state (cursor, selection, marks, viewport), core FocusStack, HitMap,
│                        #   per-list capacity. No ratatui, TuiRealm, crossterm, or GUI toolkit
├── mbv-components/      # TuiRealm Interactive Components
├── mbv-render/          # screens/ prepare content, arrangements/ place it, components/ paint it, layout.rs composes
├── mbv-ui-model/        # Plain presentation models shared by UI crates
├── mbv-ui-msg/          # Typed messages crossing the interactive-component boundary
├── mbv-gui/             # GUI Client, Dioxus Native binary (mbv-gui); outside default-members
├── mbv-audiobookshelf/  # Audiobookshelf provider client
├── mbv-emby/            # Emby provider client
├── mbv-emby-model/      # Emby DTOs and tick units
├── mbv-feed/            # Feed parsing and entry state
├── mbv-cast/            # Google Cast client, discovery, per-provider dispatch
├── mbv-core/            # App logging; Service state and setup-generation types
├── mbv-remote-player/   # Remote player runtime
├── mbv-player/          # mpv-backed player
├── mbv-daemon/          # Daemon library used by mbvd and the TUI in-process
├── mbvd/                # Packaged daemon, persistence, sockets
├── mbv-queue/           # Canonical queue, kinds, state, lineage
├── mbv-ctrl/            # Player-owner protocol vocabulary
├── mbv-config/          # App configuration
├── mbv-ids/             # Type-safe media ID newtypes (ItemId, MediaSourceId, EmbySessionId)
├── mbv-keybinds/        # Configurable keybinding registry, chord grammar, validation
├── mbv-net/             # Shared HTTP, TLS-agent, socket, and retry primitives
├── mbv-ws/              # Emby websocket client transport
├── mbv-visualizer/      # PipeWire stereo audio capture worker for the visualizer
├── mbv-desktop/         # MPRIS D-Bus server and system tray (owns zbus/tokio/ksni)
├── mbv-text/            # Fuzzy-match acceptance and control-character predicates for text input
├── mbv-theme/           # Semantic theme roles and palette values
└── mbv-images/          # Image cache, loading and processing, terminal image protocol support
```

`src/lib.rs` is the root package's library target. The `mbv` binary is a thin `main` over `run`. The rest of `src/` is the shared `mbv` library and the thin `mbv` binary over it. `lib.rs` exposes the Client-side Owner find, start, and attach steps that every Client shares.

`crates/mbv-components/`: a component over a shared surface (Queue, Queue playback, Status bar) is an adapter over its Surface state. It forwards neutral events, paints from that state, and reports geometry. Other components still own their local interaction state. Both kinds send typed `Msg`s (`Msg`s live in `mbv-ui-msg`).

`crates/mbv-gui/` renders with Blitz and links the `mbv` library (ADR 0039). Its synchronous core thread owns the `App` and the Client core tick (`src/core_thread.rs`, `src/attach.rs`). `src/window/` is the Dioxus component tree, in the same process, with one module for each former front-end file. `src/bridge/` holds the DOM-input translation. `assets/` holds the embedded CSS and fonts.

`pinwin` is an external Rust program (Wayland layer-shell panel, separate repo `slatkin/pinwin`, minimum version 0.4.1). It docks the mbv pinned panel beside tiled windows. mbv links no pinwin library. Fix pinwin bugs upstream, never here.

## Architecture Documentation

| When working on...                                         | Read...                                                                                                                  |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Naming any domain concept                                  | `CONTEXT.md`                                                                                                             |
| Accepted decisions (the why)                               | `docs/adr/` (read `docs/adr/README.md` before you write or amend an ADR)                                                 |
| Current behavior                                           | `openspec/specs/`                                                                                                        |
| In-progress work                                           | The full `openspec/changes/<name>/` directory                                                                            |
| Queue, progress, or playback-lifecycle state               | `docs/invariants/`                                                                                                       |
| A ctrl protocol variant, a keybind action, a Service capability | `docs/recipes.md`                                                                                                   |
| Any TUI change                                             | `docs/architecture/tui-frontend.md`                                                                                      |
| Interactive Components                                     | ADRs 0022–0024, `openspec/specs/interactive-component-framework/spec.md`, `docs/architecture/interactive-surface-ledger.md` |
| TUI component map                                          | `docs/architecture/interactive-tui-component-map.md`                                                                     |
| mpv-owned playback prompts                                 | `docs/architecture/mpv-owned-playback-prompts.md`                                                                        |
| Owner and Client operation split                           | `docs/architecture/owner-client-operation-split.md`                                                                      |
| Performance                                                | `docs/architecture/performance.md`                                                                                       |
| The GUI Client                                             | `docs/architecture/gui-client.md`                                                                                        |
| Coding standards                                           | `docs/standards/README.md`                                                                                               |
| Dated plans / cross-session handoffs                       | `docs/plans/` / `openspec/handoffs/`                                                                                     |

For docs and concept discovery (ADRs, openspec, `CONTEXT.md`), run `qmd query "..."` (collection `mbv`). Use `rg` only for exact strings.

## Key Concepts

- **Shell `Model`**: Owns terminal and worker lifecycle, Services, Player and queue authority, persistence, protocols, external effects, and the TuiRealm `Application`. It projects owned presentation models. It is not a second store of component-local UI state.
- **`App`**: Stays the shell domain and effect state and base-frame geometry composer. `Model::draw_frame` composes that frame one time. Then mounted components paint their owned surfaces.
- **`AppComponent`**: A mounted component (`crates/mbv-components/`). It owns cursor, scroll, local focus and selection, filters, drafts, viewport, event interpretation, `view()`, and hit geometry. It mutates local state directly. It sends typed `Msg` only for work outside its authority.
- **Projection**: One-way. `sync_*` and `push_*` carry shell-owned content, not cursor, scroll, or selection mirrors.
- **Composed / Bound queue**: One canonical ordered sequence of `QueueItem` slots. Use stable `QueueSlotId` for occurrences.
- **Player owner**: Authoritative for Bound queue state, the active slot, and the playback lifecycle.
- **Output projection**: mpv, a Cast receiver, and an Emby Session. Each is an output of the Owner, never queue authority.
- **Owner admission**: The capability boundary. Media kind, required Service setup, and negotiated ctrl transport decide what enters a Bound queue.

## Common Patterns

**Interactive data flow**:

```text
App/runtime -> shell sync/push -> Interactive Component -> Render Component
App/runtime <- shell handles typed Msg with resolved target <- component update
```

**Commands**:

```sh
cargo check -p <package>                              # check
cargo nextest run -p <package>                        # test locally (prefer nextest)
cargo nextest run --release --test-threads=4          # CI test run (fd-budget throttling, see build.yml comment)
cargo llvm-cov                                        # measure coverage
cargo clippy --workspace --all-targets -- -D warnings # lint
cargo fmt                                             # run for each Rust change
cargo fmt --all -- --check                            # read-only check
make check-code-file-lines                            # just before you push
```

## Notes for Agents

### Workflow

1. Read `CONTEXT.md` before you name domain concepts. Each term's *Avoid* line names the words you must not use. Add new terms with the change. Ask before you rename or collide.
2. For in-progress work, read the full `openspec/changes/<name>/` directory. Its delta specs overlay main specs until archived under `openspec/changes/archive/`. Do not edit archived changes.
3. `docs/invariants/` holds properties that the code must maintain but that no type enforces. It tells why they matter, how the code upholds them today, and where they still fail. Read it before you touch queue, progress, or playback-lifecycle state. Add one when you find or fix this kind of bug. Do not leave the lesson only in a commit message.
4. `docs/recipes.md` holds ordered site lists for recurring multi-site changes. Follow the recipe when your change is one of these kinds. Update the recipe when a site moves.
5. Write durable plans in OpenSpec markdown, not in chat. Commit plans, specs, and docs with code. Sync applied deltas into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge. Never sync or archive when you open a PR: the change stays open while review and live tests can still change it.
6. Do not write openspec changes without being prompted. Treat bug reports as issues to be fixed immediately.
7. Probe the real service before you plan API work. If the design of a feature depends on what a remote API returns, probe that endpoint first. Emby, Audiobookshelf, feeds, and any remote endpoint are examples.
8. Change source-of-truth types before callers. Ask only about material design and product choices.
9. Never check out another branch in the main workspace (`~/Dev/mbv`), EVER. It stays on `main`. Work in the main workspace by default. Create a worktree only when the user authorizes it for that change (then use the `git-worktrees` skill, `wt <change-name>`). A bug fix or a task is not authorization.
10. Commit your changes or undo them. Never leave a dirty worktree.
11. Keep no file over 800 lines at push time. Split along responsibility seams. Run `make check-code-file-lines` just before you push. Never use it as a per-task, CI, or acceptance gate.
12. A pre-existing gate failure is still a failing gate. Before you push, every red gate is yours to fix or to put in front of the user for a decision, whoever introduced it and however unrelated the line looks. "Not from this work" never green-lights a push; it names the next fix, not a deferral. This is not a blame game — it is how the standards stay real.

### Interactive architecture

1. Components and painters cannot name `App`. Their crates sit below the TUI crate, so a break of this boundary is a compile error.
2. Components never receive `App`, Service clients, credentials, `Config`, `PlayerProxy`, protocol objects, integration locks, or channels. Msgs carry semantic intent and stable opaque identities. They never carry raw events or coordinates for shell re-resolution.
3. Local movement that drives persistence or effects sends the resolved value from the component. The shell never recomputes it. Re-anchor only on discrete navigation or responsive transition.
4. Destination components stay mounted while their Service library is in the catalog. Local state then survives tab and breakpoint changes. Mounted, focused, active, and painted stay distinct. Overlays mount and unmount through the TuiRealm focus stack.
5. Give every boundary-crossing request variant an exhaustive dispatch arm or a documented no-op. Never hide a variant with a wildcard.

### Playback and queue authority

1. Do not recreate parallel Emby, Feed, or Audiobookshelf lists. Do not use content identity to address a slot.
2. mpv is a source and output projection (and can load only the active file). It is never queue authority.
3. A Cast receiver, like mpv, holds only the active item. The Owner keeps the canonical queue and the active slot.
4. An Emby Session, like mpv and a Cast receiver: the Owner drives the device through the Emby Sessions API, keeps the canonical queue and the active slot, and the Session holds the active item and the next item, never more. The TUI is a picker and has no attached-session play path.
5. Components can edit Composed content. They never perform admission and never mutate the canonical queue directly. Shell and Player paths do that work.

### TUI work

1. Before any TUI change, read `docs/architecture/tui-frontend.md`. It covers the two UI trees, Panel and slot composition, the reuse workflow, the test-layer matrix, and the completion checklist.
2. These tripwires apply even when the work does not look like TUI work. Use one owner and one painter per surface per breakpoint. Route keyboard precedence only in `src/app/input/`.
3. `src/app/input/` is the only keyboard routing site (`router.rs` precedence, `key_policy.rs` order, `resolver.rs` chords) for the TUI and for the surfaces that both Clients share. Per-surface key modules sit beside them (`browse_dispatch.rs`, `confirm_keys`, `gui`, `lib_keys.rs`, `playlist_keys.rs`, `queue_keys.rs`, `search_sidebar_keys.rs`); precedence, order, and chords still resolve only in the three named files. Never add another routing site. The GUI Library panel takes its own keys in the GUI window (`crates/mbv-gui/src/window/`), before they reach this router (ADR 0036).

### Code

1. Coding standards live in `docs/standards/README.md` (Microsoft Pragmatic Rust Guidelines, pinned locally, one file per rule). Scan its checklist. Then run `qmd query` for the full text of a rule. Never read every rule.
2. Write no bespoke scripting. Write no shell, awk, or python checkers, including their self-tests, CI jobs, and Makefile targets. Never use one as proof. Use unit tests instead, unless the user explicitly asks for a script for that request.
3. Use no lint suppression without per-instance user approval. Use no `allow` or `expect` attribute in any form. Do not loosen `[lints]`. Never edit `clippy.toml` (thresholds included) unless the user explicitly asks. Fix the cause instead (params struct, delete dead code and its tests, drop the unused import).
4. Run `cargo fmt` for each Rust change (stock edition-2024, max-width-100). Accept all reflow and never revert it.
5. Write custom domain error types (for example, `AudiobookshelfError`). Do not introduce `anyhow`, `thiserror`, or `eyre`.
6. Lay out modules as one file per module through `mod`. Never splice a module across files with `include!` or `#[path]`. Give a module with children as `foo.rs` plus a `foo/` directory to hold them (tests as `foo/tests.rs`, or `foo/tests.rs` plus `foo/tests/` when split). Never use `mod.rs`.
7. Prefer synchronous code first. `tokio` is edge-only (`crates/mbv-desktop`, `crates/mbv-gui` — `dioxus-native`'s asset resolver only, `zbus`). Do not spread it.
8. For sharing, prefer owned data and `Msg` identities over new `Arc` or `Rc`.

### Tests

1. Prevent test inflation. Do not cover every execution path for every addition. Small additions need few tests, if any.
2. A test owns a contract or it does not exist. Before you add a test, name the contract and the layer that owns it (`docs/architecture/tui-frontend.md`, Tests matrix). If another test already covers the claim, extend that test or add nothing. Keep in `#[case]` tables only cases with different expected outcomes. Cite in a regression test the issue or commit it guards, in its name or a comment. A plan item that only says "add tests" is not sufficient. Name the contract. Test count and coverage are never goals (`docs/invariants/14-test-ownership.md`).
3. Public-API tests of a library crate go in that crate's `tests/<name>.rs` binary (one binary per crate, `<name>/` submodules); `src/` tests are for private items. Never widen visibility to keep a test in `src/` (`docs/standards/rules/M-INTEGRATION-TESTS.md`).
4. Unit tests are hermetic mocks with no live or smoke tests. Use no live mpv handle (`init_mpv`/`test_mpv`), no real configuration or state directories, and no live servers. Mock the boundary. Anything that needs the real external system needs a manual run.
5. Live-test debugging needs the approval of the user. The user does live testing. Before you run anything live on the user machine, ask first.
6. Use no real sleeps in tests (`thread::sleep`, timeouts, retry backoff). Inject the outcome, not the delay (mock returning the terminal state, zeroed timeout, seam observing attempts). If a test cannot run fast without a change to production timing, it asserts the wrong thing. Delete it.
7. Keep tests simple. Use one behavior per test. Use no loops or branching in the body. Put setup in helpers. The Clippy complexity threshold (25) applies to the whole crate, so review holds tests to the stricter bar.
8. Fixture-varying test families use named `#[case]` tables (through the `rstest` dev-dependency). `#[case]` is never a way to generate many thin tests. Convert opportunistically and file by file.

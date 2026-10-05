# mbv

mbv is a Rust terminal media client for Emby, Audiobookshelf, and Feeds. It embeds mpv. Every local TUI is a Client of the per-user Owner process. That process is the only local Player-owner host. Stay-alive controls whether that process outlives its Clients. Packaged `mbvd` stays a separate Player owner. `mbvd` is a headless server daemon (a systemd unit, no desktop session). It never has a tray, a pinned panel, or any Client functionality. Connection to it still leaves a Local process on the user machine, and that process owns those (`CONTEXT.md`: *mbvd*, *Local process*). 

## Start here

* Read `CONTEXT.md` before you name domain concepts. Its *Avoid* terms name the words you must not use. Add new terms with the change. Ask before you rename or collide.
* Architecture work follows fixed sources. `docs/adr/` holds accepted decisions (the why). `openspec/specs/` holds current behavior. For in-progress work, read the full `openspec/changes/<name>/` directory. Its delta specs overlay main specs until archived. Do not edit archived changes.
* `docs/invariants/` holds properties that the code must maintain but that no type enforces. It tells why they matter, how the code upholds them today, and where they still fail. Read it before you touch queue, progress, or playback-lifecycle state. Add one when you find or fix this kind of bug. Do not leave the lesson only in a commit message.
* Write durable plans in OpenSpec markdown, not in chat. Commit plans, specs, and docs with code. Sync applied deltas into `openspec/specs/`. Archive when done.
* Probe the real service before you plan API work. If the design of a feature depends on what a remote API returns, probe that endpoint first. Emby, Audiobookshelf, feeds, and any remote endpoint are examples.
* Change source-of-truth types before callers. Ask only about material design and product choices.
* Commit your changes or undo them. Never leave a dirty worktree.
* Keep no file over 800 lines at push time. Split along responsibility seams. Run `make check-code-file-lines` just before you push. Never use it as a per-task, CI, or acceptance gate.
* Coding standards live in `docs/standards/README.md` (Microsoft Pragmatic Rust Guidelines, pinned locally, one file per rule). Scan its checklist. Then run `qmd query` for the full text of a rule. Never read every rule.
* Prevent test inflation. Do not cover every execution path for every addition. Small additions need few tests, if any.

## Repository map

* `src/app/shell/`: interactive shell and TuiRealm `Model`. It holds `App`, mount and focus, runtime lifecycle, projections, dispatch, effects, and the single tick and draw path (`shell/run.rs`, `shell/draw.rs`).
* `crates/mbv-components/`: TuiRealm Interactive Components. It holds local interaction state, embedded list controls, and pointer primitives (`Msg`s live in `mbv-ui-msg`).
* `crates/mbv-render/`: `screens/` prepare content, `arrangements/` place it, `components/` paint it, and `layout.rs` composes layouts.
* `src/app/input/`: the only keyboard routing site (`router.rs` precedence, `key_policy.rs` order, `resolver.rs` chords). Never add another.
* `src/local_daemon.rs`: Local-daemon bootstrap. The rest of `src/` is the TUI binary.
* `crates/mbv-audiobookshelf/`: Audiobookshelf provider client.
* `crates/mbv-cast/`: Google Cast client, discovery, per-provider dispatch.
* `crates/mbv-emby/`: Emby provider client.
* `crates/mbv-core/`: app logging and Service state and setup-generation types.
* `crates/mbv-remote-player/`: Remote player runtime.
* `crates/mbv-player/`: mpv-backed player.
* `crates/mbv-daemon/`: Daemon library used by `mbvd` and the TUI in-process.
* `crates/mbv-emby-model/`: Emby DTOs and tick units.
* `crates/mbv-queue/`: canonical queue, kinds, state, and lineage.
* `crates/mbv-ctrl/`: Player-owner protocol vocabulary.
* `crates/mbv-config/`: app configuration.
* `crates/mbv-feed/`: feed parsing and entry state.
* `crates/mbvd/`: packaged daemon, persistence, sockets.
* `crates/mbv-ids/`: type-safe media identifier newtypes (`ItemId`, `MediaSourceId`, `EmbySessionId`).
* `crates/mbv-keybinds/`: configurable keybinding registry, chord grammar, validation.
* `crates/mbv-net/`: shared HTTP, TLS-agent, socket and retry primitives.
* `crates/mbv-ws/`: Emby websocket client transport.
* `crates/mbv-visualizer/`: PipeWire stereo audio capture worker for the visualizer.
* `crates/mbv-desktop/`: MPRIS D-Bus server and system tray (owns zbus/tokio/ksni).
* `crates/mbv-text/`: fuzzy-match acceptance and control-character predicates for text input.
* `crates/mbv-theme/`: semantic theme roles and palette values.
* `crates/mbv-images/`: image cache, loading and processing, terminal image protocol support.
* `crates/mbv-ui-model/`: plain presentation models shared by UI crates.
* `crates/mbv-ui-msg/`: typed messages crossing the interactive-component boundary.
* `pinwin`: the Rust pinwin crate (GTK4 layer-shell panel, separate repo `slatkin/pinwin`). It docks the mbv pinned panel beside tiled windows. `src/pin.rs` is the only boundary. It is a git-rev dependency pinned in `Cargo.toml` (`pinwin = { git = "https://github.com/slatkin/pinwin", rev = "<sha>" }`). Update the pin by changing `rev` there. Fix pinwin bugs upstream, never here.

## Interactive architecture

```text
App/runtime -> shell sync/push -> Interactive Component -> Render Component
App/runtime <- shell handles typed Msg with resolved target <- component update
```

* Shell `Model` owns terminal and worker lifecycle, Services, Player and queue authority, persistence, protocols, external effects, and the TuiRealm `Application`. It projects owned presentation models. It is not a second store of component-local UI state. `App` stays the shell domain and effect state and base-frame geometry composer. `Model::draw_frame` composes that frame one time. Then mounted components paint their owned surfaces.
* Components and painters cannot name `App`. Their crates sit below the TUI crate, so a break of this boundary is a compile error.
* A mounted `AppComponent` (`crates/mbv-components/`) owns cursor, scroll, local focus and selection, filters, drafts, viewport, event interpretation, `view()`, and hit geometry. It mutates local state directly. It sends typed `Msg` only for work outside its authority.
* Components never receive `App`, Service clients, credentials, `Config`, `PlayerProxy`, protocol objects, integration locks, or channels. Msgs carry semantic intent and stable opaque identities. They never carry raw events or coordinates for shell re-resolution.
* Projection is one-way. `sync_*` and `push_*` carry shell-owned content, not cursor, scroll, or selection mirrors. Local movement that drives persistence or effects sends the resolved value from the component. The shell never recomputes it. Re-anchor only on discrete navigation or responsive transition.
* Destination components stay mounted while their Service library is in the catalog. Local state then survives tab and breakpoint changes. Mounted, focused, active, and painted stay distinct. Overlays mount and unmount through the TuiRealm focus stack.
* Give every boundary-crossing request variant an exhaustive dispatch arm or a documented no-op. Never hide a variant with a wildcard.

ADRs 0022 through 0024, `openspec/specs/interactive-component-framework/spec.md`, and `docs/architecture/interactive-surface-ledger.md` describe this architecture.

## Playback and queue authority

* A Composed or Bound queue has one canonical ordered sequence of `QueueItem` slots. Use stable `QueueSlotId` for occurrences. Do not recreate parallel Emby, Feed, or Audiobookshelf lists. Do not use content identity to address a slot.
* The Player owner is authoritative for Bound queue state, the active slot, and the playback lifecycle. mpv is a source and output projection (and can load only the active file). It is never queue authority.
* Owner admission is the capability boundary. Media kind, required Service setup, and negotiated ctrl transport decide what enters a Bound queue. Components can edit Composed content. They never perform admission and never mutate the canonical queue directly. Shell and Player paths do that work.

## TUI work

Before any TUI change, read `docs/architecture/tui-frontend.md`. It covers the two UI trees, Panel and slot composition, the reuse workflow, the test-layer matrix, and the completion checklist. These tripwires apply even when the work does not look like TUI work. Use one owner and one painter per surface per breakpoint. Route keyboard precedence only in `src/app/input/`.

## Tooling

* Write no bespoke scripting. Write no shell, awk, or python checkers, including their self-tests, CI jobs, and Makefile targets. Never use one as proof. Use unit tests instead, unless the user explicitly asks for a script for that request.
* check: `cargo check -p <package>`
* test: `cargo nextest run -p <package>` locally (prefer nextest). CI runs `cargo nextest run --release --test-threads=4` (fd-budget throttling, see `build.yml` comment). Use `cargo llvm-cov` to measure coverage.
* A test owns a contract or it does not exist. Before you add a test, name the contract and the layer that owns it (`docs/architecture/tui-frontend.md`, Tests matrix). If another test already covers the claim, extend that test or add nothing. Keep in `#[case]` tables only cases with different expected outcomes. Cite in a regression test the issue or commit it guards, in its name or a comment. A plan item that only says "add tests" is not sufficient. Name the contract. Test count and coverage are never goals (`docs/invariants/14-test-ownership.md`).
* Unit tests are hermetic mocks with no live or smoke tests. Use no live mpv handle (`init_mpv`/`test_mpv`), no real configuration or state directories, and no live servers. Mock the boundary. Anything that needs the real external system needs a manual run.
* Live-test debugging needs the approval of the user. The user does live testing. Before you run anything live on the user machine, ask first. State the exact command and why. mbv, pinwin, a demo, compositor commands (`niri msg`), and any self-driving diagnostic are examples. Never proceed without a yes. "Find the issue" is not that approval.
* Use no real sleeps in tests (`thread::sleep`, timeouts, retry backoff). Inject the outcome, not the delay (mock returning the terminal state, zeroed timeout, seam observing attempts). If a test cannot run fast without a change to production timing, it asserts the wrong thing. Delete it.
* Keep tests simple. Use one behavior per test. Use no loops or branching in the body. Put setup in helpers. The Clippy complexity threshold (25) applies to the whole crate, so review holds tests to the stricter bar.
* Fixture-varying test families use named `#[case]` tables (through the `rstest` dev-dependency). `#[case]` is never a way to generate many thin tests. Convert opportunistically and file by file.
* lint: `cargo clippy --workspace --all-targets -- -D warnings`
* Use no lint suppression without per-instance user approval. Use no `allow` or `expect` attribute in any form. Do not loosen `[lints]`. Never edit `clippy.toml` (thresholds included) unless the user explicitly asks. Fix the cause instead (params struct, delete dead code and its tests, drop the unused import).
* Run `cargo fmt` for each Rust change (stock edition-2024, max-width-100). Accept all reflow and never revert it. `cargo fmt --all -- --check` is a read-only check.
* Write custom domain error types (for example, `AudiobookshelfError`). Do not introduce `anyhow`, `thiserror`, or `eyre`.
* Lay out modules as one file per module through `mod`. Never splice a module across files with `include!` or `#[path]`. Give a module with children as `foo.rs` plus a `foo/` directory to hold them (tests as `foo/tests.rs`, or `foo/tests.rs` plus `foo/tests/` when split). Never use `mod.rs`.
* Prefer synchronous code first. `tokio` is edge-only (`crates/mbv-desktop`, `zbus`). Do not spread it.
* For sharing, prefer owned data and `Msg` identities over new `Arc` or `Rc`.
* For anything web related, use `ketch`, not curl.
* For docs and concept discovery (ADRs, openspec, CONTEXT.md), run `qmd query "..."` (collection `mbv`). Use `rg` only for exact strings.

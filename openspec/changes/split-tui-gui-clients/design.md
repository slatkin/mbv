# Design: split-tui-gui-clients

## Context

See proposal.md (Why). This is change 2 of 3 (`decouple-client-core`, `split-tui-gui-clients`, `add-owner-program`). The decision numbers are the numbers of the original single-change plan, kept in all three designs so that cross-references stay true. This design holds D1, D2 (restated), D3 (the crate placement), D4 (restated), D4b, D4c, D10 (part), D11, and D12. `decouple-client-core` holds D4a, D5, D6, D6a, D6b, D7, and D13. `add-owner-program` holds D8, D9, and the Owner parts of D3 and D10.

The state after `decouple-client-core` that shapes the approach:

- The root package `mbv` is a library and the `mbv` binary. Its `src/app/` holds the shared `App`, dispatch, state, infra, and the keyboard router, plus the TUI shell (`src/app/shell/`) and the GUI-only modules `gui_library`, `gui_settings`, `gui_playlists`, `gui_client_state`, and `input/gui*`. `src/gui.rs` holds `GuiCore { app, gui }`.
- Shared code outside the shell and the GUI modules names no ratatui, crossterm, tuirealm, `mbv-render`, or `mbv-components` in non-test files. `App` holds no GUI state. The core names no Client (`ClientCapabilities`).
- `crates/mbv-gui` depends on the root `mbv` package. So it links ratatui, crossterm, and TuiRealm, and every GUI edit recompiles the root package.
- `App` fields and most core methods are `pub(in crate::app)`. The shell and the GUI modules sit inside `crate::app`, so they reach both directly (D4b measures it).
- The Owner process is `mbv --__local-daemon` (`src/startup.rs`, `src/local_daemon.rs`). It stays so in this change; `add-owner-program` moves it.
- A first attempt at the split failed at one row (94 library and 697 test errors in one step), because the shell, its tests, and the GUI modules reached `App` internals directly. D4b and D4c answer that failure.

## Goals / Non-Goals

**Goals:**

- Cargo enforces the boundary: `mbv-client-core` cannot name a terminal or GUI toolkit, and neither Client crate can name the other. A break is a compile error, not a review finding.
- Every crate move is a path-only move, because the compiler has proved the access surface before it (D4c).
- Each Client keeps its present behaviour. The TUI and the GUI attach to one Owner at the same time.
- An agent that works in one Client crate reads only that crate, the core, and the shared leaf crates.

**Non-Goals:**

- No Owner program (`add-owner-program`). The `mbv` program keeps the hidden `--__local-daemon` Owner host, and the GUI keeps starting the sibling `mbv` to host its Owner.
- No new user feature, key, screen, or command. No change to the ctrl protocol or the wire schema. No change to `mbvd`.
- No redesign of `App`. The core API that the Clients use mirrors today's `App` methods; this change makes it explicit and narrow, not deep.
- No merge of the TUI Library and the GUI Library panel.

## Decisions

### D1. Crate layout

```
  crates/mbv-tui  (bin "mbv")        crates/mbv-gui  (bin "mbv-gui")
        |   \                              /   |
        |    mbv-components, mbv-render,  /    |
        |    mbv-tui-support (TUI-only)  /     |
        v                               v      |
  crates/mbv-client-core  <-------------+      |
        |                                      |
        v                                      v
  shared leaf crates: mbv-surfaces, mbv-ui-model, mbv-ui-msg, mbv-keybinds,
  mbv-text, mbv-theme, mbv-images, mbv-desktop, and the domain and
  provider crates (queue, ctrl, config, emby, audiobookshelf, feed, cast,
  remote-player, player, daemon, visualizer, net, ws, ids, core)
```

The root `Cargo.toml` becomes a virtual workspace (`[workspace]`, `[workspace.package]`, `[workspace.dependencies]`, `[workspace.lints]` only). `default-members` lists every crate except `mbv-gui`, as today, and adds `mbv-client-core` and `mbv-tui`. Until `add-owner-program`, `mbv-tui` also hosts the Owner process (`--__local-daemon`).

Alternatives considered:

- Keep the root package as the TUI and only move the core out. Rejected: the root `src/` would still look like "the app", which is the confusion this change removes.
- Separate repositories. Rejected by the user: one repo.

### D2. Placement rule

For each function, type, field, and test:

1. Both Clients reach it (directly, or through the shared router and `Command` dispatch) → `mbv-client-core`.
2. Only one Client reaches it, and it is presentation or Client-local state → that Client's crate.
3. Only one Client reaches it, but it is Owner, Service, queue, or playback behaviour → `mbv-client-core`, as a public call.
4. A type both Clients need but that has no `App` dependency → the lowest shared leaf crate that fits (`mbv-ui-model`, `mbv-ui-msg`, `mbv-surfaces`).

### D3. Root `src/` placement

| Today | Goes to |
|---|---|
| `src/main.rs`, `src/startup.rs` (args, `-q`, `--toggle`, `-d`, `--pin`, TUI run, the `--__local-daemon` branch) | `mbv-tui` |
| `src/app/shell/`, `src/app/shell.rs` | `mbv-tui`, except the `impl App` blocks below (Rust allows no inherent `impl App` outside the core) |
| `src/app/shell/playback.rs`: `impl App` (`handle_playback_request`, `slotless_playback_title_parts`); `src/app/shell/ui_state_reset.rs`: `impl App` (`reset_browse_root_defaults`, `reset_library_browse_root`) | `mbv-client-core` (`impl App`); the `impl Model` halves stay in `mbv-tui` |
| `src/app/input/` (router, key policy, resolver, per-surface key modules) | `mbv-client-core` |
| `src/app/input/gui.rs`, `src/app/input/gui/`, `src/app/gui_library`, `gui_settings`, `gui_playlists`, `gui_client_state`, `src/gui.rs` (`GuiCore`) | `mbv-gui` |
| `src/app/{dispatch,state,infra}/` | `mbv-client-core` |
| `src/app/{client_core,client_drains,waking_relay,app_stub}.rs`, the toolkit-free seams of `src/app/test_seams.rs` | `mbv-client-core` (seams under the `test` feature) |
| `src/owner_link.rs` + `owner_link/`, `client_hello_claims` (today `src/client_kind.rs`), `src/config.rs` | `mbv-client-core` |
| `src/config.rs`: the image disk cache family (`image_disk_cache_dir`, `image_disk_cache_path`, `read_`/`write_`/`clear_image_disk_cache*`, `evict_old_image_cache`, their private helpers) | `mbv-config`: the Owner's MPRIS hook and the core's Client MPRIS (`state/construct/remote.rs`) both pass `image_disk_cache_path` |
| `src/startup.rs`: `parse_log_level_arg` | `mbv-core` (`applog`, beside `LogSpec`, with an `applog` error in place of `UiModelError`). `print_usage` stays in `mbv-tui` |
| `src/owner_restart.rs`: pure `follow_up` decision | `mbv-client-core`; the terminal prompt goes to `mbv-tui` |
| `src/local_daemon.rs`: `run_local_daemon_main`, swap launch, tray hooks (the Owner host) | `mbv-tui`, as a root module beside `startup`, until `add-owner-program` moves it to `mbv-owner` |
| `src/local_daemon.rs`: `spawn_detached`, `local_daemon_args`, `session_display_env`, `to_io`, `READY_TIMEOUT`, `READY_POLL_INTERVAL` (a Client starts the Owner) | `mbv-client-core` (`owner_link`) |
| `src/local_daemon.rs`: `detach` (the Owner's swap launch and the Client's `spawn_detached` both use it) | `mbv-desktop` (`launch` module) |
| `src/single_instance.rs` (`resolve`, `Resolution`, `LockGuard` with `write_pid`, `socket_path`) | `mbv-daemon` (`owner_lock`, beside `try_lock`, `write_pid`, `locked_owner_pid`): one function serves the Owner's lock holder, `owner_link::find_owner`, and the core's Owner-restart and session-switch probes |
| `src/pin.rs`: `--pin` launch (`launch`, `run_argv`, `finish`, `exit_action`, `PinStartError`), `IN_PANEL_FLAG`, `toggle_running` and its failure report, `fatal` | `mbv-tui` |
| `src/pin.rs`: panel identity and swap-token claims (`claim_panel_identity`, `is_pinned`, `claim_swap_token`, `swap_token`); `src/pin/swap.rs` (`claim_pin_swap_participant`, `pin_swap_participant`, `pin_swap_hello`); the pinned-setting requests (`PinnedSettingRequest`, `PanelReserveMode`, `PanelGutter`, `spawn_pinned_request`, its argv builders and test seams) | `mbv-client-core` (`pin` module): core dispatch and state, `client_hello_claims`, and both Clients' `owner_link` call them. The TUI calls the claims at start and builds `run_argv` from the `pub` argv parts |
| `src/pin.rs`: `terminal_prefix`, `notify`, and the pinwin request runner (`PANEL_NAME`, `toggle_argv`, `run_request`, `request_result`, `PinRequestError`, `PinSpawnError`), `toggle_panel` | `mbv-desktop` (new `launch` module). `run_request` takes the pinwin socket as a parameter: the Owner host passes `None` (its starting Client removed `PINWIN_SOCKET`), the TUI and the core pass the claimed one. The Owner's tray hook calls `toggle_panel`; `toggle_running` and `spawn_pinned_request` call `run_request`. Nothing is copied |
| `src/launch.rs` (`launch_exe`) | `mbv-desktop` (`launch` module) |
| `src/startup.rs`: `install_signal_handlers`, `install_panic_hook`, `write_crash_log` | `mbv-tui` |
| `src/app/tests/` | each test moves with the code it owns: ratatui and tick-integration tests to `mbv-tui`, core tests to `mbv-client-core`, GUI tests to `mbv-gui` |

The leaf-crate moves (`mbv-desktop`, `mbv-daemon`, `mbv-core`, `mbv-config`) land first (tasks group 1), because the crates of group 4 and the Owner program of `add-owner-program` both need them.

### D4. Each Client wraps `App` by composition (restated)

`App` lives in `mbv-client-core`. The TUI shell `Model` owns it and the TUI-only state; `GuiCore { app, gui }` owns it and all GUI state (`decouple-client-core` built both). `App` fields stay `pub(in crate::app)` inside the core. A Client reads and changes `App` only through `pub` methods. No field becomes `pub`, also not under the `test` feature.

### D4b. The access surface lands before any crate move

Every crate move in this plan must be a path-only move. So the surface that the Clients need is built while everything is one crate, and the compiler proves it before any crate exists (D4c).

Measured at plan time (HEAD `9a7a2f11c`, before `decouple-client-core`). `decouple-client-core` moves some sites (terminal size, layout, GUI fields), so re-measure before you dispatch each row. These counts are guides for slicing; the compiler at the rehearsal (D4c) is the gate:

| Code that leaves the core | What it reaches | Count |
|---|---|---|
| Shell production code | `App` fields | 303 sites, 68 fields (37 assigned, 9 more changed through `&mut` calls) |
| Shell production code | `App` methods that are not `pub` | 197 methods, 315 sites |
| Shell production code | core modules and types through private paths (`src/app.rs` declares `dispatch`, `input`, `infra`, `client_drains`, ... as private) | 41 path uses |
| Shell tests, including `shell/**/tests/` directories | `App` fields | 250 + 42 sites |
| TUI-staying tests outside the shell (D4c defines the set) | `App` fields, under any binding name | about 900 sites |
| TUI-staying tests | private core methods that no Client's production code calls (`play_item`, `remove_from_queue`, `handle_player_event`, `drain_pinned_requests`, `switch_to_direct_remote`, ...) | about 31 methods |
| GUI modules | non-`gui_*` `App` fields, through `self.app` and through `app: &App` parameters | 187 + 4 sites, 42 fields |
| GUI modules | core methods that are not `pub` | about 127 methods |
| Core code | `pub(crate)` items (visible to a sibling module, not to another crate) | 32 in `src/app`, 35 in the root modules that join the core |

The rules:

1. Production read: a `pub` read method on `App` (`fn panel_mode(&self) -> PanelMode`), or an existing core method that already does the work.
2. Production write: first, a `pub` core method that names the intent and holds the logic the Client runs today (`fn open_settings(&mut self, dest)`), so the rule stays in the core. A plain setter only where the Client alone owns the decision (`set_list_pane_width`). Never a `&mut` getter that hands out a field.
3. A field that only one Client reads or writes leaves `App` for that Client (rule D2.2). It gets no method.
4. Private core methods and types that a Client's production code calls become `pub`, and their module path becomes reachable (`pub mod` or a `pub use` in `app.rs`). Core items use only `pub` or `pub(in crate::app)` (or narrower). `pub(crate)` is not used in the core, because it hides a crate boundary that does not exist yet.
5. Tests. A test that asserts only core behaviour and does not need the shell `Model` moves to the core, with its private access, and gets no seam. A test that needs the `Model` or the GUI state:
   - sets core state through `test`-feature fixture seams in `app_stub.rs` (the pattern `mbv-gui` already uses: `stub_app_with_queue_and_cmd_rx`). A seam names the fixture it builds (`seed_queue_column(width, hidden)`), not one field, and one seam serves every test that builds that fixture;
   - asserts through production read methods;
   - acts through production entry points (`Command` dispatch, `Msg`, the Client tick) where one exists, and else through a named `test`-feature action seam (`app_stub::play_item_for_test`). A private method never becomes `pub` only for a test.
6. The widening is transient where no Client needs it. The commits that widen are recorded in the task note (base and end commit). Task 4.1 narrows every widened item that no non-test target of `mbv-tui` or `mbv-gui` names, and keeps every `test`-feature seam.

`rg` patterns in the tasks find the bulk of each inventory. They are not the gate: they cannot see every binding name or path form. The gate is the compile of the rehearsal moves in D4c.

### D4c. Boundary rehearsal inside the root package

Before any new Client crate exists, the root package takes the module shape of the final crates:

- `crate::app` (`src/app/`, `src/app.rs`) holds exactly the future `mbv-client-core`. The root modules that join the core move under it first: `owner_link` (+ `owner_link/`), `client_hello_claims`, `config`, the core half of `pin` (+ `pin/swap.rs`), the pure `follow_up` of `owner_restart`, the Client half of `local_daemon`, and the `impl App` halves of `shell/playback.rs` and `shell/ui_state_reset.rs`.
- `crate::tui` (`src/tui.rs`, `src/tui/`) holds the shell (today `src/app/shell/`) and every TUI-staying test. The TUI-staying tests are every test file under `src/app/shell/`, and every other test file that names `shell::`, `Model`, `TickHarness`, ratatui, crossterm, tuirealm, `mbv_components`, or `mbv_render`.
- `crate::gui` (`src/gui.rs`, `src/gui/`) holds `GuiCore`, `GuiClientState`, the GUI modules, and the GUI routing entry (`input/gui.rs`, `input/gui/`).
- The other root modules (`main`, `startup`, the TUI half of `pin`, the `owner_restart` prompt, the Owner host in `local_daemon`) stay at the root; they go to `mbv-tui` with `crate::tui`.

Because `App` fields and the not-yet-widened core items are `pub(in crate::app)`, the compiler then rejects every access from `crate::tui`, `crate::gui`, and the root modules that would fail across a crate boundary. `crate::app` names neither `crate::tui` nor `crate::gui`. When the rehearsal compiles, the crate moves change only paths (`crate::app::` becomes `mbv_client_core::`).

A rehearsal move is a single `git mv` plus path edits, so it cannot compile in slices. Its rule: if the move leaves more than 40 compile errors, undo it, add a conversion slice for the largest error class, and try again. The rows before it are sized so that the move leaves only a few errors.

Alternative considered: rely on `rg` checks and fix the rest at the crate move. Rejected: that is how the first attempt failed. An `rg` check can pass while a binding name or a path form it does not know still reaches a private item.

### D10 (part). Packaging and build

- The `.deb` metadata moves from the root package to `crates/mbv-tui/Cargo.toml` with `name = "mbv"` in `[package.metadata.deb]`, and asset paths become relative to that crate. CI runs `cargo deb -p mbv-tui --no-build`. The assets stay `mbv` and `mbv-gui`.
- `Makefile` GUI targets build `-p mbv-tui -p mbv-gui`.
- `tools/release.sh` bumps the version in the root `[workspace.package]`, which it already edits. Check that its `sed` pattern still matches only that line once the root `[package]` is gone.
- The CI ast-grep rule `mbv-dispatch-no-wildcard-arm` changes its glob from `src/app/dispatch/**` to `crates/mbv-client-core/src/app/dispatch/**`.
- The unsafe-site audit comment in `build.yml` updates its file paths.

### D11. Dependencies

No new external dependency, and no version change. Each existing workspace dependency moves with its code at the version in `[workspace.dependencies]`:

- `ratatui` 0.30, `crossterm` 0.29, `tuirealm` 4.1, `ratatui-image` 11, `tui-scrollbar` 0.2.7: `mbv-tui`, `mbv-tui-support`, `mbv-components`, `mbv-render` only.
- `mimalloc` 0.1: `mbv-tui`.
- `nix` 0.31, `libc` 0.2: where the moved FFI sites go (`mbv-tui`, `mbv-client-core`, `mbv-desktop`, `mbv-daemon`).
- All internal crates are path dependencies at the workspace version.

### D12. Docs for agents

- Root `AGENTS.md`: the workspace map and the shared core only. It points to each Client's guide.
- New `crates/mbv-tui/AGENTS.md` (TUI rules, the TuiRealm and component rules, the TUI tests matrix) and `crates/mbv-gui/AGENTS.md` (GUI rules). Text that applies to one Client moves out of the root guide into that Client's guide.
- New ADR `docs/adr/0043-clients-are-separate-crates-over-one-client-core.md`. It amends ADR 0035 and ADR 0039 (the GUI links the core, not the TUI) and records D1, D2, D4b, and D4c.
- `CONTEXT.md`: update *Client core* to name its crate.
- Update `docs/architecture/tui-frontend.md`, `docs/architecture/gui-client.md`, `docs/architecture/interactive-tui-component-map.md`, and `docs/recipes.md` site lists to the new paths.

## Risks / Trade-offs

- [The move touches most of the workspace, so the diff is very large] → Move in compile-green steps: first the leaf-crate helpers, then the access surface, then the rehearsal, then the crates. Each step builds and tests green before the next.
- [Making `App` reads `pub` widens the core API, and the API mirrors `App` internals] → Add a read method only where a Client needs one; prefer intent methods; never make a field `pub`. Task 4.1 narrows every item that no Client needs. A deeper core API (splitting `App`) is out of scope.
- [Tests move with code and some depend on private items] → A test stays a `src/` test of the crate whose private items it uses. Public-API tests go in the crate's `tests/<name>.rs` binary. No visibility widens to keep a test: a TUI or GUI test that needs core state or a core action uses a `test`-feature seam (D4b rule 5).
- [A move row carries hundreds of private accesses and fails as one large error set, as the first attempt did] → D4b builds the access surface in compile-green slices, and the D4c rehearsal proves it with the compiler while everything is still one crate. The crate moves start only after the rehearsal compiles and its tests pass.
- [The plan's counts were measured before `decouple-client-core`] → Re-measure each inventory before its row is dispatched. A row's `rg` check is tested against the code before dispatch.
- [Compile time] → `mbv-gui` stops compiling ratatui, crossterm, and TuiRealm. A GUI edit no longer recompiles the TUI, and a TUI edit no longer recompiles the GUI modules.

## Migration Plan

No user migration. The installed programs, the Owner, its sockets, its lock path, and the prefs file do not change.

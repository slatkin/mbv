# Tasks

## 1. Binary lookup for launches (design D4)

- [x] 1.1 In `src/local_daemon.rs`, add `launch_exe(current, owner_pid)` next to `swap_argv` and call it from the `swap_command` hook in place of the raw `std::env::current_exe()` path. Add one `#[case]` table in that file's tests with two rows: a live path is returned unchanged; `<p> (deleted)` returns `/proc/<pid>/exe`. Name it for the regression it guards (Owner log `daemon.pin_swap.spawn_failed … (deleted)`, 2026-10-10 and 2026-10-11). Verify: `cargo nextest run -p mbv launch_exe`.
- [x] 1.2 In `swap_argv`, change the Unpin error text to "cannot start a terminal Client: neither [panel] terminal (config) nor TERMINAL (environment) is set". `swap_argv_without_a_terminal_names_the_missing_source` (`src/local_daemon.rs`) only checks that the text names `[panel] terminal` and `TERMINAL`, so it needs no edit. Verify: `cargo nextest run -p mbv swap_argv`.

## 2. Three-state attached Clients (design D1)

- [x] 2.1 In `mbv-ctrl`, add `AttachedClients { None, Unpinned, Pinned }` and `AttachedClientsCell` (an `AtomicU8` wrapper with `new(AttachedClients)`, `load`, `store`). Export them next to `OwnerAction`. Verify: `cargo check -p mbv-ctrl`.
- [x] 2.2 Replace `pinned_client_attached: Arc<AtomicBool>` with `attached_clients: Arc<AttachedClientsCell>` in `crates/mbv-daemon/src/ctrl.rs` (`CtrlClients`, and `refresh_pinned_client_attached` becomes `refresh_attached_clients` with the D1 rules), `core.rs` (`DaemonPlayerHandle`), `run.rs`, and `src/local_daemon.rs`. The compiler also forces edits at every `CtrlClients::new` call: `run.rs`, `core_ctrl_spawn.rs` (two tests), `ctrl.rs` (test helper), `event_loop/pin_swap.rs` (test helper), `tests/loop.rs`, `tests/basic.rs`. Update the two `ctrl.rs` tests that read the old field (`tcp_clients_are_never_swap_clients`, `pinned_flag_clears_when_the_pinned_client_leaves`) to read the enum; those already cover the TCP-only and pinned rows. Add one test for the new outcome: a local Client with no pinned surface gives `Unpinned`, and an Owner-action-role connection alone gives `None`. Verify: `cargo nextest run -p mbv-daemon ctrl`.

## 3. Open action and launch kind (design D2, D3)

- [x] 3.1 Add `OwnerAction::OpenTerminal` (serde `open-terminal`, flag `--open`, help "open mbv in a new terminal") to `crates/mbv-ctrl/src/owner_action.rs` and to `ALL`. Verify: `cargo check --workspace` shows no non-exhaustive match.
- [x] 3.2 In `crates/mbv-daemon/src/event_loop/pin_swap.rs`, add `LaunchKind { Swap, Open }` to `Phase::Awaiting` only (`Preparing` is always a swap). Add `PinSwapState::open(now)`, pass the kind to `spawn_replacement`, and derive the busy and abandon texts from the kind. Leave every `Swap` text unchanged. Route `OwnerAction::OpenTerminal` to it in `event_loop/owner_action.rs`. Add tests in the existing `pin_swap` test module, using its helpers: (a) `open` spawns with no `SwapPrepare` to an attached terminal Client, and its admit sends no `SwapQuit`; (b) the `open` deadline notifies "opening mbv timed out". The busy refusal needs no new test: `second_click_while_running_is_refused` owns it. Verify: `cargo nextest run -p mbv-daemon pin_swap`.
- [x] 3.3 In `crates/mbv-remote-player/src/connect/owner_actions.rs`, pass the action to `await_owner_action_reply` and map `CtrlEvent::CommandRejected(_)` to the existing `restart_required` error with the text "the running mbv is too old for {flag}; restart it" (design D2). Add one test next to `owner_action_refused_reply_surfaces_the_owner_reason` in `connect/tests.rs`, using its fake peer: a `CommandRejected` reply gives that restart error. Verify: `cargo nextest run -p mbv-remote-player owner_action`.

## 4. Tray item (spec pin-swap "The Tray offers Open…")

- [x] 4.1 In `crates/mbv-desktop/src/tray.rs`, take `Arc<AttachedClientsCell>` in `spawn`/`MbvTray`, and derive the Pin label from `Pinned`. When the state is `None`, add an **Open** item directly above **Pin** that sends `TransportCommand::OwnerAction(OwnerAction::OpenTerminal)`. The watcher compares the enum. Update the test helpers `spy_tray` and `spy_tray_with_flag`, and rewrite `pin_swap_label_follows_a_flag_flip_after_construction` (it builds an `AtomicBool`). Change the `pin_swap_item_label_and_activation_follow_the_pinned_flag` case table to take an `AttachedClients` value, with one row per state, and assert whether **Open** is present. Add one test that activating **Open** sends `OpenTerminal`. Verify: `cargo nextest run -p mbv-desktop tray`.

## 5. Domain docs (AGENTS.md "Add new terms with the change")

- [ ] 5.1 In `CONTEXT.md`, update the *Tray* entry (it "carries one Pin or Unpin item"; add the **Open** item that shows while no local Client is attached), the *Owner action* entry ("Pin swap is the first Owner action"; name Open as the second), and check the *Avoid* lists for a clash with "Open". Ask before any rename.

## 6. Integration gates

- [ ] 6.1 Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`, and verify both are clean.
- [ ] 6.2 Run `cargo nextest run --workspace` and verify it passes.

## Workflow follow-up

- Sync the delta specs to `openspec/specs/` and archive the change after review.

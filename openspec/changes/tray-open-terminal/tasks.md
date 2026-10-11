# Tasks

## 1. Binary lookup for launches (design D4)

- [ ] 1.1 In `src/local_daemon.rs`, add `launch_exe(current, exists, owner_pid)` next to `swap_argv` and call it from the `swap_command` hook in place of the raw `std::env::current_exe()` path. Add one `#[case]` table in that file's tests with three rows: a live path is returned unchanged; `<p> (deleted)` with `<p>` present returns `<p>`; `<p> (deleted)` with `<p>` absent returns `/proc/<pid>/exe`. Name it for the regression it guards (Owner log `daemon.pin_swap.spawn_failed … (deleted)`, 2026-10-10 and 2026-10-11). Verify: `cargo nextest run -p mbv launch_exe`.
- [ ] 1.2 In `swap_argv`, change the Unpin error text to "cannot start a terminal Client: neither [panel] terminal (config) nor TERMINAL (environment) is set". Update any existing assertion on the old text. Verify: `cargo nextest run -p mbv swap_argv`.

## 2. Three-state attached Clients (design D1)

- [ ] 2.1 In `mbv-ctrl`, add `AttachedClients { None, Unpinned, Pinned }` and `AttachedClientsCell` (an `AtomicU8` wrapper with `load`/`store`). Export them next to `OwnerAction`. Verify: `cargo check -p mbv-ctrl`.
- [ ] 2.2 Replace `pinned_client_attached: Arc<AtomicBool>` with `attached_clients: Arc<AttachedClientsCell>` in `crates/mbv-daemon/src/ctrl.rs` (`CtrlClients`, and `refresh_pinned_client_attached` becomes `refresh_attached_clients` with the D1 rules), `core.rs` (`DaemonPlayerHandle`), `run.rs`, and `src/local_daemon.rs`. Extend the existing registry flag test in `ctrl.rs` to a `#[case]` table: no connection → `None`; local terminal Client → `Unpinned`; local pinned Client → `Pinned`; TCP-only Client → `None`; Owner-action role only → `None`. Verify: `cargo nextest run -p mbv-daemon ctrl`.

## 3. Open action and launch kind (design D2, D3)

- [ ] 3.1 Add `OwnerAction::OpenTerminal` (serde `open-terminal`, flag `--open`, help "open mbv in a new terminal") to `crates/mbv-ctrl/src/owner_action.rs` and to `ALL`. Verify: `cargo check --workspace` shows no non-exhaustive match.
- [ ] 3.2 In `crates/mbv-daemon/src/event_loop/pin_swap.rs`, add `LaunchKind { Swap, Open }` to `Phase::Preparing`/`Phase::Awaiting`, add `PinSwapState::open(now)`, and derive the busy and abandon texts from the kind. Leave every `Swap` text unchanged. Route `OwnerAction::OpenTerminal` to it in `event_loop/owner_action.rs`. Add tests in the existing `pin_swap` test module, using its helpers: (a) `open` spawns with no `SwapPrepare` to an attached terminal Client, and its admit sends no `SwapQuit`; (b) `open` while a swap is `Preparing` is refused and spawns nothing; (c) the `open` deadline notifies "opening mbv timed out". Verify: `cargo nextest run -p mbv-daemon pin_swap`.

## 4. Tray item (spec pin-swap "The Tray offers Open…")

- [ ] 4.1 In `crates/mbv-desktop/src/tray.rs`, take `Arc<AttachedClientsCell>` in `spawn`/`MbvTray`, and derive the Pin label from `Pinned`. When the state is `None`, add an **Open** item directly above **Pin** that sends `TransportCommand::OwnerAction(OwnerAction::OpenTerminal)`. The watcher compares the enum. Change the existing `pin_swap_item_label_and_activation_follow_the_pinned_flag` case table to take an `AttachedClients` value, with one row per state, and assert whether **Open** is present. Add one test that activating **Open** sends `OpenTerminal`. Verify: `cargo nextest run -p mbv-desktop tray`.

## 5. Integration gates

- [ ] 5.1 Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`, and verify both are clean.
- [ ] 5.2 Run `cargo nextest run --workspace` and verify it passes.

## Workflow follow-up

- Sync the delta specs to `openspec/specs/` and archive the change after review.

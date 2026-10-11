# Design

## Context

See proposal.md for the reason for this change. These facts about the code shape the design:

- The Tray (`crates/mbv-desktop/src/tray.rs`) reads one shared `Arc<AtomicBool>`,
  `pinned_client_attached`. The ctrl registry (`crates/mbv-daemon/src/ctrl.rs`,
  `refresh_pinned_client_attached`) sets it on every connect and disconnect. A watcher thread polls
  it each second and pushes a menu rebuild when it changes.
- `PinSwapState` (`crates/mbv-daemon/src/event_loop/pin_swap.rs`) already handles a launch with no
  target. `Pin` with no Client goes directly to `spawn_replacement(command, None, deadline)`.
  `on_swap_admitted` sends `SwapQuit` only when the target is `Some`.
- The swap command hook (`src/local_daemon.rs`, `swap_command`) calls `std::env::current_exe()`
  for each launch. On Linux this reads `/proc/self/exe`. After the file is unlinked, the kernel
  adds ` (deleted)` to the path, and `Command::spawn` fails with ENOENT.
- `OwnerAction` (`crates/mbv-ctrl/src/owner_action.rs`) is the only place that holds the CLI
  flag strings. `ALL` drives `--help` and `from_cli_flag`. `event_loop/owner_action.rs` is the one
  handler that both the Tray and the CLI use.

## Goals / Non-Goals

**Goals:**

- One shared value tells the Tray which Clients are attached. Invalid combinations cannot exist.
- Open reuses the existing launch machine, terminal command, and notifications.
- Launches keep working after the file that the Owner started from is removed or replaced.

**Non-Goals:**

- `spawn_detached` (a Client starting the Owner) keeps `current_exe()`. A Client is started
  fresh, so its binary is rarely stale. Change it only if logs show a failure.
- A Wayland or desktop `.desktop` launcher entry.
- New terminal discovery. Open uses only `[panel] terminal` or `TERMINAL`.

## Decisions

### D1. Three-state `AttachedClients` replaces the pinned flag

Add to `mbv-ctrl` (the Tray and the daemon both depend on it):

```rust
pub enum AttachedClients { None, Unpinned, Pinned }
pub struct AttachedClientsCell(AtomicU8);   // load() -> AttachedClients, store(AttachedClients)
```

The registry computes the value in one function, which replaces
`refresh_pinned_client_attached`:

- `Pinned`: a local Client has the `Pinned` swap surface (this is the current rule).
- `Unpinned`: no pinned Client, and there is at least one local connection with the `Client` role.
  This also includes a Client that does not advertise `pin-swap`.
- `None`: otherwise. Owner-action and admin roles, and TCP Clients, do not count.

`DaemonPlayerHandle.pinned_client_attached` becomes `attached_clients: Arc<AttachedClientsCell>`.
The tray watcher compares the enum in place of the bool.

*Alternative:* a second `AtomicBool` named `any_client_attached`. This was rejected because two
flags can show "pinned but none attached".

### D2. `OwnerAction::OpenTerminal`, flag `--open`

Add the new variant to `ALL`, with `cli_flag` `--open`, the `help` text "open mbv in a new
terminal", and the serde name `open-terminal`. `event_loop/owner_action.rs` sends it to
`PinSwapState::open(now)`. The match stays exhaustive.

An older Owner that supports Owner actions advertises the `owner-action` capability, so the CLI's
"too old" check passes. That Owner cannot parse `open-terminal`, and it replies `CommandRejected`
(`core_ctrl_spawn.rs`). Today `await_owner_action_reply`
(`crates/mbv-remote-player/src/connect/owner_actions.rs`) skips that event. The CLI then waits for
the 6 s read timeout and prints "owner-action reply unavailable". The fix: pass the action to
`await_owner_action_reply` and map `CommandRejected` to the existing `restart_required` error, with
the same text as the capability check: "the running mbv is too old for --open; restart it". The
only command that an Owner-action connection can send is `RunOwnerAction`, so a `CommandRejected`
there always means the Owner cannot run the action. No protocol bump is needed.

### D3. Open is a launch kind in `PinSwapState`

Add a `LaunchKind { Swap, Open }` to `Phase::Awaiting` only. `Preparing` always belongs to a swap
(it has a target and waits for `SwapPrepared`), so `Open` there would be an unreachable state.
`on_swap_prepared` passes `LaunchKind::Swap`. `open(now)`:

1. Returns the busy refusal when the phase is not `Idle`. The text depends on the kind that is in
   flight: `BUSY_MESSAGE` stays the same for `Swap`, and `Open` uses "mbv is already opening".
2. Resolves `(self.swap_command)(SwapDirection::Unpin)`. On `Err` it notifies and returns `Err`,
   the same as `start`.
3. Calls `spawn_replacement(command, None, deadline, LaunchKind::Open)`. It never sends
   `SwapPrepare`, and it never reads the registry.

The abandon reasons get their prefix from the kind. For `Swap` the text stays as it is now, so
the pin-swap scenarios do not change. For `Open` the texts are "opening mbv timed out", "could not
open mbv: could not start {program}: {error}", and "opening mbv failed: the new client exited
before attaching". The swap token and admission path do not change. The replacement attaches with
the token, and `on_swap_admitted` finds `target: None`.

The `Unpin` error from `swap_argv` drops the words "to unpin". The new text is "cannot start a
terminal Client: neither [panel] terminal (config) nor TERMINAL (environment) is set". This lets
Open and Unpin send the same notification. The spec only requires that it names both settings.

*Alternative:* a separate `OpenState` machine. This was rejected because it would duplicate the
deadline, child-waiter, and token logic, and it would need its own single-flight rule.

### D4. Binary lookup for launches

Add a pure function next to `swap_argv` in `src/local_daemon.rs`:

```rust
fn launch_exe(current: &Path, owner_pid: u32) -> PathBuf
```

- If `current` does not end in ` (deleted)`, return `current`.
- Otherwise return `/proc/{owner_pid}/exe`. This starts the same build as the Owner. The path is
  valid while the Owner runs. It also works through a terminal emulator, because the path names
  the Owner's pid, not `self`.

The `swap_command` hook calls `launch_exe(&current_exe()?, std::process::id())`.

Both logged failures (`theme-slot-model/target/debug/mbv (deleted)` and
`home-recently-added-tree/target/release/mbv (deleted)`) are removed worktrees, so the fallback
alone fixes them.

*Alternative:* when the original path exists again (an upgrade or a rebuild), start that file.
This was rejected. A local Client refuses an Owner whose `app_version` differs from its own,
before it sends any hello (`mbv-remote-player/src/connect/handshake.rs`,
`docs/invariants/15-local-client-credential-gated-on-owner-build.md`). After a version change the
new build would start, refuse to attach, and the launch would time out. The Owner's own build
always passes that gate.

## Risks / Trade-offs

- [After an in-place rebuild or upgrade, a launch starts the Owner's older build] → This is
  required by the build gate (D4). The user restarts mbv to get the new build, as with any Owner
  change.
- [The ` (deleted)` suffix is a Linux procfs detail] → mbv targets Linux only. If the suffix is
  absent, the path is returned unchanged.
- [`/proc/<pid>/exe` shows a strange argv[0] in `ps`] → This is acceptable for a fallback that
  only runs when the binary was removed.

## Migration Plan

None. There is no persisted state and no config change. A running old Owner needs a restart to
offer Open, as with any Tray change.

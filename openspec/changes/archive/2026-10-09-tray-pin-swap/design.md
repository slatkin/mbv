# Design

## Context

See proposal.md for the motivation. The constraints that shape the approach:

- Pinned mode is fixed at process start. `src/pin.rs` opens a pty, starts the pinwin panel on its
  master side, detaches the controlling terminal and points stdio at the pty, all before the TUI
  connects to the Owner process (`src/main.rs`, `pin::start`). Nothing in a running TUI can move
  it into or out of a panel.
- The Owner process holds the Player and the Bound queue, so any Client can leave and another can
  attach without touching playback.
- With Stay-alive off, the Owner refuses a second Client (`send_admission_refusal`,
  `crates/mbv-daemon/src/core_ctrl_spawn.rs`, `DisconnectReason::ExclusiveOwner`), exits when its
  last Client leaves (`LastClientGone`), and a quitting TUI requests coordinated shutdown, which
  the Owner accepts whatever the Client count (`teardown`,
  `src/app/dispatch/run_loop/teardown.rs`).
- The Tray lives in the Owner process (`crates/mbv-desktop/src/tray.rs`). It reaches the Owner only
  through `transport_tx: Sender<TransportCommand>` and reads `Arc<Mutex<PlayerStatus>>`. Both come
  from the `on_tray_ready` hook in `src/local_daemon.rs`.
- `mbv-daemon` is shared with headless `mbvd`. Desktop and process-launch concerns stay out of it:
  the TUI binary supplies them through `DaemonRuntimeHooks`, as it already does for the Tray.
- `TuiLaunchState` is saved by `teardown(launch_state)` and restored at startup.

## Goals / Non-Goals

**Goals:**
- A Tray click moves the session between terminal and pinned panel with no playback gap, under
  either Stay-alive setting.
- A failed swap never leaves the user with no Client.

**Non-Goals:**
- Moving a live TUI process between terminals (see D1).
- Carrying state beyond `TuiLaunchState`.
- Any pinwin change.
- A keybinding form of the swap. The Tray and `mbv --swap-panel` are the only triggers.
- Moving the other Tray items (Play/Pause, Next, Previous, Quit) onto Owner actions. D7 makes that
  a later, mechanical change.
- The Tray starting `mbv --swap-panel` as a child process. The Tray lives in the Owner process and
  sends the same `OwnerAction` in-process (D7).
- Swapping Clients attached over TCP, or Clients of `mbvd`.

## Decisions

### D1: Swap Clients, do not re-home the process

The Owner starts a new Client in the other mode, then ends the old one.

Alternative considered: re-point the running TUI's stdio between the terminal's pty and a pinwin
pty. Pinning would work, but unpinning needs a terminal to return to. That terminal is gone when
the session started pinned (desktop entry) or the user closed the window, and a new terminal
emulator cannot adopt an existing process. Rejected.

### D2: Ctrl vocabulary

All of it is additive and negotiated by capability. No protocol version bump.

- Capability `pin-swap`: the Client handles `SwapPrepare` and `SwapQuit`. Every TUI Client built
  with this change advertises it. Older terminal Clients never become Pin targets. An older pinned
  Client can still be an Unpin target: it never answers `SwapPrepared`, so the swap abandons at the
  deadline and the pinned Client stays. Accepted for a single-user setup.
- Capability `pinned-surface`: the Client runs in a pinned panel. A Client advertises it when
  `pin::is_pinned()` is true at Hello time. That is always settled, because `pin::start` runs
  before the Owner connection.
- `CtrlHello.swap_token: Option<String>` (`#[serde(default)]`): set by a Client the Owner started
  for a swap. The value comes from the `MBV_SWAP_TOKEN` environment variable. The Client removes
  the variable from its own environment once it has read it, before it spawns any worker.
- `CtrlEvent::SwapPrepare` (Owner to target): save launch state now.
- `CtrlCmd::SwapPrepared` (target to Owner): saved.
- `CtrlEvent::SwapQuit` (Owner to target): exit now as swapped out.

The Owner's registry stores one value per Client, derived at Hello:
`enum SwapSurface { Terminal, Pinned }`, held as `Option<SwapSurface>`. `None` means the Client
lacks `pin-swap`. It is never two bools.

Alternative considered: a "surface" field on `CtrlHello` instead of a capability. Rejected. The
existing pattern is capabilities, and a capability needs no new field. `swap_token` does need a
field, because it carries a value.

### D3: Swap state machine in the daemon loop

New module `crates/mbv-daemon/src/event_loop/pin_swap.rs`, sibling to `event_loop/tray.rs`. It
follows that module's shape: owned state, and a `poll(now)` that the loop calls with an injected
`Instant` so tests need no sleep.

```
 Idle --click, target T--> Preparing{T, cmd, deadline} --SwapPrepared from T--> spawn
 Idle --click, no target--> spawn
 spawn ok  --> Awaiting{T?, token, deadline}
 Awaiting --Client attaches with token--> send SwapQuit to T (if any) --> Idle
 any --deadline / spawn error / child exit != 0 / T gone while Preparing--> notify --> Idle
```

- A swap starts from `OwnerAction::SwapPanel` (D7), whether the Tray or the CLI sent it. The loop
  picks the direction from the registry, not from the label the Tray showed, so a stale menu
  cannot act on the wrong Client.
- Starting returns `Result<(), String>`. The `Err` reason is "a panel swap is already running" or
  the unresolvable-command message (D5). The CLI reports it; the Tray drops it, because the
  unresolvable case already notifies and a busy click stays silent.
  Pinned Client attached: Unpin, and the target is that Client. Otherwise: Pin, and the target is
  the last Client in connection order with `Some(Terminal)`, or none.
- A request while the machine is not `Idle` starts nothing and returns the busy refusal.
- The deadline is 10 s from the click and covers both phases.
- If the target leaves during `Awaiting`, the swap still completes when the new Client attaches.
- The Tray label reads a shared `Arc<AtomicBool>` ("pinned Client attached"). It is added to
  `DaemonPlayerHandle` beside `status` and updated by `CtrlClients` on connect and disconnect.
  The Tray already reads `status` the same way.

### D4: Admission exception

`send_admission_refusal` gains the pending token. A Hello whose `swap_token` equals the
`Awaiting` token is admitted despite `ExclusiveOwner`, and the token is consumed at once, so it
works for one connection only. The shutting-down refusal still wins. With no swap pending, or a
wrong token, behaviour is unchanged. The token is a `uuid::Uuid::new_v4()` string, the same source
as the Control credential. It travels only through the child's environment and the
Control-authenticated Unix socket.

### D5: Process launch belongs to the TUI binary

New hook `DaemonRuntimeHooks::swap_command: Box<dyn Fn(SwapDirection) -> Result<Command, String> + Send>`.
`mbvd` passes a hook that returns `Err`. It never has a Tray, so the hook is never reached there.
The `src/local_daemon.rs` hook builds the command:

- Pin: `current_exe() --pin`.
- Unpin: the argv prefix from `[panel] terminal` (re-read from the config file, as
  `owner_settings::reader` does), otherwise `$TERMINAL` from the Owner's own environment followed
  by `-e`, then `current_exe()`. Neither set gives `Err` with a message naming both. That
  message is notified, and no `SwapPrepare` is sent.
- Both: stdin, stdout and stderr null, and `setsid` in `pre_exec`, the same as `spawn_detached`.
  The Owner's environment already carries the session display variables.

The loop resolves the command before it sends `SwapPrepare`, so an Unpin that cannot run saves
nothing and touches no Client. After `SwapPrepared`, the loop sets `MBV_SWAP_TOKEN`, spawns, and
moves the `Child` into a waiter thread. The thread reaps it and sends
`DaemonEvent::PinSwapChildExited { token, success }`. Only a non-success exit while `Awaiting`
abandons the swap. A terminal emulator that forks and exits 0 is normal and is ignored.

Failure notification: a second hook, `notify: Box<dyn Fn(&str) + Send>`. The TUI binary
implements it with the existing `notify-send` helper from `src/pin.rs`, made `pub(crate)`.

### D6: Client side

- `mbv-remote-player`'s connect path builds the Hello with the two capabilities and `swap_token`.
  The caller passes `pinned: bool` and `swap_token: Option<String>` in, and `connect.rs` maps them.
- `SwapPrepare` and `SwapQuit` become `PlayerEvent` variants that `connect.rs` forwards, the same
  way as `CommandRejected`.
- On `SwapPrepare`, the shell takes the same launch snapshot as `teardown` uses
  (`launch_state_snapshot`), saves it, and sends `SwapPrepared`. If the save fails, the failure is
  logged and `SwapPrepared` is still sent. The swap is worth more than the snapshot.
- On `SwapQuit`, the shell sets the exit kind and requests quit. `teardown` takes
  `enum ExitKind { Quit, SwappedOut }` in place of the launch-state `Option`. `SwappedOut` skips
  both the launch-state save and the coordinated shutdown request. Everything else in teardown
  still runs. A bool flag is not used: the exit kind decides two behaviours together.

### D7: Owner actions, one vocabulary for the Tray and the CLI

The swap is the first Owner action. The goal: a later Tray item becomes an Owner action, with a
CLI flag, by adding one variant, one flag row and one handler arm. No new ctrl command, capability
or connection kind.

- `mbv-ctrl` owns `enum OwnerAction { SwapPanel }`, each variant with a fixed
  `serde(rename)` (`"swap-panel"`). `OwnerAction::ALL` lists every variant. `cli_flag(self)`
  (`"--swap-panel"`) and `help(self)` are exhaustive matches, and `from_cli_flag(&str)` searches
  `ALL`. These are the only places a flag string lives.
- Ctrl vocabulary: capability `owner-action`, `CtrlHello::current_owner_action(control_token)`,
  `CtrlCmd::RunOwnerAction(OwnerAction)`, and the replies `CtrlEvent::OwnerActionAccepted` and
  `CtrlEvent::OwnerActionRefused { reason: String }`. `TransportCommand::OwnerAction(OwnerAction)`
  carries the same value from the Tray.
- Connection role. The Owner classifies each Hello into one
  `enum CtrlConnectionRole { Client, ServiceSetupAdmin, OwnerAction }`. It replaces the
  `service_setup_admin` bool in `core_ctrl_spawn.rs` and `admin_only` in the registry, so the two
  non-Client kinds are not two flags. A Hello that advertises both admin capabilities is rejected.
  Both non-Client roles keep today's admin rules: let in from the local transport while Stay-alive
  is off, never a driver, never held, never a Pin target. The shutting-down refusal still wins. An
  `OwnerAction` connection is refused outright on TCP, and any command other than `RunOwnerAction`
  gets `CommandRejected`, the same as the admin role's check.
- One handler. `event_loop/owner_action.rs` has `run(action) -> Result<(), String>`, an exhaustive
  match (`SwapPanel` => the D3 start). The loop calls it for `CtrlCmd::RunOwnerAction`, replies
  accepted or refused on the connection's `reply_tx`, and calls it for
  `TransportCommand::OwnerAction` with the result dropped.
- Client. `mbv-remote-player::run_local_owner_action(action)` follows
  `signal_local_daemon_service_setup`. It connects to `control_socket_path()`, does the handshake
  with the `OwnerAction` role, sends the command and waits for the reply under a 6 s read timeout.
  The handshake's `service_setup_admin: bool` becomes a role enum with the same three meanings. No
  socket gives "no running mbv". A server Hello without `owner-action` gives "the running mbv is too
  old for --swap-panel; restart it", and nothing is sent.
- CLI. `main.rs` has one dispatch site, placed beside `--toggle` before applog and config. The
  first argument that `from_cli_flag` accepts selects the action. mbv runs it, prints the error to
  stderr and exits 1 on `Err`, and exits 0 on `Ok`. The other arguments are ignored, the same as
  `-q`. `print_usage` prints one row per `ALL` entry from `cli_flag` and `help`.

Alternative considered: a dedicated `CtrlCmd::PinSwap` and a `--swap-panel`-only path. It is fewer
lines today, but each later Tray item would add a command, a capability and a CLI branch. Rejected.

Alternative considered: have the Tray start `mbv --swap-panel`. The Tray is already inside the
Owner, so that adds a process and a socket round trip back to itself to deliver the same value.
Rejected. The shared `OwnerAction` and handler are the single entry point.

## Risks / Trade-offs

- [`$TERMINAL` comes from the TUI that started the Owner. With Stay-alive on, a long-lived Owner
  keeps an old value.] → Document it beside `[panel] terminal`. The config key is re-read on each
  swap and is the reliable path.
- [A terminal that does not accept `-e`.] → `[panel] terminal` takes a full argv prefix (for
  example `["wezterm", "start", "--"]`).
- [The user moves the cursor in the old Client between `SwapPrepare` and `SwapQuit`, about a
  second.] → That movement is lost. This is accepted.
- [A pinned start that fails after spawn.] → `mbv --pin` with no tty already sends its own
  notification and exits 1. The Owner then sees `success: false` and abandons the swap with a
  second, generic notification. Two notifications is acceptable for a rare failure.
- [Pin while another pinned mbv already runs on the display.] → Not reachable through the menu,
  because a pinned Client attached makes it Unpin. A pinned mbv attached to a different Owner
  exits 0 with `ShownExisting`. The swap then times out and notifies, and the old Client stays.
- [`OwnerAction::ALL` is a hand-kept list, and the compiler does not force a new variant into it.]
  → A variant missing from `ALL` has no flag and no help row. The `mbv-ctrl` flag round-trip test
  covers every listed variant. Adding a variant to `cli_flag` without `ALL` is a review catch.
- [`mbv --swap-panel` exits 0 before the swap finishes.] → The exit status means "accepted", not
  "swapped". A later failure arrives as the desktop notification.

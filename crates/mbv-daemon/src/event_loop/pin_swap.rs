//! Pin-swap state machine for the daemon event loop (tray-pin-swap design
//! D3). One swap runs at a time: the machine picks the direction and the
//! target Client from the ctrl registry, asks the runtime hook for the
//! replacement command, waits for the target's `SwapPrepared`, then spawns
//! the replacement and waits for it to attach (the admission exception,
//! task 3.4, completes the swap). Every failure path notifies and returns
//! the machine to `Idle`, leaving the old Client attached.

use crate::DaemonEvent;
use crate::core::{NotifyHook, SwapCommandHook, SwapDirection};
use crate::ctrl::{ClientRegistry, CtrlClientId, SwapSurface};
use mbv_ctrl::{CtrlEvent, SWAP_TOKEN_ENV};
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A swap must complete — the replacement Client attached — within this
/// window of the click (design D3). One deadline covers both the Prepare
/// phase and the wait for the replacement to attach.
const SWAP_DEADLINE: Duration = Duration::from_secs(10);

/// The reason a second start returns while a swap is in flight (spec
/// pin-swap "Second click during a swap").
const BUSY_MESSAGE: &str = "a panel swap is already running";

/// The reason a second `open` returns while an Open launch is in flight
/// (spec pin-swap "Open shares the Pin swap launch rules").
const OPEN_BUSY_MESSAGE: &str = "mbv is already opening";

/// The one-shot admission token of the swap currently `Awaiting` its
/// replacement (tray-pin-swap design D4). The machine publishes it when it
/// spawns the replacement and clears it whenever the swap leaves `Awaiting`;
/// the ctrl admission path consumes it when a Hello presents it, so it works
/// for one connection only. Shared behind an `Arc` because admission runs on
/// the per-connection ctrl threads, not on the event loop.
#[derive(Default)]
pub(crate) struct PendingSwapToken {
    token: Mutex<Option<String>>,
}

impl PendingSwapToken {
    pub(crate) fn publish(&self, token: String) {
        *self.token.lock().unwrap() = Some(token);
    }

    /// Consumes the pending token when `candidate` matches it. `true` means
    /// this Hello is the replacement Client attaching; the token is spent —
    /// a second presenting Hello finds nothing pending.
    pub(crate) fn take_matching(&self, candidate: Option<&str>) -> bool {
        let mut pending = self.token.lock().unwrap();
        let matched = candidate.is_some() && pending.as_deref() == candidate;
        if matched {
            *pending = None;
        }
        matched
    }

    fn clear(&self) {
        *self.token.lock().unwrap() = None;
    }
}

/// What the launch in flight is for. `Preparing` is always a swap (it has a
/// target and waits for `SwapPrepared`), so `LaunchKind` lives on `Awaiting`
/// only (spec pin-swap "Open shares the Pin swap launch rules").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaunchKind {
    /// Replace one Client with another (the Pin swap).
    Swap,
    /// Start a fresh terminal Client with no effect on any attached Client.
    Open,
}

impl LaunchKind {
    /// The busy refusal for a second request while this kind is in flight.
    fn busy(self) -> &'static str {
        match self {
            LaunchKind::Swap => BUSY_MESSAGE,
            LaunchKind::Open => OPEN_BUSY_MESSAGE,
        }
    }

    /// The abandon reason when the deadline passes before the launch
    /// attaches.
    fn timed_out(self) -> &'static str {
        match self {
            LaunchKind::Swap => "panel swap timed out; the old client stays attached",
            LaunchKind::Open => "opening mbv timed out",
        }
    }

    /// The abandon reason when the replacement process could not start.
    fn spawn_failed(self, program: &str, error: &std::io::Error) -> String {
        match self {
            LaunchKind::Swap => format!("panel swap could not start {program}: {error}"),
            LaunchKind::Open => format!("could not open mbv: could not start {program}: {error}"),
        }
    }

    /// The abandon reason when the replacement process exited before it
    /// attached.
    fn child_failed(self) -> &'static str {
        match self {
            LaunchKind::Swap => "panel swap abandoned: the new client exited before attaching",
            LaunchKind::Open => "opening mbv failed: the new client exited before attaching",
        }
    }

    /// The Owner-log line when the launch process spawns.
    fn spawned_message(self) -> &'static str {
        match self {
            LaunchKind::Swap => "panel swap replacement client spawned",
            LaunchKind::Open => "open terminal client spawned",
        }
    }

    /// The Owner-log line when the launch process fails to spawn.
    fn spawn_failed_message(self) -> &'static str {
        match self {
            LaunchKind::Swap => "panel swap spawn failed",
            LaunchKind::Open => "open terminal client spawn failed",
        }
    }
}

/// One swap in flight, from the click to the replacement Client's attach or
/// the abandon (design D3).
enum Phase {
    Idle,
    /// `SwapPrepare` sent; waiting for the target's `SwapPrepared`.
    Preparing {
        target: CtrlClientId,
        command: Command,
        deadline: Instant,
    },
    /// Replacement process spawned; waiting for it to attach (task 3.4's
    /// admission exception consumes the token and completes the swap).
    Awaiting {
        target: Option<CtrlClientId>,
        token: String,
        deadline: Instant,
        kind: LaunchKind,
    },
}

/// The loop's Pin-swap state: the phase, the runtime hooks, and the merged
/// event channel the child-waiter thread reports on. `poll(now)` takes an
/// injected `Instant`, so tests drive the deadline without sleeping.
pub(crate) struct PinSwapState {
    phase: Phase,
    swap_command: SwapCommandHook,
    notify: NotifyHook,
    merged_tx: mpsc::Sender<DaemonEvent>,
    /// Shared with the ctrl admission path (design D4).
    pending_token: Arc<PendingSwapToken>,
}

impl PinSwapState {
    pub(crate) fn new(
        swap_command: SwapCommandHook,
        notify: NotifyHook,
        merged_tx: mpsc::Sender<DaemonEvent>,
        pending_token: Arc<PendingSwapToken>,
    ) -> Self {
        Self {
            phase: Phase::Idle,
            swap_command,
            notify,
            merged_tx,
            pending_token,
        }
    }

    /// Starts a swap from an Owner action (design D3). The direction and the
    /// target come from the registry, not from the Tray label: a pinned
    /// Client attached means Unpin against that Client, otherwise Pin against
    /// the last-connected terminal Client — or no target at all, which
    /// spawns directly. `Err` means the click did nothing: the busy refusal,
    /// or an unresolvable command after the user was notified.
    /// Resolves the launch command through the runtime hook, notifying the
    /// user on failure. `launch` names the launch in the Owner log.
    fn resolve_command(
        &self,
        direction: SwapDirection,
        launch: &'static str,
    ) -> Result<Command, String> {
        match (self.swap_command)(direction) {
            Ok(command) => Ok(command),
            Err(reason) => {
                tracing::warn!(
                    name: "daemon.pin_swap.unresolvable",
                    target: "pin_swap",
                    reason = %reason,
                    "{launch} command unresolvable"
                );
                (self.notify)(&reason);
                Err(reason)
            }
        }
    }

    pub(crate) fn start(
        &mut self,
        now: Instant,
        ctrl_clients: &ClientRegistry,
    ) -> Result<(), String> {
        if !matches!(self.phase, Phase::Idle) {
            return Err(self.busy_message().to_string());
        }
        let deadline = now + SWAP_DEADLINE;
        let clients = ctrl_clients.lock().unwrap();
        let (direction, target) = match clients.newest_swap_client(SwapSurface::Pinned) {
            Some(pinned) => (SwapDirection::Unpin, Some(pinned)),
            None => (
                SwapDirection::Pin,
                clients.newest_swap_client(SwapSurface::Terminal),
            ),
        };
        let command = match self.resolve_command(direction, "panel swap") {
            Ok(command) => command,
            Err(reason) => {
                drop(clients);
                return Err(reason);
            }
        };
        match target {
            Some(target) => {
                // The command is resolved before `SwapPrepare` goes out, so
                // an Unpin that cannot run saves nothing and touches no
                // Client (design D5).
                clients.send_to_client(target, &CtrlEvent::SwapPrepare);
                tracing::info!(name: "daemon.pin_swap.prepare_sent", target: "pin_swap", client = %target, direction = ?direction, "SwapPrepare sent to the swap target");
                self.phase = Phase::Preparing {
                    target,
                    command,
                    deadline,
                };
            }
            None => self.spawn_replacement(command, None, deadline, LaunchKind::Swap),
        }
        Ok(())
    }

    /// The busy refusal for a request while the machine is not `Idle`. The
    /// text names the kind that is in flight: `Preparing` is always a swap.
    fn busy_message(&self) -> &'static str {
        match &self.phase {
            Phase::Idle => BUSY_MESSAGE,
            Phase::Preparing { .. } => LaunchKind::Swap.busy(),
            Phase::Awaiting { kind, .. } => kind.busy(),
        }
    }

    /// Opens a fresh terminal Client from an Owner action (spec pin-swap
    /// "Open starts a terminal Client"). It reuses the swap launch machine
    /// with no target: no `SwapPrepare` is sent, the registry is not read,
    /// and no attached Client is affected. The replacement attaches with the
    /// one-shot token and the admission path completes the launch as it
    /// completes a swap.
    pub(crate) fn open(&mut self, now: Instant) -> Result<(), String> {
        if !matches!(self.phase, Phase::Idle) {
            return Err(self.busy_message().to_string());
        }
        let deadline = now + SWAP_DEADLINE;
        let command = self.resolve_command(SwapDirection::Unpin, "terminal client")?;
        self.spawn_replacement(command, None, deadline, LaunchKind::Open);
        Ok(())
    }

    /// The target answered `SwapPrepared` (launch state saved): stamp the
    /// one-shot token, spawn, and wait for the replacement to attach. A
    /// `SwapPrepared` from any other Client, or while not `Preparing`, is
    /// ignored.
    pub(crate) fn on_swap_prepared(&mut self, client_id: CtrlClientId) {
        let phase = std::mem::replace(&mut self.phase, Phase::Idle);
        match phase {
            Phase::Preparing {
                target,
                command,
                deadline,
            } if target == client_id => {
                tracing::info!(name: "daemon.pin_swap.prepared", target: "pin_swap", client = %target, "swap target saved launch state; spawning the replacement");
                self.spawn_replacement(command, Some(target), deadline, LaunchKind::Swap);
            }
            other => self.phase = other,
        }
    }

    /// A ctrl Client left. Leaving while `Preparing` abandons the swap — the
    /// target can no longer be replaced. Leaving while `Awaiting` keeps the
    /// swap: it still completes when the replacement attaches (design D3).
    pub(crate) fn on_client_left(&mut self, client_id: CtrlClientId) {
        if let Phase::Preparing { target, .. } = &self.phase
            && *target == client_id
        {
            let reason = "panel swap abandoned: the client being replaced left";
            tracing::warn!(name: "daemon.pin_swap.target_left", target: "pin_swap", client = %client_id, reason);
            self.abandon(reason);
        }
    }

    /// Notifies the user, spends any published token, and returns to `Idle`.
    fn abandon(&mut self, reason: &str) {
        (self.notify)(reason);
        self.pending_token.clear();
        self.phase = Phase::Idle;
    }

    /// The child-waiter thread reports the replacement process's exit. Only
    /// a non-success exit while `Awaiting` abandons the swap: a terminal
    /// emulator that forks and exits 0 is normal and is ignored (design D5).
    pub(crate) fn on_child_exited(&mut self, token: &str, success: bool) {
        let Phase::Awaiting {
            token: pending,
            kind,
            ..
        } = &self.phase
        else {
            return;
        };
        if pending != token || success {
            return;
        }
        let reason = kind.child_failed();
        tracing::warn!(name: "daemon.pin_swap.child_failed", target: "pin_swap", reason);
        self.abandon(reason);
    }

    /// Abandons the swap when the deadline passes without the replacement
    /// attaching (checked with the injected `now`, so tests need no sleep).
    pub(crate) fn poll(&mut self, now: Instant) {
        let (deadline, reason) = match &self.phase {
            Phase::Idle => return,
            Phase::Preparing { deadline, .. } => (*deadline, LaunchKind::Swap.timed_out()),
            Phase::Awaiting { deadline, kind, .. } => (*deadline, kind.timed_out()),
        };
        if now < deadline {
            return;
        }
        tracing::warn!(name: "daemon.pin_swap.timeout", target: "pin_swap", reason);
        self.abandon(reason);
    }

    /// The ctrl admission path consumed the pending token: the replacement
    /// Client attached (tray-pin-swap design D4). The swap completes — the
    /// replaced target is told to quit if it is still attached, and the
    /// machine returns to `Idle`. Any other phase, or a token that does not
    /// match the phase's, is ignored: the phase and the pending token stay
    /// untouched, so a stale admit cannot clear the token of a swap that is
    /// still live.
    pub(crate) fn on_swap_admitted(&mut self, token: &str, ctrl_clients: &ClientRegistry) {
        let matched_target = match &self.phase {
            Phase::Awaiting {
                target,
                token: pending,
                ..
            } if *pending == *token => Some(*target),
            _ => None,
        };
        let Some(target) = matched_target else {
            return;
        };
        self.pending_token.clear();
        self.phase = Phase::Idle;
        let clients = ctrl_clients.lock().unwrap();
        if let Some(target) = target
            && clients.has_client(target)
        {
            clients.send_to_client(target, &CtrlEvent::SwapQuit);
            tracing::info!(name: "daemon.pin_swap.replaced", target: "pin_swap", client = %target, "replacement client attached; SwapQuit sent to the replaced client");
        }
    }

    /// Spawns the replacement with the one-shot token in its environment and
    /// hands the child to a waiter thread that reports the exit status. On a
    /// spawn error the user is notified and the machine returns to `Idle`.
    fn spawn_replacement(
        &mut self,
        mut command: Command,
        target: Option<CtrlClientId>,
        deadline: Instant,
        kind: LaunchKind,
    ) {
        let token = uuid::Uuid::new_v4().to_string();
        command.env(SWAP_TOKEN_ENV, &token);
        match command.spawn() {
            Ok(mut child) => {
                let merged_tx = self.merged_tx.clone();
                let waiter_token = token.clone();
                std::thread::spawn(move || {
                    let success = child.wait().is_ok_and(|status| status.success());
                    let _ = merged_tx.send(DaemonEvent::PinSwapChildExited {
                        token: waiter_token,
                        success,
                    });
                });
                let spawned = kind.spawned_message();
                tracing::info!(
                    name: "daemon.pin_swap.spawned",
                    target: "pin_swap",
                    "{spawned}"
                );
                self.pending_token.publish(token.clone());
                self.phase = Phase::Awaiting {
                    target,
                    token,
                    deadline,
                    kind,
                };
            }
            Err(error) => {
                let program = command.get_program().to_string_lossy();
                let reason = kind.spawn_failed(&program, &error);
                let failed = kind.spawn_failed_message();
                tracing::warn!(
                    name: "daemon.pin_swap.spawn_failed",
                    target: "pin_swap",
                    program = %program,
                    error = %error,
                    "{failed}"
                );
                self.abandon(&reason);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{CtrlClients, CtrlConnectionRole, CtrlOutbound, CtrlTransport};
    use mbv_ctrl::CtrlAudiobookshelfCapabilities;

    type NotifyCalls = Arc<Mutex<Vec<String>>>;

    /// A swap-command hook that never runs: the returned command is only
    /// carried through the `Preparing` phase, never spawned (no real process
    /// in the tests that use this hook).
    fn fake_command() -> SwapCommandHook {
        Box::new(|_direction| Ok(Command::new("mbv-test-never-spawned")))
    }

    fn unresolvable_command() -> SwapCommandHook {
        Box::new(|_direction| Err("no terminal is configured".to_string()))
    }

    fn notify_recorder() -> (NotifyHook, NotifyCalls) {
        let calls: NotifyCalls = Arc::new(Mutex::new(Vec::new()));
        let hook: NotifyHook = Box::new({
            let calls = Arc::clone(&calls);
            move |message: &str| calls.lock().unwrap().push(message.to_string())
        });
        (hook, calls)
    }

    fn pending_token() -> Arc<PendingSwapToken> {
        Arc::new(PendingSwapToken::default())
    }

    fn notified(calls: &NotifyCalls) -> Vec<String> {
        calls.lock().unwrap().clone()
    }

    /// A registry with one local terminal Client attached, plus the receiver
    /// of that Client's event stream.
    fn registry_with_terminal_client() -> (ClientRegistry, mpsc::Receiver<CtrlOutbound>) {
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let clients = Arc::new(Mutex::new(CtrlClients::new(
            merged_tx,
            Arc::new(mbv_ctrl::AttachedClientsCell::default()),
            Arc::new(PendingSwapToken::default()),
        )));
        let (client_tx, client_rx) = mpsc::channel::<CtrlOutbound>();
        clients.lock().unwrap().connect_with_role(
            client_tx,
            CtrlTransport::Local,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            false,
            false,
            Some(SwapSurface::Terminal),
            CtrlConnectionRole::Client,
        );
        (clients, client_rx)
    }

    /// The first event the Client received, decoded back to a `CtrlEvent`.
    fn last_client_event(client_rx: &mpsc::Receiver<CtrlOutbound>) -> Option<CtrlEvent> {
        let event = client_rx.try_recv().ok()?;
        let CtrlOutbound::Event(json) = event else {
            return None;
        };
        serde_json::from_str(&json).ok()
    }

    /// Contract: spec pin-swap "New Client never attaches" — past the 10 s
    /// deadline the swap abandons, the user is notified that it timed out,
    /// and the machine is idle again so a later click can start a swap.
    #[test]
    fn deadline_abandons_and_notifies() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx, pending_token());
        let start = Instant::now();

        swap.start(start, &clients).unwrap();
        assert!(matches!(
            last_client_event(&client_rx),
            Some(CtrlEvent::SwapPrepare)
        ));

        swap.poll(start);
        assert!(matches!(swap.phase, Phase::Preparing { .. }));

        swap.poll(start + SWAP_DEADLINE);
        let messages = notified(&calls);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("timed out"));
        assert!(matches!(swap.phase, Phase::Idle));
        // Not busy any more: a later click starts a swap instead of being
        // refused.
        swap.start(start + SWAP_DEADLINE, &clients).unwrap();
    }

    /// Contract: spec pin-swap "Second click during a swap" — a request while
    /// the machine is not `Idle` starts nothing and returns the busy refusal.
    #[test]
    fn second_click_while_running_is_refused() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx, pending_token());
        let now = Instant::now();

        swap.start(now, &clients).unwrap();
        let busy = swap.start(now, &clients);

        assert_eq!(busy, Err(BUSY_MESSAGE.to_string()));
        // Still exactly one `SwapPrepare`: the refused click sent nothing.
        assert!(matches!(
            last_client_event(&client_rx),
            Some(CtrlEvent::SwapPrepare)
        ));
        assert!(client_rx.try_recv().is_err());
        assert_eq!(notified(&calls), Vec::<String>::new());
        assert!(matches!(swap.phase, Phase::Preparing { .. }));
    }

    /// Contract: spec pin-swap "Neither set" — an unresolvable command
    /// notifies the user with the reason, sends no `SwapPrepare`, and the
    /// machine stays idle.
    #[test]
    fn unresolvable_command_notifies_without_swapping() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut swap =
            PinSwapState::new(unresolvable_command(), notify, merged_tx, pending_token());
        let now = Instant::now();

        let result = swap.start(now, &clients);

        assert_eq!(result, Err("no terminal is configured".to_string()));
        assert_eq!(
            notified(&calls),
            vec!["no terminal is configured".to_string()]
        );
        assert!(client_rx.try_recv().is_err());
        assert!(matches!(swap.phase, Phase::Idle));
    }

    /// Contract: tray-pin-swap design D4 / task 3.4 — when the replacement
    /// Client attaches (the admission path consumed the pending token), the
    /// replaced target is told to quit and the machine returns to `Idle`, so
    /// a later click can start a new swap.
    #[test]
    fn swap_admitted_sends_swap_quit_to_the_target_and_returns_to_idle() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx, pending_token());
        let target = clients
            .lock()
            .unwrap()
            .newest_swap_client(SwapSurface::Terminal);
        swap.phase = Phase::Awaiting {
            target,
            token: "token-1".to_string(),
            deadline: Instant::now(),
            kind: LaunchKind::Swap,
        };

        swap.on_swap_admitted("token-1", &clients);

        assert!(matches!(
            last_client_event(&client_rx),
            Some(CtrlEvent::SwapQuit)
        ));
        assert!(matches!(swap.phase, Phase::Idle));
        assert_eq!(notified(&calls), Vec::<String>::new());
    }

    /// Contract: reviewer finding in tray-pin-swap tasks 3.3–3.4 — a stale
    /// or mismatched `SwapPrepared`-style admit must not disturb a swap that
    /// is still live: the phase and the pending token stay untouched, so the
    /// real replacement can still attach and complete the swap.
    #[test]
    fn mismatched_admit_leaves_the_live_swap_untouched() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let pending = pending_token();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx, Arc::clone(&pending));
        let target = clients
            .lock()
            .unwrap()
            .newest_swap_client(SwapSurface::Terminal);
        swap.phase = Phase::Awaiting {
            target,
            token: "token-live".to_string(),
            deadline: Instant::now(),
            kind: LaunchKind::Swap,
        };
        pending.publish("token-live".to_string());

        swap.on_swap_admitted("token-stale", &clients);

        assert!(
            matches!(&swap.phase, Phase::Awaiting { token, .. } if token == "token-live"),
            "stale admit disturbed the live swap"
        );
        assert!(pending.take_matching(Some("token-live")));
        assert!(client_rx.try_recv().is_err());
        assert_eq!(notified(&calls), Vec::<String>::new());
    }

    /// Contract: spec pin-swap "Open starts a terminal Client" — `open`
    /// asks the hook for the terminal command without reading the registry:
    /// when the command is unresolvable it notifies, sends no `SwapPrepare`,
    /// and stays idle. (Hermetic: the hook refuses, so no process spawns.)
    #[test]
    fn open_resolves_without_touching_attached_clients() {
        let (_clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let hook: SwapCommandHook = Box::new({
            let asked = Arc::clone(&asked);
            move |direction| {
                asked.lock().unwrap().push(direction);
                Err("no terminal is configured".to_string())
            }
        });
        let mut swap = PinSwapState::new(hook, notify, merged_tx, pending_token());
        let now = Instant::now();

        let result = swap.open(now);

        assert_eq!(result, Err("no terminal is configured".to_string()));
        assert_eq!(*asked.lock().unwrap(), vec![SwapDirection::Unpin]);
        assert!(
            client_rx.try_recv().is_err(),
            "open must not send SwapPrepare"
        );
        assert!(matches!(swap.phase, Phase::Idle));
        assert_eq!(
            notified(&calls),
            vec!["no terminal is configured".to_string()]
        );
    }

    /// Contract: spec pin-swap "Open starts a terminal Client" — the admit
    /// of an Open launch's one-shot token completes the launch without a
    /// `SwapQuit` (nothing is being replaced) and returns the machine to
    /// `Idle`.
    #[test]
    fn open_admit_completes_without_swap_quit() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let pending = pending_token();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx, Arc::clone(&pending));
        pending.publish("token-open".to_string());
        swap.phase = Phase::Awaiting {
            target: None,
            token: "token-open".to_string(),
            deadline: Instant::now(),
            kind: LaunchKind::Open,
        };

        swap.on_swap_admitted("token-open", &clients);

        assert!(
            client_rx.try_recv().is_err(),
            "admit must not send SwapQuit"
        );
        assert!(matches!(swap.phase, Phase::Idle));
        assert_eq!(notified(&calls), Vec::<String>::new());
    }

    /// Contract: spec pin-swap "Open shares the Pin swap launch rules" —
    /// past the deadline without the replacement attaching, the Open launch
    /// abandons with the Open timeout text and the machine returns to `Idle`.
    #[test]
    fn open_deadline_notifies_opening_timed_out() {
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx, pending_token());
        let start = Instant::now();
        swap.phase = Phase::Awaiting {
            target: None,
            token: "token-open".to_string(),
            deadline: start + SWAP_DEADLINE,
            kind: LaunchKind::Open,
        };

        swap.poll(start + SWAP_DEADLINE);

        assert_eq!(notified(&calls), vec!["opening mbv timed out".to_string()]);
        assert!(matches!(swap.phase, Phase::Idle));
    }
}

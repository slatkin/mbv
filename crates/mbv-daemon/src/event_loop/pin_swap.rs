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
use mbv_ctrl::CtrlEvent;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A swap must complete — the replacement Client attached — within this
/// window of the click (design D3). One deadline covers both the Prepare
/// phase and the wait for the replacement to attach.
const SWAP_DEADLINE: Duration = Duration::from_secs(10);

/// The environment variable that carries the one-shot swap token to the
/// replacement Client (design D2).
const SWAP_TOKEN_ENV: &str = "MBV_SWAP_TOKEN";

/// The reason a second start returns while a swap is in flight (spec
/// pin-swap "Second click during a swap").
const BUSY_MESSAGE: &str = "a panel swap is already running";

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
}

impl PinSwapState {
    pub(crate) fn new(
        swap_command: SwapCommandHook,
        notify: NotifyHook,
        merged_tx: mpsc::Sender<DaemonEvent>,
    ) -> Self {
        Self {
            phase: Phase::Idle,
            swap_command,
            notify,
            merged_tx,
        }
    }

    /// Starts a swap from an Owner action (design D3). The direction and the
    /// target come from the registry, not from the Tray label: a pinned
    /// Client attached means Unpin against that Client, otherwise Pin against
    /// the last-connected terminal Client — or no target at all, which
    /// spawns directly. `Err` means the click did nothing: the busy refusal,
    /// or an unresolvable command after the user was notified.
    pub(crate) fn start(
        &mut self,
        now: Instant,
        ctrl_clients: &ClientRegistry,
    ) -> Result<(), String> {
        if !matches!(self.phase, Phase::Idle) {
            return Err(BUSY_MESSAGE.to_string());
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
        let command = match (self.swap_command)(direction) {
            Ok(command) => command,
            Err(reason) => {
                drop(clients);
                tracing::warn!(name: "daemon.pin_swap.unresolvable", target: "pin_swap", reason = %reason, "panel swap command unresolvable");
                (self.notify)(&reason);
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
            None => self.spawn_replacement(command, None, deadline),
        }
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
                self.spawn_replacement(command, Some(target), deadline);
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
            (self.notify)(reason);
            self.phase = Phase::Idle;
        }
    }

    /// The child-waiter thread reports the replacement process's exit. Only
    /// a non-success exit while `Awaiting` abandons the swap: a terminal
    /// emulator that forks and exits 0 is normal and is ignored (design D5).
    pub(crate) fn on_child_exited(&mut self, token: &str, success: bool) {
        let Phase::Awaiting { token: pending, .. } = &self.phase else {
            return;
        };
        if pending != token || success {
            return;
        }
        let reason = "panel swap abandoned: the new client exited before attaching";
        tracing::warn!(name: "daemon.pin_swap.child_failed", target: "pin_swap", reason);
        (self.notify)(reason);
        self.phase = Phase::Idle;
    }

    /// Abandons the swap when the deadline passes without the replacement
    /// attaching (checked with the injected `now`, so tests need no sleep).
    pub(crate) fn poll(&mut self, now: Instant) {
        let deadline = match &self.phase {
            Phase::Idle => return,
            Phase::Preparing { deadline, .. } | Phase::Awaiting { deadline, .. } => *deadline,
        };
        if now < deadline {
            return;
        }
        let reason = "panel swap timed out; the old client stays attached";
        tracing::warn!(name: "daemon.pin_swap.timeout", target: "pin_swap", reason);
        (self.notify)(reason);
        self.phase = Phase::Idle;
    }

    /// Spawns the replacement with the one-shot token in its environment and
    /// hands the child to a waiter thread that reports the exit status. On a
    /// spawn error the user is notified and the machine returns to `Idle`.
    fn spawn_replacement(
        &mut self,
        mut command: Command,
        target: Option<CtrlClientId>,
        deadline: Instant,
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
                tracing::info!(name: "daemon.pin_swap.spawned", target: "pin_swap", "panel swap replacement client spawned");
                self.phase = Phase::Awaiting {
                    target,
                    token,
                    deadline,
                };
            }
            Err(error) => {
                let program = command.get_program().to_string_lossy();
                let reason = format!("panel swap could not start {program}: {error}");
                tracing::warn!(name: "daemon.pin_swap.spawn_failed", target: "pin_swap", program = %program, error = %error, "panel swap spawn failed");
                (self.notify)(&reason);
                self.phase = Phase::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{CtrlClients, CtrlOutbound, CtrlTransport};
    use mbv_ctrl::CtrlAudiobookshelfCapabilities;
    use std::sync::{Arc, Mutex};

    type NotifyCalls = Arc<Mutex<Vec<String>>>;

    /// A swap-command hook that never runs: the returned command is only
    /// carried through the `Preparing` phase, never spawned (no real process
    /// in any test).
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

    fn notified(calls: &NotifyCalls) -> Vec<String> {
        calls.lock().unwrap().clone()
    }

    /// A registry with one local terminal Client attached, plus the receiver
    /// of that Client's event stream.
    fn registry_with_terminal_client() -> (ClientRegistry, mpsc::Receiver<CtrlOutbound>) {
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let clients = Arc::new(Mutex::new(CtrlClients::new(
            merged_tx,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )));
        let (client_tx, client_rx) = mpsc::channel::<CtrlOutbound>();
        clients.lock().unwrap().connect(
            client_tx,
            CtrlTransport::Local,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            Some(SwapSurface::Terminal),
        );
        (clients, client_rx)
    }

    fn state(swap_command: SwapCommandHook) -> (PinSwapState, mpsc::Receiver<DaemonEvent>) {
        let (notify, _calls) = notify_recorder();
        let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();
        (
            PinSwapState::new(swap_command, notify, merged_tx),
            merged_rx,
        )
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
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx);
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
        assert!(swap.start(start + SWAP_DEADLINE, &clients).is_ok());
    }

    /// Contract: spec pin-swap "Second click during a swap" — a request while
    /// the machine is not `Idle` starts nothing and returns the busy refusal.
    #[test]
    fn second_click_while_running_is_refused() {
        let (clients, client_rx) = registry_with_terminal_client();
        let (notify, calls) = notify_recorder();
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut swap = PinSwapState::new(fake_command(), notify, merged_tx);
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
        assert!(notified(&calls).is_empty());
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
        let mut swap = PinSwapState::new(unresolvable_command(), notify, merged_tx);
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
}

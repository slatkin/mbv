//! Owner actions (tray-pin-swap design D7): one handler for every action
//! the Tray or an `mbv` CLI flag can request from the running Owner
//! process. The ctrl command path replies with the result; the transport
//! path (Tray) drops it.

use super::DaemonLoop;
use mbv_ctrl::OwnerAction;
use std::time::Instant;

/// Runs one Owner action. `Err` carries the user-facing reason: the ctrl
/// path replies `OwnerActionRefused { reason }`, the transport path drops
/// it.
pub(super) fn run(daemon: &mut DaemonLoop, action: OwnerAction) -> Result<(), String> {
    match action {
        OwnerAction::SwapPanel => {
            let now = Instant::now();
            daemon.pin_swap.start(now, &daemon.ctrl_clients)
        }
    }
}

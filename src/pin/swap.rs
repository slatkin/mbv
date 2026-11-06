//! The Pin-swap advertisement this process's Client connections send.

use std::sync::OnceLock;

use super::{is_pinned, swap_token};

/// Whether this Client process takes part in Pin swap: the TUI Client does —
/// it answers `SwapPrepare` and is a swap target — and the GUI Client never
/// does: it is neither a terminal Client nor a pinned panel, and the swap's
/// launch snapshot is a TUI shell query it cannot produce (design D9 of
/// `extract-client-core`). Claimed once at start-up, before the first Owner
/// connection, so every connection this process makes advertises alike.
static PIN_SWAP_PARTICIPANT: OnceLock<bool> = OnceLock::new();

/// Claim whether this process's Client connections take part in Pin swap.
/// Called once at start-up. The unclaimed default `true` is the TUI Client's
/// claim; the GUI Client claims `false` through the library's
/// [`crate::declare_gui_client`].
pub(crate) fn claim_pin_swap_participant(participates: bool) {
    let _ = PIN_SWAP_PARTICIPANT.set(participates);
}

/// Whether this process's Client connections take part in Pin swap. The
/// unclaimed default is the TUI Client's `true`.
fn pin_swap_participant() -> bool {
    *PIN_SWAP_PARTICIPANT.get_or_init(|| true)
}

/// The Pin-swap Hello advertisement for this process's Client connections
/// (tray-pin-swap design D2): the claimed panel state and swap token behind
/// the `pin-swap` capability for the TUI Client, `None` — a plain Client
/// Hello — for the GUI Client, so the Owner never picks it as a swap target.
pub(crate) fn pin_swap_hello() -> Option<mbv_remote_player::PinSwapHello> {
    pin_swap_participant().then(|| mbv_remote_player::PinSwapHello {
        pinned: is_pinned(),
        swap_token: swap_token(),
    })
}

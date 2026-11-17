//! What kind of Client this process is, and the claims its Owner-link
//! connections make in the ctrl Hello (gui-named-queues design D7).
//!
//! The kind is claimed once at start-up, before the first Owner connection, so
//! every connection this process makes advertises alike. The TUI Client is the
//! unclaimed default and keeps its behaviour; a GUI Client claims two facts:
//! a plain Hello that the Owner never picks as a Pin-swap target
//! (`extract-client-core` design D9), and `named-queues` on its link to the
//! local Owner, so that Owner publishes its Named queues to it.

use std::sync::OnceLock;

use mbv_remote_player::{ClientHelloClaims, DaemonEndpoint};

/// Whether this process is a GUI Client. The unclaimed default `false` is the
/// TUI Client's claim.
static GUI_CLIENT: OnceLock<bool> = OnceLock::new();

/// Claim this process for the GUI Client kind. Called once at start-up by
/// [`crate::declare_gui_client`], before the first Owner connection.
pub(crate) fn claim_gui_client() {
    let _ = GUI_CLIENT.set(true);
}

/// Whether this process is a GUI Client.
#[must_use]
pub(crate) fn is_gui_client() -> bool {
    GUI_CLIENT.get().copied().unwrap_or(false)
}

/// The Hello claims one connection to `endpoint` makes (gui-named-queues
/// design D7). The Pin-swap claim follows the Client kind this process
/// declared. `named-queues` is claimed on the local Owner link of a GUI
/// Client only: the GUI shows one Named queue at a time and needs the list and
/// the shown queue's snapshots, while the TUI Client shows the current queue by
/// its connection rule and only ever sees that one.
#[must_use]
pub(crate) fn client_hello_claims(endpoint: &DaemonEndpoint) -> ClientHelloClaims {
    ClientHelloClaims {
        pin_swap: crate::pin::pin_swap_hello(),
        named_queues: endpoint.is_local() && is_gui_client(),
    }
}

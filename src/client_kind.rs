//! What kind of Client a process runs, and the claims its Owner-link
//! connections make in the ctrl Hello (gui-named-queues design D7).
//!
//! A Client passes its kind at attach time, before its first Owner
//! connection, so every connection it makes advertises alike. The TUI Client
//! is the unclaimed default and keeps its behaviour; a GUI Client claims two
//! facts: a plain Hello that the Owner never picks as a Pin-swap target
//! (`extract-client-core` design D9), and `named-queues` on its link to the
//! local Owner, so that Owner publishes its Named queues to it.

use mbv_remote_player::{ClientHelloClaims, DaemonEndpoint};

/// Which Client a process runs. The kind decides the Hello claims every
/// Owner-link connection of the process sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientKind {
    /// The terminal Client. The default kind.
    #[default]
    Tui,
    /// The GUI Client.
    Gui,
}

/// The Hello claims one connection to `endpoint` makes (gui-named-queues
/// design D7). The Pin-swap claim follows this process's claimed Pin-swap
/// participation (`pin_swap_hello`, claimed at start-up).
/// `named-queues` is claimed on the local Owner link of a GUI Client only:
/// the GUI shows one Named queue at a time and needs the list and the shown
/// queue's snapshots, while the TUI Client shows the current queue by its
/// connection rule and only ever sees that one.
#[must_use]
pub(crate) fn client_hello_claims(
    kind: ClientKind,
    endpoint: &DaemonEndpoint,
) -> ClientHelloClaims {
    ClientHelloClaims {
        pin_swap: crate::pin::pin_swap_hello(),
        named_queues: endpoint.is_local() && matches!(kind, ClientKind::Gui),
    }
}

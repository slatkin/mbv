//! The bus-name side of publishing: the name to request and the claim/release
//! bookkeeping that the publisher loop keeps in step with the publish rule.
//!
//! Non-test builds only: a claim from a test process would take a real
//! session-bus name (issue #757).

use zbus::fdo::{RequestNameFlags, RequestNameReply};

/// The Client's instance bus name (design D1): the MPRIS Bus Name Policy asks
/// for a unique identifier that does not begin with a digit, which
/// `instance<pid>` satisfies.
pub(super) fn instance_name() -> String {
    format!("org.mpris.MediaPlayer2.mbv.instance{}", std::process::id())
}

/// Claims `name` for this connection (owner-mpris-fallback design D2/D4).
///
/// `ReplaceExisting | DoNotQueue` without `AllowReplacement`: the Owner takes
/// the fixed name from a Client of an older build that still holds it at
/// upgrade time, and no later claim can take the name from the Owner. A
/// Client's instance name carries its process id, so no other publisher holds
/// it.
async fn claim_name(conn: &zbus::Connection, name: &str) -> Result<RequestNameReply, zbus::Error> {
    conn.request_name_with_flags(
        name,
        RequestNameFlags::ReplaceExisting | RequestNameFlags::DoNotQueue,
    )
    .await
}

/// Whether this connection believes it holds its bus name. Both publishers
/// start unclaimed: the Owner's rule always holds, so it claims on the first
/// tick and never releases, while a Client claims and releases its instance
/// name as its rule turns.
#[derive(Default)]
pub(super) struct NameClaim {
    pub(super) held: bool,
    /// Set when a failed release left zbus's own registration for the name
    /// missing while the bus may still count this connection as its owner. The
    /// next release restores that registration before it releases again.
    stale: bool,
}

impl NameClaim {
    /// Keeps the claim in step with the publish rule, and answers whether the
    /// name was just claimed -- whose first publish reports the full state. A
    /// refused claim or release is logged and retried on the next tick.
    pub(super) async fn sync(
        &mut self,
        conn: &zbus::Connection,
        name: &str,
        publish: bool,
    ) -> bool {
        if publish && !self.held {
            return self.claim(conn, name).await;
        }
        if self.held && !publish {
            self.release(conn, name).await;
        }
        false
    }

    async fn claim(&mut self, conn: &zbus::Connection, name: &str) -> bool {
        match claim_name(conn, name).await {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {
                self.held = true;
                self.stale = false;
                true
            }
            Ok(reply) => {
                // zbus now carries a registration for the name, so a later
                // release has one to send.
                self.stale = false;
                tracing::debug!(
                    name: "mpris.name.not_claimed",
                    target: "mpris",
                    bus_name = %name,
                    reply = %reply,
                    "MPRIS name stays with its current owner; retrying next tick"
                );
                false
            }
            Err(e) => {
                tracing::debug!(
                    name: "mpris.name.claim_failed",
                    target: "mpris",
                    bus_name = %name,
                    error = %e,
                    "MPRIS name claim failed; retrying next tick"
                );
                false
            }
        }
    }

    /// Releases `name`, and keeps the claim releasable when the bus call fails.
    ///
    /// `Connection::release_name` drops zbus's own registration for the name
    /// *before* it sends `ReleaseName`, so a failed call leaves the name
    /// registered on the bus while zbus has forgotten it: the next
    /// `release_name` would then answer `Ok(false)` without contacting the bus
    /// at all. A failed release therefore keeps `held` set, and
    /// [`Self::restore`] puts the registration back.
    async fn release(&mut self, conn: &zbus::Connection, name: &str) {
        if self.stale {
            self.restore(conn, name).await;
            return;
        }
        match conn.release_name(name).await {
            Ok(_) => self.held = false,
            Err(e) => {
                tracing::debug!(
                    name: "mpris.name.release_failed",
                    target: "mpris",
                    bus_name = %name,
                    error = %e,
                    "MPRIS name release failed; retrying next tick"
                );
                self.stale = true;
                self.restore(conn, name).await;
            }
        }
    }

    /// Puts zbus's registration for `name` back after a failed release, and
    /// settles the claim from the bus's answer.
    ///
    /// The request does not steal: it asks for the name without
    /// `ReplaceExisting` or `AllowReplacement`, so it can only report whether
    /// this connection still owns it. `PrimaryOwner` and `AlreadyOwner` say it
    /// does, and a later tick releases again. Any other answer — or
    /// `Error::NameTaken`, which zbus returns for `Exists` — says another
    /// owner has the name, so `held` clears and the claim stops retrying. A
    /// request that fails without an ownership answer leaves the registration
    /// stale, and the next tick restores it again.
    async fn restore(&mut self, conn: &zbus::Connection, name: &str) {
        let still_owns = match conn
            .request_name_with_flags(name, RequestNameFlags::DoNotQueue.into())
            .await
        {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => true,
            Ok(reply) => {
                tracing::debug!(
                    name: "mpris.name.not_held",
                    target: "mpris",
                    bus_name = %name,
                    reply = %reply,
                    "MPRIS name is no longer owned here; releasing stopped"
                );
                false
            }
            // zbus reports the bus's `Exists` as `NameTaken`.
            Err(zbus::Error::NameTaken) => {
                tracing::debug!(
                    name: "mpris.name.not_held",
                    target: "mpris",
                    bus_name = %name,
                    "MPRIS name is no longer owned here; releasing stopped"
                );
                false
            }
            Err(e) => {
                tracing::debug!(
                    name: "mpris.name.restore_failed",
                    target: "mpris",
                    bus_name = %name,
                    error = %e,
                    "MPRIS name registration not restored; retrying next tick"
                );
                return;
            }
        };
        // The registration matches what the bus reports again.
        self.stale = false;
        if !still_owns {
            self.held = false;
        }
    }
}

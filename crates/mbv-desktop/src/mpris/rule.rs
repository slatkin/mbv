//! The Client publishing rule (design D3): whether a Client that controls a
//! remote Player owner holds its own MPRIS bus name.
//!
//! Kept pure and free of D-Bus so it is unit-tested without a bus: test builds
//! must never claim an MPRIS name on the real session bus (issue #757).

use mbv_ctrl::player::PlayerStatus;

/// Whether a Client publishes MPRIS under its instance name.
///
/// A Client publishes only when it controls a remote Player owner
/// (`endpoint_is_local` is false) and the local Owner is not playing: a playing
/// local Owner wins. A paused local Owner yields to the remote playback.
///
/// `None` -- no live link to the local Owner -- counts as not playing: the
/// Client cannot observe local playback, and the Owner publishes its own name
/// either way.
#[must_use]
pub fn client_publishes(endpoint_is_local: bool, local_status: Option<&PlayerStatus>) -> bool {
    !endpoint_is_local && !is_playing(local_status)
}

/// Whether the status reports playback that is running, not merely loaded.
fn is_playing(status: Option<&PlayerStatus>) -> bool {
    status.is_some_and(|status| status.active && !status.paused)
}

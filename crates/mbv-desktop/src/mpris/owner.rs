//! The local Owner process's MPRIS publisher (design D2).
//!
//! The Owner holds `org.mpris.MediaPlayer2.mbv` for as long as it runs, so the
//! name stays on the session bus whatever Clients attach or exit, with no
//! handoff. The packaged `mbvd` never reaches this entry point: it runs the
//! packaged daemon loop, whose Player-ready hook is a no-op (design D5).

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use mbv_ctrl::TransportCommand;
use mbv_ctrl::player::PlayerStatus;

use super::{MprisHandle, MprisSource, spawn_publisher};

/// The bus name the Owner holds for its lifetime, with no instance suffix: the
/// two publishers never contend for a name (design D1).
const OWNER_NAME: &str = "org.mpris.MediaPlayer2.mbv";

/// Starts the Owner's MPRIS publisher.
///
/// `status` is the Owner's live playback status (Cast and Emby Session outputs
/// write their presented status into the same cell), `transport_tx` its
/// transport channel, and `art_path` the image-cache lookup behind
/// `mpris:artUrl`.
///
/// The source never changes, so this publisher has no rebind: the Owner is the
/// only Player owner it serves. Nothing here returns a handle; the publisher
/// thread ends with the process.
///
/// Failure is not fatal (design D6): an unavailable session bus is logged and
/// parks the thread, and a claim the bus refuses is retried on the next tick.
///
/// # Panics
///
/// This call and the publisher thread panic if `status`'s mutex is poisoned,
/// like every other reader of that cell.
pub fn start_owner(
    status: Arc<Mutex<PlayerStatus>>,
    transport_tx: mpsc::Sender<TransportCommand>,
    art_path: fn(&str) -> Option<PathBuf>,
) {
    // The property getters read this snapshot, never the live cell: the poll
    // writes the snapshot every tick, so sharing the live cell would let a
    // poll that read the status before a Player update write the older value
    // back over it.
    let snapshot = Arc::new(Mutex::new(status.lock().unwrap().clone()));
    let source: MprisHandle = Arc::new(Mutex::new(MprisSource {
        status: Box::new(move || status.lock().unwrap().clone()),
        send: Box::new(move |transport| {
            if let Err(e) = transport_tx.send(transport) {
                tracing::debug!(
                    name: "mpris.transport.dropped",
                    target: "mpris",
                    error = %e,
                    "transport command dropped"
                );
            }
        }),
        disconnected: None,
        // The Owner publishes for as long as it runs; while it plays nothing
        // it reports `Stopped`, like any other publisher of this crate.
        publish: Box::new(|| true),
    }));
    spawn_publisher(OWNER_NAME.to_string(), source, snapshot, art_path);
}

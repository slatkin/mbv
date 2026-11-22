use super::ImageProtocols;
use ratatui_image::picker::Picker;
use ratatui_image::thread::ThreadProtocol;
use std::sync::mpsc;

/// A store whose resize worker has already exited: the response sender is
/// dropped at construction, the same disconnected channel a worker exit
/// produces.
fn store_with_dead_worker() -> ImageProtocols {
    let (register_tx, register_rx) = mpsc::channel();
    let (_, response_rx) = mpsc::channel();
    drop(register_rx);
    ImageProtocols::with_worker(register_tx, response_rx)
}

fn halfblock_protocol() -> ThreadProtocol {
    let (tx, _rx) = mpsc::channel();
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        1,
        image::Rgba([0, 0, 0, 255]),
    ));
    ThreadProtocol::new(tx, Some(Picker::halfblocks().new_resize_protocol(img)))
}

/// Issue #895 (M-PANIC-CONTINUATION): after the resize worker exits (a
/// caught panic closes the shared response channel), the respawn must
/// install a live worker and drop every cached protocol — the old
/// worker took their per-key request receivers with it, so each entry
/// re-encodes and re-registers on its next paint.
#[test]
fn respawn_replaces_a_dead_worker_and_clears_cached_protocols() {
    let mut protocols = store_with_dead_worker();
    protocols.insert_protocol("base:P", "halfblock", halfblock_protocol());
    assert!(matches!(
        protocols.try_recv_resize_response(),
        Err(mpsc::TryRecvError::Disconnected)
    ));

    protocols.respawn_resize_worker();

    assert!(
        matches!(
            protocols.try_recv_resize_response(),
            Err(mpsc::TryRecvError::Empty)
        ),
        "the respawned worker's response channel must be live"
    );
    assert!(
        !protocols.has_any("base:P"),
        "cached protocols must be dropped so they re-register with the new worker"
    );
}

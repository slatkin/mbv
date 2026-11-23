use super::ImageProtocols;
use mbv_images::CachedImage;
use mbv_images::cache::ImageCache;
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

fn image_cache() -> ImageCache {
    let (card_image_tx, card_image_rx) = mpsc::channel();
    ImageCache::new(4, None, true, card_image_tx, card_image_rx)
}

fn entry() -> CachedImage {
    CachedImage {
        img: None,
        source: None,
        cover_box: None,
        applied_logo_key: None,
    }
}

/// Issue #895 (M-PANIC-CONTINUATION): after the resize worker exits (a
/// caught panic closes the shared response channel), the respawn must
/// install a live worker and drop every cached protocol — the old
/// worker took their per-key request receivers with it, so each entry
/// re-encodes and re-registers on its next paint.
#[test]
fn respawn_replaces_a_dead_worker_and_clears_cached_protocols() {
    let mut cache = image_cache();
    let mut protocols = store_with_dead_worker();
    cache.insert_image("base:P".into(), entry());
    protocols.insert_protocol(&cache, "base:P", "halfblock", halfblock_protocol());
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
        !protocols.has_any(&cache, "base:P"),
        "cached protocols must be dropped so they re-register with the new worker"
    );
}

/// Invariant 38: a stored protocol lives exactly as long as the cache entry
/// it was encoded from. A removed key, a re-fetched key, and a pruned store
/// all stop handing out the old encoding, so a painter can never draw the
/// previous entry's art under a key that now holds another entry.
#[test]
fn a_stored_protocol_dies_with_its_cache_entry() {
    let mut cache = image_cache();
    let mut protocols = store_with_dead_worker();
    cache.insert_image("base:P".into(), entry());
    protocols.insert_protocol(&cache, "base:P", "halfblock", halfblock_protocol());
    assert!(protocols.contains(&cache, "base:P", "halfblock"));

    cache.remove_image("base:P");

    assert!(
        !protocols.contains(&cache, "base:P", "halfblock"),
        "a removed entry's protocol must not be handed out"
    );

    cache.insert_image("base:P".into(), entry());
    assert!(
        !protocols.contains(&cache, "base:P", "halfblock"),
        "a re-fetched key must not serve the previous entry's encoding"
    );

    protocols.insert_protocol(&cache, "base:P", "halfblock", halfblock_protocol());
    cache.clear_images_and_loading();
    protocols.prune(&cache);

    assert!(
        !protocols.has_any(&cache, "base:P"),
        "pruning after the cache cleared an entry drops its protocol"
    );
}

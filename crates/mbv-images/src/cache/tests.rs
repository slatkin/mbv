use super::{
    CachedImage, INTERRUPTED_FETCH_RETRY_DELAY, ImageCache, ROW_IMAGE_CACHE_CAPACITY,
    ROW_IMAGE_FETCH_LIMIT,
};
use std::sync::mpsc;

fn cache(capacity: usize) -> ImageCache {
    let (card_image_tx, card_image_rx) = mpsc::channel();
    let (resize_register_tx, _) = mpsc::channel();
    let (_, resize_response_rx) = mpsc::channel();
    let mut cache = ImageCache::new(
        capacity,
        None,
        true,
        card_image_tx,
        card_image_rx,
        resize_register_tx,
        resize_response_rx,
    );
    cache.set_cache_capacity_for_test(capacity);
    cache
}

fn image() -> CachedImage {
    CachedImage {
        img: Some(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])),
        )),
        source: None,
        protocols: std::collections::HashMap::new(),
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
    let mut cache = cache(2);
    let mut entry = image();
    let (tx, _rx) = mpsc::channel();
    entry.protocols.insert(
        "halfblock",
        ratatui_image::thread::ThreadProtocol::new(
            tx,
            Some(
                ratatui_image::picker::Picker::halfblocks()
                    .new_resize_protocol(entry.img.clone().expect("test image")),
            ),
        ),
    );
    cache.insert_image("base:P".into(), entry);
    // The `cache` helper drops the response sender at construction —
    // the same disconnected channel a worker exit produces.
    assert!(matches!(
        cache.try_recv_resize_response(),
        Err(mpsc::TryRecvError::Disconnected)
    ));

    cache.respawn_resize_worker();

    assert!(
        matches!(
            cache.try_recv_resize_response(),
            Err(mpsc::TryRecvError::Empty)
        ),
        "the respawned worker's response channel must be live"
    );
    assert!(
        cache
            .image("base:P")
            .is_some_and(|entry| entry.protocols.is_empty()),
        "cached protocols must be dropped so they re-register with the new worker"
    );
}

fn fetch_req(key: &str) -> super::ImageFetchReq {
    super::ImageFetchReq {
        cache_key: key.into(),
        item_id: "item".into(),
        series_id: String::new(),
        types: Vec::new(),
        source: super::super::ImageSource::Emby,
        rendition: super::super::ImageRendition::Hero,
    }
}

/// User report 2026-10-30: three episodes that timed out at 05:57 showed
/// the no-image placeholder for the rest of the run. A key with no answer
/// keeps no entry, waits out the delay, and can then fetch again.
#[rstest::rstest]
#[case::before_the_delay(INTERRUPTED_FETCH_RETRY_DELAY, false)]
#[case::after_the_delay(std::time::Duration::ZERO, true)]
fn interrupted_fetch_writes_no_entry_and_retries_after_the_delay(
    #[case] delay: std::time::Duration,
    #[case] retries: bool,
) {
    let mut cache = cache(2);
    cache.interrupted_fetch_retry_delay = delay;
    cache.reserve_fetch(fetch_req("item:P"), 6);

    cache.fetch_interrupted("item:P".into());

    assert!(!cache.is_cached("item:P"));
    assert!(cache.is_fetch_interrupted("item:P"));
    assert_eq!(
        matches!(
            cache.reserve_fetch(fetch_req("item:P"), 6),
            super::FetchReservation::Start(_)
        ),
        retries
    );
}

/// Review a05a530ba: derived overlay variants share the cache budget
/// with base images — inserting a new variant replaces the old one
/// without displacing bases, and removing a base drops its variants.
#[test]
fn review_a05a530ba_derived_variants_replace_and_die_with_their_base() {
    let mut cache = cache(2);
    cache.complete_fetch("base:P".into(), image());
    cache.complete_fetch("other:P".into(), image());

    cache.insert_derived_image("base:P:t:8x4:old".into(), image());
    cache.insert_derived_image("base:P:t:8x4:new".into(), image());

    assert!(cache.is_cached("base:P"));
    assert!(cache.is_cached("other:P"));
    assert!(!cache.is_cached("base:P:t:8x4:old"));
    assert!(cache.is_cached("base:P:t:8x4:new"));

    cache.remove_image("base:P");

    assert!(!cache.is_cached("base:P"));
    assert!(!cache.is_cached("base:P:t:8x4:new"));
    assert!(cache.is_cached("other:P"));
}

fn row_req(index: usize) -> super::ImageFetchReq {
    fetch_req(&format!("row:item:{index}"))
}

fn row_reqs(count: usize) -> Vec<super::ImageFetchReq> {
    (0..count).map(row_req).collect()
}

fn insert_rows(cache: &mut ImageCache, count: usize) {
    for index in 0..count {
        cache.complete_fetch(row_req(index).cache_key, image());
    }
}

fn start_fetches(cache: &mut ImageCache, count: usize) {
    for _ in 0..count {
        cache.start_fetch();
    }
}

/// Design D8: row images have their own 512-entry budget. Filling it
/// evicts row entries only, so a Hero or queue-card image survives a long
/// scroll.
#[test]
fn row_inserts_never_evict_a_non_row_image() {
    let mut cache = cache(1);
    cache.complete_fetch("item:P".into(), image());

    insert_rows(&mut cache, ROW_IMAGE_CACHE_CAPACITY + 1);

    assert!(cache.is_cached("item:P"), "the non-row image survives");
    assert!(
        cache.is_cached(&row_req(ROW_IMAGE_CACHE_CAPACITY).cache_key),
        "the newest row image is kept"
    );
    assert!(
        !cache.is_cached(&row_req(0).cache_key),
        "the oldest row image at the row budget was evicted"
    );
}

/// A row image the window read last survives the budget that evicts the
/// older inserts: eviction follows the last read, not the insertion order
/// (the 2026-11-20 TV Library thumbnail reload).
#[test]
fn a_touched_row_image_outlives_older_untouched_ones() {
    let mut cache = cache(1);
    insert_rows(&mut cache, ROW_IMAGE_CACHE_CAPACITY);
    let oldest = row_req(0).cache_key;
    cache.touch_row_image(&oldest);

    cache.complete_fetch(row_req(ROW_IMAGE_CACHE_CAPACITY).cache_key, image());

    assert!(cache.is_cached(&oldest), "the read row stays");
    assert!(
        !cache.is_cached(&row_req(1).cache_key),
        "the least recently read row is evicted"
    );
}

/// Design D7: a replaced in-view set drops a row request that never
/// started and clears its loading mark, so its row fetches again when it
/// comes back into view.
#[test]
fn a_replaced_row_set_drops_an_unstarted_request_and_clears_its_loading_mark() {
    let mut cache = cache(2);
    let waiting = row_req(ROW_IMAGE_FETCH_LIMIT).cache_key;

    let started = cache.replace_pending_row_fetches(row_reqs(ROW_IMAGE_FETCH_LIMIT + 1));

    assert_eq!(started.len(), ROW_IMAGE_FETCH_LIMIT);
    assert!(cache.is_loading(&waiting));

    assert!(cache.replace_pending_row_fetches(Vec::new()).is_empty());

    assert!(
        !cache.is_loading(&waiting),
        "dropping the waiting request clears its loading mark"
    );
    let restarted = cache.replace_pending_row_fetches(row_reqs(ROW_IMAGE_FETCH_LIMIT + 1));
    assert_eq!(restarted.len(), 1, "the dropped row fetches again");
}

/// Design D7: the Hero/card queue drains before the row queue, and row
/// fetches stay below their own slot limit so a Hero always fits.
#[test]
fn a_waiting_hero_request_is_taken_before_a_waiting_row_request() {
    let mut cache = cache(2);
    start_fetches(&mut cache, ROW_IMAGE_FETCH_LIMIT);
    assert!(matches!(
        cache.reserve_fetch(fetch_req("item:P"), ROW_IMAGE_FETCH_LIMIT),
        super::FetchReservation::Queued
    ));
    assert!(
        cache
            .replace_pending_row_fetches(row_reqs(ROW_IMAGE_FETCH_LIMIT))
            .is_empty(),
        "rows wait while four fetches are in flight"
    );

    let first = cache
        .take_pending_fetch(6)
        .expect("the waiting Hero starts first");
    assert_eq!(first.cache_key, "item:P");
    assert!(
        cache.take_pending_fetch(6).is_none(),
        "no row starts while its slots are taken"
    );

    cache.fetch_interrupted("item:P".into());
    let row = cache
        .take_pending_fetch(6)
        .expect("a freed slot starts the first waiting row");
    assert!(row.cache_key.starts_with("row:"));
}

#[test]
fn reported_movie_primary_reservation_does_not_block_queue_backdrop_fetch() {
    // Regression: a Primary-only reservation under {id}:P must not pin the queue card's landscape bytes.
    use crate::ImageSource;
    use crate::cache::FetchReservation;

    let mut cache = cache(2);
    let primary = super::ImageFetchReq {
        cache_key: "item:P".into(),
        item_id: "item".into(),
        series_id: String::new(),
        types: vec!["Primary".into()],
        source: ImageSource::Emby,
        rendition: crate::ImageRendition::Hero,
    };
    let landscape = super::ImageFetchReq {
        cache_key: "item:QB".into(),
        item_id: "item".into(),
        series_id: String::new(),
        types: vec!["Backdrop".into(), "Primary".into()],
        source: ImageSource::Emby,
        rendition: crate::ImageRendition::Hero,
    };

    assert!(matches!(
        cache.reserve_card_image_fetch(primary, 2),
        FetchReservation::Start(_)
    ));
    assert!(matches!(
        cache.reserve_card_image_fetch(landscape, 2),
        FetchReservation::Start(_)
    ));
}

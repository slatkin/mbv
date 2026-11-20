//! Tests of the Library's in-view row art path (gui-delineated-rows design D7,
//! invariant 27): a `RowsInView` set queues nothing while navigation is in
//! flight, the idle pass fetches only the rows the set names, and an
//! interrupted row key is queued again once its retry time passes, while its
//! row is still in the stored set. The Emby runtime holds no client, so a
//! reservation stays visible and no request leaves the process; the retry
//! passes through the cache's own test hook instead of a sleep.

use super::*;
use crate::app::state::service_runtime::EmbyRuntime;

const LIB_ID: &str = "lib-movies";

/// One movie row declaring a `Thumb` tag: the row chain's first kind.
fn tagged_movie(id: &str, thumb: &str) -> EmbyItem {
    let mut item = make_item(id, "Movie");
    item.id = id.to_owned();
    item.image_tags.thumb = thumb.to_owned();
    item
}

/// The row cache key an Emby row with a `Thumb` tag answers to (design D6).
fn row_key(item: &EmbyItem) -> String {
    mbv_images::row::emby_row_request(item, false).cache_key
}

/// A stub `App` on a plain Emby library whose root list holds `items`, with the
/// image cache enabled and no Emby client: a reserved fetch stays visible.
fn app_with_rows(items: Vec<EmbyItem>) -> App {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.emby_runtime = EmbyRuntime::default();
    app.images
        .configure_protocol(Some("kitty".to_owned()), true);
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = LIB_ID.to_owned();
    library.collection_type = "movies".to_owned();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    app.gui_emby_roots
        .insert(0, crate::app::gui_library::GuiEmbyRootItems::Loaded(items));
    app
}

/// A past instant the navigation-idle gate reads as idle: no sleep.
fn idle_at() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(1))
        .expect("a past instant")
}

/// A set sent during navigation queues nothing; the idle pass then fetches only
/// the rows the set names, and the out-of-view row stays unfetched.
#[test]
fn a_set_sent_during_navigation_fetches_only_its_rows_once_idle() {
    let shown = tagged_movie("m-1", "t1");
    let hidden = tagged_movie("m-2", "t2");
    let mut app = app_with_rows(vec![shown.clone(), hidden.clone()]);

    app.apply_gui_library_command(GuiLibraryCommand::RowsInView {
        scope: GuiRowArtScope::Emby,
        ids: vec![shown.id.clone()],
    });
    assert!(
        !app.images.is_loading(&row_key(&shown)),
        "a set sent during navigation queues nothing"
    );

    app.last_nav_at = idle_at();
    app.gui_queue_in_view_row_fetches();

    assert!(
        app.images.is_loading(&row_key(&shown)),
        "the in-view row's fetch is queued once navigation is idle"
    );
    assert!(
        !app.images.is_loading(&row_key(&hidden)),
        "a row outside the set is never fetched"
    );
}

/// An interrupted in-view row key is queued again when its retry time passes
/// (invariant 27), and only while the key is still in the stored set.
#[test]
fn an_interrupted_in_view_key_is_queued_again_only_while_it_stays_in_the_set() {
    let row = tagged_movie("m-1", "t1");
    let mut app = app_with_rows(vec![row.clone()]);
    let key = row_key(&row);
    app.last_nav_at = idle_at();
    app.apply_gui_library_command(GuiLibraryCommand::RowsInView {
        scope: GuiRowArtScope::Emby,
        ids: vec![row.id.clone()],
    });
    app.images.fetch_interrupted(key.clone());
    let checked = Instant::now();

    app.last_nav_at = idle_at();
    app.gui_queue_in_view_row_fetches();
    assert!(
        app.gui_art_retry_due(checked, checked + Duration::from_secs(3600)),
        "the interrupted key holds a retry time"
    );
    assert!(
        !app.images.is_loading(&key),
        "a key still waiting out its retry is not fetched again"
    );

    app.images.pass_interrupted_fetch_retries_for_test();
    app.last_nav_at = idle_at();
    app.gui_queue_in_view_row_fetches();
    assert!(
        app.images.is_loading(&key),
        "the passed retry queues the in-view key again"
    );

    app.apply_gui_library_command(GuiLibraryCommand::RowsInView {
        scope: GuiRowArtScope::Emby,
        ids: Vec::new(),
    });
    app.images.fetch_interrupted(key.clone());
    app.images.pass_interrupted_fetch_retries_for_test();
    app.last_nav_at = idle_at();
    app.gui_queue_in_view_row_fetches();
    assert!(
        !app.images.is_loading(&key),
        "a key whose row left the set is not queued again"
    );
}

/// A row whose artwork policy declares no source makes no request, whatever
/// the set names: the Emby row with no declared image and no series.
#[test]
fn an_in_view_row_without_a_source_queues_nothing() {
    let mut row = make_item("Plain", "BoxSet");
    row.id = "m-plain".to_owned();
    let mut app = app_with_rows(vec![row.clone()]);
    app.last_nav_at = idle_at();

    app.apply_gui_library_command(GuiLibraryCommand::RowsInView {
        scope: GuiRowArtScope::Emby,
        ids: vec![row.id.clone(), "absent".to_owned()],
    });

    assert_eq!(
        app.images.card_image_fetch_calls(),
        0,
        "a row with no declared image and an absent id reserve no fetch"
    );
}

/// The stored set is per list, and only the active destination's list queues:
/// a set of another list fetches nothing until its own destination is open.
#[test]
fn a_set_of_another_list_queues_nothing() {
    let row = tagged_movie("m-1", "t1");
    let mut app = app_with_rows(vec![row.clone()]);
    app.last_nav_at = idle_at();

    app.apply_gui_library_command(GuiLibraryCommand::RowsInView {
        scope: GuiRowArtScope::Feeds,
        ids: vec![row.id.clone()],
    });
    app.last_nav_at = idle_at();
    app.gui_queue_in_view_row_fetches();

    assert!(
        !app.images.is_loading(&row_key(&row)),
        "the Emby row is not in the Feeds list's set"
    );
}

/// A cached image of one pixel, enough for the row budget to count it.
fn cached_pixel() -> mbv_images::CachedImage {
    mbv_images::CachedImage {
        img: Some(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])),
        )),
        source: None,
        protocols: std::collections::HashMap::new(),
        cover_box: None,
        applied_logo_key: None,
    }
}

/// The 2026-11-20 TV Library thumbnail reload: a `RowsInView` set marks its
/// cached rows as read, so the row budget evicts a row the window scrolled past
/// before the row it still paints, even when the painted row was cached first.
#[test]
fn a_set_keeps_its_cached_rows_when_the_row_budget_overflows() {
    let painted = tagged_movie("m-1", "t1");
    let mut app = app_with_rows(vec![painted.clone()]);
    let key = row_key(&painted);
    app.images.complete_fetch(key.clone(), cached_pixel());
    for index in 0..511 {
        app.images
            .complete_fetch(format!("row:filler:{index}"), cached_pixel());
    }

    app.apply_gui_library_command(GuiLibraryCommand::RowsInView {
        scope: GuiRowArtScope::Emby,
        ids: vec![painted.id.clone()],
    });
    app.images
        .complete_fetch("row:filler:overflow".to_owned(), cached_pixel());

    assert!(app.images.is_cached(&key), "the in-view row stays cached");
    assert!(
        !app.images.is_cached("row:filler:0"),
        "the oldest untouched row is evicted instead"
    );
}

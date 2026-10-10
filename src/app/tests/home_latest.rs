use crate::app::shell::Model;
use crate::app::tests::make_app_stub;
use mbv_components::library_panel::LibraryContentOwner;
use mbv_config::{HomeSelectorKey, SelectorIdentity};
use mbv_queue::QueueItem;

#[test]
fn legacy_home_section_preference_falls_back_to_continue_watching() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "home_section": "abs:book-lib" }).to_string(),
    )
    .expect("write legacy prefs");

    let model = Model::new(make_app_stub());
    let (selector, _) = model.home_owner_shared().unwrap().launch_snapshot();
    assert_eq!(
        selector,
        Some(SelectorIdentity::Home {
            key: HomeSelectorKey::Continue,
        })
    );
}

// ── Destination Latest markers read Home's data (row 7.2) ─────
//
// Every marker below is driven the way the shell sees the data this run:
// a Home `Recently added` fetch event stored by `accept_home_recently
// added`, never a destination snapshot.

/// Opens the launch window `previous_age_secs` before the stub launch so
/// markers have an interval to fire in (a first launch never marks).
fn open_launch_window(model: &mut Model, previous_age_secs: u64) {
    let current = model.app.home_latest_launch_window.current;
    model.app.home_latest_launch_window.previous = Some(current - previous_age_secs);
}

fn date_added_iso(secs: i64) -> String {
    use time::format_description::well_known::Iso8601;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .expect("in-range timestamp")
        .format(&Iso8601::DEFAULT)
        .expect("ISO-8601 formatting always succeeds")
}

fn movie_dated(id: &str, added_secs: i64) -> QueueItem {
    let mut item = mbv_emby_model::test_support::make_item("Movie", "Movie");
    item.id = id.into();
    item.date_added = date_added_iso(added_secs);
    QueueItem::Emby(Box::new(item))
}

fn feed_entry(pub_date_secs: u64) -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: "entry".into(),
        title: "Entry".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: Some(pub_date_secs),
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

/// Spec scenario "Unvisited Emby library marks at launch"
/// (`destination-latest-modes`): an Emby library that has not been opened
/// this run — no destination Latest snapshot was ever fetched for it —
/// shows the marker from its Home fetch alone.
#[test]
fn an_unvisited_emby_library_marks_from_the_home_fetch_alone() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(make_app_stub());
    open_launch_window(&mut model, 2 * 60 * 60);
    let current = model.app.home_latest_launch_window.current;
    let source = crate::app::DestinationLatestSource::Emby("movies-lib".into());

    model.accept_home_recently_added(
        source.clone(),
        model.app.emby_runtime.generation(),
        vec![movie_dated("movie", i64::try_from(current - 60).unwrap())],
    );

    assert!(
        model.tv_latest_snapshots.is_empty(),
        "the library was never opened, so no destination Latest list was fetched"
    );
    assert!(model.destination_latest_marker(&source));
}

/// Spec scenario "Visiting clears marker across refresh"
/// (`destination-latest-modes`): selecting Latest acknowledges the source,
/// and the marker stays cleared for the rest of the run.
#[test]
fn acknowledging_the_source_clears_its_marker() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(make_app_stub());
    open_launch_window(&mut model, 2 * 60 * 60);
    let current = model.app.home_latest_launch_window.current;
    let source = crate::app::DestinationLatestSource::Emby("movies-lib".into());
    model.accept_home_recently_added(
        source.clone(),
        model.app.emby_runtime.generation(),
        vec![movie_dated("movie", i64::try_from(current - 60).unwrap())],
    );
    assert!(model.destination_latest_marker(&source));

    model.record_home_latest_acknowledgement(source.clone());

    assert!(!model.destination_latest_marker(&source));
}

/// Spec scenario "Window older than fourteen days"
/// (`destination-latest-modes`): an item 20 days old is outside the 14-day
/// cutoff, so it is never stored as a Home row and never marks.
#[test]
fn an_item_older_than_the_14_day_cutoff_does_not_mark() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(make_app_stub());
    open_launch_window(&mut model, 30 * 24 * 60 * 60);
    let current = model.app.home_latest_launch_window.current;
    let source = crate::app::DestinationLatestSource::Emby("movies-lib".into());

    model.accept_home_recently_added(
        source.clone(),
        model.app.emby_runtime.generation(),
        vec![movie_dated(
            "movie",
            i64::try_from(current - 20 * 24 * 60 * 60).unwrap(),
        )],
    );

    assert!(!model.destination_latest_marker(&source));
}

/// An item dated after the launch instant never marks during that run
/// (`destination-latest-modes`: "Items dated after the launch instant SHALL
/// NOT mark during that run").
#[test]
fn an_item_dated_after_the_launch_instant_does_not_mark() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(make_app_stub());
    open_launch_window(&mut model, 2 * 60 * 60);
    let current = model.app.home_latest_launch_window.current;
    let source = crate::app::DestinationLatestSource::Emby("movies-lib".into());

    model.accept_home_recently_added(
        source.clone(),
        model.app.emby_runtime.generation(),
        vec![movie_dated(
            "movie",
            i64::try_from(current + 60 * 60).unwrap(),
        )],
    );

    assert!(!model.destination_latest_marker(&source));
}

/// A podcast library's marker comes from its Home `Recently added` rows,
/// not from the destination shelf cache (spec `destination-latest-modes`, "Latest markers follow their
/// destination pills"): the fetch that stored
/// the rows is Home's, and the Latest list itself may not have loaded.
#[test]
fn a_podcast_library_marks_from_its_home_recently_added_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(make_app_stub());
    open_launch_window(&mut model, 2 * 60 * 60);
    let current = model.app.home_latest_launch_window.current;
    let source = crate::app::DestinationLatestSource::Audiobookshelf("pod-lib".into());
    let queue_episode = mbv_queue::AudiobookshelfQueueItem {
        library_item_id: "pod-lib".into(),
        episode_id: "episode".into(),
        title: "Episode".into(),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: Some(current - 60),
        is_finished: false,
        cover_path: None,
    };

    model.accept_home_recently_added(
        source.clone(),
        model.app.audiobookshelf_runtime.generation(),
        vec![QueueItem::Audiobookshelf(
            mbv_queue::AudiobookshelfItem::Episode(queue_episode),
        )],
    );

    assert!(model.destination_latest_marker(&source));
}

/// Feeds has no store entry: its marker derives from the feed entries the
/// same way the Home rows do — inside the launch window and inside the
/// 14-day cutoff.
#[test]
fn a_feed_entry_inside_the_launch_window_marks_the_feeds_destination() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(make_app_stub());
    open_launch_window(&mut model, 2 * 60 * 60);
    let current = model.app.home_latest_launch_window.current;
    model.app.feed_tab.all_entries = vec![feed_entry(current - 60)];

    assert!(model.destination_latest_marker(&crate::app::DestinationLatestSource::Feeds));
}

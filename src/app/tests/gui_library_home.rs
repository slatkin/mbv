//! Tests of the GUI Library's Home projection (add-gui-library-panel design
//! D12, row 6.2, the USER-APPROVED GUI Home design): the flat list's two
//! sections as heading rows with the TUI flat list's rules, the stable ids
//! the Home commands resolve (`App::home_stable_target`), and the selected
//! row's Hero with the first item row as the Hero default. The projection
//! reads the seeded snapshot and store alone, so no Service, worker, or
//! mock transport runs.

use super::*;
use crate::config::TestStateDirGuard;
use mbv_config::FeedSubscription;
use mbv_queue::{QueueItem, QueueItemContentId};

/// One Continue Watching row's Emby item, `position_ticks` of resume.
fn continue_item(id: &str, name: &str, position_ticks: i64, runtime_ticks: i64) -> EmbyItem {
    let mut item = make_item(name, "Movie");
    item.id = id.to_string();
    item.playback_position_ticks = position_ticks;
    item.runtime_ticks = runtime_ticks;
    item
}

/// One `Recently added` row's Audiobookshelf podcast episode, one hour old.
fn recent_episode(episode_id: &str, pub_date_secs: u64) -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "pod-lib".into(),
            episode_id: episode_id.into(),
            title: "Episode".into(),
            show_title: Some("The Show".into()),
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: Some(pub_date_secs),
            is_finished: false,
            cover_path: None,
        },
    ))
}

/// One `Recently added` row's feed entry, one hour old, from the
/// subscription the entry's feed id names.
fn recent_feed_entry(guid: &str, pub_date_secs: u64) -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: guid.into(),
        title: "Entry".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: Some(pub_date_secs),
        feed_kind: None,
        feed_id: Some("https://feed.example/rss".into()),
        position_ticks: 0,
        played: false,
    }
}

/// The merged store the projection reads: the Audiobookshelf source's
/// episode and the Feeds subscription's entry, both inside the 14-day
/// cutoff at seeding time (`SystemTime::now`-relative, as the store
/// accepts at land time).
fn seed_recently_added(app: &mut App) {
    let now = mbv_render::screens::feeds_model::current_time_secs();
    app.home_recently_added.insert(
        DestinationLatestSource::Audiobookshelf("pod-lib".to_string()),
        vec![recent_episode("recent-episode", now - 3600)],
    );
    app.feed_tab.subscriptions.push(FeedSubscription {
        name: "Tone Feed".into(),
        url: "https://feed.example/rss".into(),
        kind: mbv_queue::FeedKind::Audio,
    });
    app.feed_tab.all_entries = vec![recent_feed_entry("feed-entry", now - 2 * 3600)];
}

/// The stub `App` with `items` as the Continue Watching snapshot, Home the
/// active destination, and no store rows.
fn home_app(items: Vec<EmbyItem>) -> App {
    let mut app = make_app_stub();
    app.seed_home_content_for_test(items);
    app.tab = TabSelection::Home;
    app
}

/// The item rows between the headings, in order.
fn item_rows(rows: &[GuiHomeRow]) -> usize {
    rows.iter()
        .filter(|row| matches!(row, GuiHomeRow::Item(_)))
        .count()
}

/// One item row of the projection's `row`, or a panic on the structural rows.
fn item_of(row: &GuiHomeRow) -> &crate::app::gui_library::GuiHomeItemRow {
    match row {
        GuiHomeRow::Item(item) => item,
        _ => panic!("an item row"),
    }
}

/// One item row's state of the projection's `row`.
fn row_state(row: &GuiHomeRow) -> &GuiHomeRowState {
    &item_of(row).state
}

/// One item row's label pair `(primary, secondary)` of the projection's
/// `row`.
fn row_labels(row: &GuiHomeRow) -> (&str, Option<&str>) {
    let item = item_of(row);
    (item.title.as_str(), item.subtitle.as_deref())
}

#[test]
fn the_seeded_sections_project_as_heading_rows_in_order() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = home_app(vec![
        continue_item(
            "cw-first",
            "Movie One",
            600 * mbv_emby_model::TICKS_PER_SECOND,
            1200 * mbv_emby_model::TICKS_PER_SECOND,
        ),
        continue_item("cw-second", "Movie Two", 0, 0),
    ]);
    seed_recently_added(&mut app);

    let projection = app.gui_home_projection();

    let headings: Vec<String> = projection
        .rows
        .iter()
        .filter_map(|row| match row {
            GuiHomeRow::Heading { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec!["Continue watching", "Recently added"],
        "both sections paint as heading rows, Continue watching first"
    );
    let spacer = projection
        .rows
        .iter()
        .filter(|row| matches!(row, GuiHomeRow::Spacer))
        .count();
    assert_eq!(
        spacer, 1,
        "the spacer paints only between two non-empty sections"
    );
    assert_eq!(
        item_rows(&projection.rows),
        4,
        "two Continue rows, then the two Recently added rows"
    );

    // The stable ids the Home commands resolve, over the identity the form
    // `App::home_stable_target` maps: Continue rows in snapshot order; the
    // Audiobookshelf and Feeds rows over their content ids.
    let ids: Vec<&GuiHomeRowId> = projection
        .rows
        .iter()
        .filter_map(|row| match row {
            GuiHomeRow::Item(item) => Some(&item.id),
            _ => None,
        })
        .collect();
    assert_eq!(ids[0], &GuiHomeRowId::Continue("cw-first".to_string()));
    assert_eq!(ids[1], &GuiHomeRowId::Continue("cw-second".to_string()));
    assert_eq!(
        ids[2],
        &GuiHomeRowId::Recent(QueueItemContentId::Audiobookshelf {
            library_item_id: "pod-lib".into(),
            episode_id: "recent-episode".into(),
        })
    );
    assert_eq!(
        ids[3],
        &GuiHomeRowId::Recent(QueueItemContentId::Feed("feed-entry".into()))
    );

    // The split title parts and the resume progress, as the TUI's flat list
    // paints them: the show's name leads the episode row, and a resume
    // position carries the row's progress.
    assert_eq!(row_labels(&projection.rows[1]), ("Movie One", None));
    assert_eq!(
        row_labels(&projection.rows[5]),
        ("The Show", Some("Episode"))
    );
    assert_eq!(
        row_state(&projection.rows[1]),
        &GuiHomeRowState::Active { progress: Some(50) },
        "the resume position carries the row's progress"
    );
    assert_eq!(row_state(&projection.rows[6]), &GuiHomeRowState::Ordinary);

    // Home's Hero default (row 6.2): the first item row, with its shared
    // hero facts.
    assert_eq!(
        projection.selected,
        Some(GuiHomeRowId::Continue("cw-first".to_string())),
        "the first item row is the Hero default"
    );
    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(hero.title, "Movie One");
    assert_eq!(hero.duration_row, Some(0), "the duration meta row leads");
    assert_eq!(
        hero.progress_row, None,
        "the shared Emby hero producer fills none"
    );
    assert!(hero.overview.is_none(), "the stub item carries no overview");
}

#[test]
fn an_empty_home_projects_no_rows_and_no_hero() {
    let _guard = TestStateDirGuard::new();
    let mut app = home_app(Vec::new());

    let projection = app.gui_home_projection();

    assert!(projection.rows.is_empty(), "no rows without sections");
    assert_eq!(projection.selected, None);
    assert_eq!(projection.hero, None);
}

#[test]
fn a_stale_selection_falls_back_to_the_first_item_row() {
    let _guard = TestStateDirGuard::new();
    let mut app = home_app(vec![
        continue_item("cw-first", "Movie One", 0, 0),
        continue_item("cw-second", "Movie Two", 0, 0),
    ]);
    app.gui_home_selection = Some(GuiHomeRowId::Recent(QueueItemContentId::Feed(
        "gone".into(),
    )));

    let projection = app.gui_home_projection();

    assert_eq!(
        projection.selected,
        Some(GuiHomeRowId::Continue("cw-first".to_string())),
        "the Hero default returns when the selection's row has left the list"
    );
}

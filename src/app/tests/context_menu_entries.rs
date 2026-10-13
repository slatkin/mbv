//! The standard action set for Emby context menus (context-menu spec,
//! "Library lists share one standard action set", the Played label, and the
//! music rule). Standard-media-context-menus tasks 2.1-2.3.
use super::podcast::add_emby_movie_library;
use super::*;
use crate::app::state::context_menu_capabilities::{ItemCapabilities, emby_item_capabilities};
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_msg::{Msg, ShellRequest};
use rstest::rstest;

fn emby_library_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
    app
}

/// Open the menu for `item` on an Emby library tab and return the ordered
/// entry labels.
fn menu_labels_for(app: &mut App, item: EmbyItem) -> Vec<&'static str> {
    app.open_context_menu_for(item);
    pending_menu_labels(app)
}

/// The ordered entry labels of the context menu waiting in the pending overlay.
fn pending_menu_labels(app: &mut App) -> Vec<&'static str> {
    match app.pending_overlay.take() {
        Some(OverlayRequest::ContextMenu(menu)) => {
            menu.entries.iter().map(|entry| entry.label).collect()
        }
        other => panic!("expected a context menu overlay, got {other:?}"),
    }
}

fn leaf_item(name: &str, item_type: &str, media_type: &str, played: bool) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.media_type = media_type.into();
    item.played = played;
    item
}

fn folder_item(name: &str, item_type: &str, unplayed_children: u32) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.is_folder = true;
    item.unplayed_item_count = unplayed_children;
    item
}

/// One case per item shape: the standard action set for that shape (context
/// menu spec, "Library lists share one standard action set", the Played label,
/// and the music rule).
#[rstest]
#[case::unplayed_movie(
    leaf_item("movie", "Movie", "Video", false),
    &["Play", "Add to Queue", "Mark Played"],
)]
#[case::played_movie(
    leaf_item("movie", "Movie", "Video", true),
    &["Play", "Add to Queue", "Mark Unplayed"],
)]
#[case::season_with_unplayed_children(
    folder_item("Season 1", "Season", 5),
    &["Play All", "Shuffle", "Add to Queue", "Mark Played"],
)]
#[case::music_album(
    folder_item("Album", "MusicAlbum", 0),
    &["Play All", "Shuffle", "Add to Queue"],
)]
#[case::music_track(leaf_item("Track", "Audio", "Audio", false), &["Play", "Add to Queue"])]
fn emby_menu_follows_the_standard_action_set_for(
    #[case] item: EmbyItem,
    #[case] expected: &[&str],
) {
    let mut app = emby_library_app();
    assert_eq!(menu_labels_for(&mut app, item), expected);
}

#[test]
fn mixed_selection_with_a_music_track_drops_the_mark_entries() {
    let mut app = emby_library_app();
    let items = vec![
        leaf_item("movie", "Movie", "Video", false),
        leaf_item("Track", "Audio", "Audio", false),
    ];
    let capabilities: Vec<ItemCapabilities> = items.iter().map(emby_item_capabilities).collect();

    app.open_context_menu_for_selection(
        &items,
        None,
        PanelFocus::Library,
        capabilities,
        Vec::new(),
    );

    let labels = match app.pending_overlay.take() {
        Some(OverlayRequest::ContextMenu(menu)) => menu
            .entries
            .iter()
            .map(|entry| entry.label)
            .collect::<Vec<_>>(),
        other => panic!("expected a context menu overlay, got {other:?}"),
    };
    assert!(
        labels.iter().all(|label| !label.starts_with("Mark ")),
        "music in the selection must drop both mark entries: {labels:?}"
    );
}

/// A stub app whose owner advertises the `play-order` ctrl capability, so
/// the queue-row toggle entry is offered (queue-play-order design D6).
fn play_order_capable_app() -> App {
    let (mut remote, _events, _cmd) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compat = mbv_ctrl::CtrlCompatibility::current();
    compat.supports_play_order = true;
    remote.set_ctrl_compatibility_for_test(compat);
    let mut app = make_app_stub();
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app
}

fn queue_state(
    items: Vec<mbv_queue::QueueItem>,
    play_order: mbv_queue::PlayOrderMode,
) -> mbv_ctrl::UnifiedQueueStateData {
    mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: Some(1),
        slots: items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: index as u64 + 1,
                item,
            })
            .collect(),
        revision: 1,
        source: mbv_queue::QueueSource::Remote,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
        play_order,
    }
}

fn opened_menu_labels(app: &mut App) -> Vec<&'static str> {
    app.open_context_menu(false, None);
    pending_menu_labels(app)
}

/// queue-play-order spec, "The Queue list menu toggles Shuffle": an Emby
/// Queue row keeps its usual entries and the shuffle-order toggle is the
/// last entry, built as the opposite of the owner-reported order.
#[test]
fn an_emby_queue_row_menu_ends_with_the_play_order_toggle() {
    let mut app = play_order_capable_app();
    app.panel_focus = PanelFocus::Queue;
    app.local_view = QueueView::from_snapshot(&queue_state(
        vec![mbv_queue::QueueItem::Emby(Box::new(leaf_item(
            "queued movie",
            "Movie",
            "Video",
            false,
        )))],
        mbv_queue::PlayOrderMode::Sequential,
    ));

    let labels = opened_menu_labels(&mut app);

    assert_eq!(labels.last(), Some(&"Shuffle Play"));
}

/// queue-play-order spec, "The Queue list menu toggles Shuffle" (Non-Emby
/// Queue row): a Feed Queue row opens a menu that offers only the
/// shuffle-order toggle — here from Shuffle, so it reads "Stop Shuffle".
#[test]
fn a_feed_queue_row_menu_offers_only_the_play_order_toggle() {
    let entry = mbv_queue::FeedEntry {
        guid: "feed-episode".into(),
        title: "Feed episode".into(),
        enclosure_url: Some("https://example.test/episode.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(mbv_queue::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let mut app = play_order_capable_app();
    app.panel_focus = PanelFocus::Queue;
    app.local_view = QueueView::from_snapshot(&queue_state(
        vec![mbv_queue::QueueItem::Feed(entry)],
        mbv_queue::PlayOrderMode::Shuffle,
    ));

    let labels = opened_menu_labels(&mut app);

    assert_eq!(labels, vec!["Stop Shuffle"]);
}

/// queue-play-order spec, "The Queue list menu toggles Shuffle" (Library
/// row): no menu outside the Queue list offers the shuffle-order entry,
/// even when the owner advertises the capability.
#[test]
fn a_library_row_menu_has_no_play_order_toggle() {
    let mut app = play_order_capable_app();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let labels = menu_labels_for(&mut app, leaf_item("movie", "Movie", "Video", false));

    assert!(!labels.contains(&"Shuffle Play"));
    assert!(!labels.contains(&"Stop Shuffle"));
}

/// Context-menu spec, "Every painted library row resolves its own menu"
/// (standard-media-context-menus task 3.3, design D7): a `RowContextMenu`
/// whose Emby item is absent from `nav_stack.last().items` — a homevideos
/// feed row or a Latest row — still opens its menu, because the component
/// already resolved the painted item and the shell must not gate the menu
/// on the nav-level lookup.
#[test]
fn row_context_menu_for_an_item_outside_the_nav_level_still_opens() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    add_emby_movie_library(&mut app);
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;

    let mut latest_row = make_item("Latest row", "Movie");
    latest_row.id = "latest-row".into();
    let mut model = Model::new(app);
    model.handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Emby(vec![latest_row]),
            None,
        ))),
        &mut false,
        &mut false,
    );

    assert!(matches!(
        model.app.pending_overlay,
        Some(OverlayRequest::ContextMenu(_))
    ));
}

use super::queue_card_height_cap;
use crate::app::images::{CachedImage, QUEUE_CARD_PLACEHOLDER_KEY};
use crate::app::tests::{make_app_stub, make_item, make_items, make_session};
use crate::app::{App, BrowseLevel, LibraryTab, PanelFocus, QueueScope, TabSelection};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn make_queue_app(n: usize, cursor: usize) -> App {
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(n), cursor);
    app
}

fn make_drilled_library_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-focused".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![movie],
            total_count: 1,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app
}

fn render_card(app: &mut App) -> (u16, u16, bool) {
    // The projection owns every fetch (task 3.4); drive it the way the
    // queue projection does, then render from the projected state.
    app.refresh_queue_card_image();
    let backend = TestBackend::new(30, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut result = (0u16, 0u16, false);
    term.draw(|f| {
        result = app.render_queue_playback_slot(f, Rect::new(0, 0, 30, 20), false);
    })
    .unwrap();
    result
}

fn set_playback(app: &mut App, active_idx: usize, paused: bool) {
    let mut status = app.player.status.lock().unwrap();
    status.active = true;
    status.current_idx = active_idx;
    status.paused = paused;
}

fn fetch_triggered(app: &App, key: &str) -> bool {
    app.card_image_loading.contains(key) || app.card_image_states.contains_key(key)
}

fn make_direct_remote_app(
    local_items: Vec<mbv_core::api::EmbyItem>,
    remote_items: Vec<mbv_core::api::EmbyItem>,
) -> App {
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let mut app = App::new_remote(
        EmbyClient::new(Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.player_tab.set_items(local_items, 0);
    app
}

#[test]
fn active_playback_overrides_visible_cursor() {
    let mut app = make_queue_app(4, 0);
    app.image_protocol_enabled = true;
    set_playback(&mut app, 2, false);

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id2:P"));
    assert!(!fetch_triggered(&app, "id0:P"));
}

#[test]
fn paused_playback_keeps_active_priority() {
    let mut app = make_queue_app(3, 0);
    app.image_protocol_enabled = true;
    set_playback(&mut app, 1, true);

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id1:P"));
}

#[test]
fn stopped_playback_uses_visible_queue_selection() {
    let mut app = make_queue_app(4, 2);
    app.image_protocol_enabled = true;

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id2:P"));
    assert!(!fetch_triggered(&app, "id0:P"));
}

#[test]
fn stopped_playback_with_audiobookshelf_selection_fetches_cover_not_placeholder() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("https://books.example"),
    );
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();

    let episode = mbv_core::playback_queue::QueueItem::Audiobookshelf(
        mbv_core::playback_queue::AudiobookshelfQueueItem {
            library_item_id: "show-1".into(),
            episode_id: "ep-1".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        },
    );
    app.player_tab.queue.append(episode);

    render_card(&mut app);

    assert!(
        !app.card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY),
        "a selected Audiobookshelf queue item must not fall back to the bundled placeholder"
    );
    let cache_key = crate::app::images::audiobookshelf_cover_cache_key(
        "https://books.example",
        "show-1",
        app.current_protocol_suffix(),
    );
    assert!(
        app.card_image_loading.contains(&cache_key)
            || app.card_image_states.contains_key(&cache_key),
        "expected the Audiobookshelf cover fetch to be triggered"
    );
}

#[test]
fn stopped_local_selection_with_empty_remote_playback_queue_does_not_panic() {
    let local_items = make_items(3);
    let mut app = make_direct_remote_app(local_items, Vec::new());
    app.set_queue_scope(QueueScope::Local);
    app.player_tab.set_items(make_items(3), 2);
    app.image_protocol_enabled = true;
    app.player.status.lock().unwrap().active = false;

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id2:P"));
}

#[test]
fn active_remote_overrides_visible_local_queue() {
    let mut local_items = make_items(2);
    local_items[0].id = "local-0".into();
    local_items[1].id = "local-1".into();
    let mut remote_items = make_items(3);
    remote_items[0].id = "remote-0".into();
    remote_items[1].id = "remote-1".into();
    remote_items[2].id = "remote-2".into();
    let mut app = make_direct_remote_app(local_items, remote_items);
    app.set_queue_scope(QueueScope::Local);
    app.image_protocol_enabled = true;
    set_playback(&mut app, 1, false);

    render_card(&mut app);

    assert!(fetch_triggered(&app, "remote-1:P"));
    assert!(!fetch_triggered(&app, "local-0:P"));
}

#[test]
fn library_focus_and_depth_do_not_affect_card_source() {
    let mut app = make_drilled_library_app();
    app.player_tab.set_items(make_items(2), 1);
    app.image_protocol_enabled = true;

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id1:P"));
    assert!(!fetch_triggered(&app, "movie-focused:P"));
}

#[test]
fn stale_active_index_falls_back_to_visible_selection() {
    let mut app = make_queue_app(3, 1);
    app.image_protocol_enabled = true;
    set_playback(&mut app, 99, false);

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id1:P"));
}

#[test]
fn completed_no_art_uses_card_placeholder() {
    let mut app = make_queue_app(6, 2);
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.card_image_states
        .insert("id2:P".into(), CachedImage::empty());

    render_card(&mut app);

    assert!(app
        .card_image_states
        .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
    assert!(!app.card_image_loading.contains("id2:P"));
}

#[test]
fn completed_no_art_prefetch_centers_on_active_source() {
    let mut app = make_queue_app(6, 0);
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.card_image_states
        .insert("id3:P".into(), CachedImage::empty());
    set_playback(&mut app, 3, false);

    render_card(&mut app);

    assert!(app
        .card_image_states
        .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
    assert!(!app.card_image_loading.contains("id3:P"));
}

#[test]
fn prefetch_centers_on_active_source_while_playing() {
    let mut app = make_queue_app(6, 0);
    app.image_protocol_enabled = true;
    set_playback(&mut app, 3, false);

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id3:P"));
    assert!(fetch_triggered(&app, "id2:P"));
    assert!(fetch_triggered(&app, "id4:P"));
    assert!(fetch_triggered(&app, "id5:P"));
    assert!(!fetch_triggered(&app, "id0:P"));
}

#[test]
fn prefetch_centers_on_selected_source_when_stopped() {
    let mut app = make_queue_app(6, 2);
    app.image_protocol_enabled = true;

    render_card(&mut app);

    assert!(fetch_triggered(&app, "id2:P"));
    assert!(fetch_triggered(&app, "id1:P"));
    assert!(fetch_triggered(&app, "id3:P"));
    assert!(fetch_triggered(&app, "id4:P"));
    assert!(fetch_triggered(&app, "id5:P"));
    assert!(!fetch_triggered(&app, "id0:P"));
}

#[test]
fn selected_visualizer_reserves_geometry_without_fetching_artwork() {
    let mut app = make_queue_app(3, 2);
    app.image_protocol_enabled = true;
    app.visualizer_enabled = true;

    app.refresh_queue_card_image();
    let backend = TestBackend::new(30, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut geometry = (0, 0, false);
    term.draw(|f| {
        geometry = app.render_queue_playback_slot(f, Rect::new(0, 0, 30, 20), false);
    })
    .unwrap();
    let (h, w, loading) = geometry;

    assert!(
        h > 0 && w > 0,
        "the selected visualizer must reserve the card rectangle, got ({h},{w})"
    );
    assert!(!loading);
    assert!(
        !fetch_triggered(&app, "id2:P"),
        "selecting the visualizer must not fetch artwork"
    );
    assert!(!app
        .card_image_states
        .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
    assert_eq!(
        term.backend().buffer()[(0, 0)].style().bg,
        Some(crate::app::palette::SURFACE_CHROME),
        "an empty selected visualizer must still paint its reserved card"
    );
}

/// The visualizer background is the playback panel's own band value in every
/// focus state, so switching `v` (or moving panel focus) never repaints the
/// reserved slot in a fill the panel around it does not have.
#[test]
fn selected_visualizer_background_is_the_panel_band_under_either_focus() {
    for focus in [PanelFocus::Library, PanelFocus::Queue] {
        let mut app = make_queue_app(3, 2);
        app.panel_focus = focus;
        app.visualizer_enabled = true;
        app.visualizer_window.samples = vec![crate::app::infra::visualizer_worker::StereoSample {
            left: 1.0,
            right: 1.0,
        }];
        app.refresh_queue_card_image();

        let backend = TestBackend::new(30, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut reserved = (0u16, 0u16);
        term.draw(|f| {
            let (h, w, _) = app.render_queue_playback_slot(f, Rect::new(0, 0, 30, 20), false);
            reserved = (h, w);
        })
        .unwrap();
        let (h, w) = reserved;

        let buffer = term.backend().buffer();
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    buffer[(x, y)].style().bg,
                    Some(crate::app::palette::SURFACE_CHROME),
                    "visualizer cell ({x},{y}) must carry the panel band fill at {focus:?}"
                );
            }
        }
        // A zero-area reservation would make the loop below vacuous; the
        // reserved rect's exact size is its own test's claim.
        assert!(
            h > 0 && w > 0,
            "the visualizer must reserve a rect, got ({h},{w})"
        );
    }
}

#[test]
fn selected_visualizer_ignores_confirmed_missing_artwork() {
    let mut app = make_queue_app(6, 2);
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.card_image_states
        .insert("id2:P".into(), CachedImage::empty());
    app.visualizer_enabled = true;

    let (h, w, loading) = render_card(&mut app);

    assert!(h > 0 && w > 0);
    assert!(!loading);
    assert!(
        !app.card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY),
        "visualizer selection must not fall back to the bundled placeholder"
    );
}

/// A now-playing item whose fetch resolves empty must reserve the card
/// rect (the bundled placeholder) even with no prior card geometry, not
/// collapse to (0, 0) and hand its rows back to the queue panel (review
/// of tasks 3.1-3.4).
#[test]
fn now_playing_resolved_empty_art_reserves_the_placeholder_slot() {
    let mut app = make_queue_app(3, 0);
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());
    set_playback(&mut app, 1, false);
    // The now-playing item's fetch resolved empty; nothing has been
    // painted yet, so there is no prior card geometry.
    app.card_image_states
        .insert("id1:P".into(), CachedImage::empty());

    let (h, w, loading) = render_card(&mut app);

    assert!(
        h > 0 && w > 0,
        "a resolved-empty now-playing fetch must reserve the card rect, got ({h},{w})"
    );
    assert!(!loading);
    assert!(app
        .card_image_states
        .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));
}

#[test]
fn pending_artwork_keeps_loading_reservation() {
    let mut app = make_queue_app(3, 1);
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());

    let (h, w, loading) = render_card(&mut app);

    assert!(
        loading,
        "pending artwork must report the in-flight loading reservation"
    );
    assert!(
        h > 0 && w > 0,
        "the loading reservation must reserve card rows, got ({h},{w})"
    );
}

#[test]
fn images_off_artwork_and_visualizer_return_identical_geometry_without_fetch() {
    // Terminal images are disabled by default in the stub.
    let mut app = make_queue_app(3, 2);
    assert!(!app.images_enabled());

    let artwork = render_card(&mut app);
    assert!(!fetch_triggered(&app, "id2:P"));
    assert!(!app
        .card_image_states
        .contains_key(QUEUE_CARD_PLACEHOLDER_KEY));

    app.visualizer_enabled = true;
    let visualizer = render_card(&mut app);

    assert!(
        artwork.0 > 0 && artwork.1 > 0,
        "artwork selection must keep a blank reservation, got ({},{})",
        artwork.0,
        artwork.1
    );
    assert_eq!(
        artwork, visualizer,
        "images-off artwork and visualizer must keep the same fallback rectangle"
    );
    assert!(
        !fetch_triggered(&app, "id2:P"),
        "images-off must not fetch artwork"
    );
    assert!(app.card_image_states.is_empty());
}

/// A watched remote Session playing foreign content is active with no local
/// slot: the visual slot paints the bundled placeholder instead of borrowing
/// the selected row's artwork.
#[test]
fn active_foreign_remote_session_uses_the_card_placeholder() {
    let mut app = make_queue_app(3, 1);
    app.image_protocol_enabled = true;
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some({
        let mut s = make_session("remote-host", "Emby");
        s.now_playing = Some("Foreign Movie".into());
        s.runtime_s = 600;
        s
    });

    render_card(&mut app);

    assert!(
        app.queue_card_projection.cache_key.is_none(),
        "the placeholder stands in for an active item with no id to resolve"
    );
    assert!(
        !fetch_triggered(&app, "id1:P"),
        "the selected row's artwork must not back the active slot"
    );
}

/// The session names the item it is playing, so the slot fetches *its* Primary
/// image (an episode's still lives there) rather than the placeholder or the
/// selected row's artwork.
#[test]
fn active_foreign_remote_session_fetches_the_playing_items_artwork() {
    let mut app = make_queue_app(3, 1);
    app.image_protocol_enabled = true;
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some({
        let mut s = make_session("remote-host", "Emby");
        s.now_playing = Some("Foreign Episode".into());
        s.now_playing_item_id = Some("remote-episode".into());
        s.runtime_s = 600;
        s
    });

    render_card(&mut app);

    assert_eq!(
        app.queue_card_projection.cache_key.as_deref(),
        Some("remote-episode:P")
    );
    assert!(
        fetch_triggered(&app, "remote-episode:P"),
        "the playing item's own artwork is fetched"
    );
    assert!(
        !fetch_triggered(&app, "id1:P"),
        "the selected row's artwork must not back the active slot"
    );
}

/// The now-playing image's height cap tiers: 12 under 40 rows of terminal
/// height, 16 under 50, 24 at full height.
#[test]
fn card_height_cap_tiers_follow_terminal_height() {
    assert_eq!(queue_card_height_cap(30), 12);
    assert_eq!(queue_card_height_cap(39), 12);
    assert_eq!(queue_card_height_cap(40), 16);
    assert_eq!(queue_card_height_cap(49), 16);
    assert_eq!(queue_card_height_cap(50), 24);
    assert_eq!(queue_card_height_cap(80), 24);
}

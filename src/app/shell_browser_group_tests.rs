#![allow(dead_code, unused_imports)]

use super::super::*;
use super::test_support::*;
use crate::app::components::{BrowserComponent, Msg};
use crate::app::render::make_movie_app;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    App, BrowseLevel, ContextAction, FeedHomeVideoGroup, FeedHomeVideoState, LibraryTab,
    PanelFocus, PanelMode, TabSelection,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

/// A two-Emby-library app: the generic `lib-films` (index 0) and a second
/// generic `lib-series` (index 1), each with its own `nav_stack` cursor.
fn two_library_app() -> App {
    let mut app = browser_app_with_flat_movies(6);
    let mut library = make_item("Series", "CollectionFolder");
    library.id = "lib-series".into();
    library.is_folder = true;
    library.collection_type = "generic".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-series".into(),
            title: "Series".into(),
            items: make_items(4),
            total_count: 4,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    app
}
/// Task 3.7: the narrow browser's shell render seam schedules neighboring
/// images using the mounted component cursor, and applies both image-fetch
/// gates without relying on the legacy list painter.
#[test]
fn narrow_browser_shell_render_prefetches_only_when_idle_and_available() {
    use std::time::{Duration, Instant};

    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(browser_app_with_flat_movies(6));
    model.app.image_protocol_enabled = true;
    model.app.image_fetches_active = 6;
    model.sync_emby_browser();
    let id = model.emby_browser_id.clone().expect("browser mounted");

    assert!(matches!(
        drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE),
        Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index: 1 }))
    ));
    assert_eq!(browser_component_cursor(&model, &id), 1);

    // Recent navigation suppresses the shell-triggered effect entirely.
    model.app.last_nav_at = Instant::now();
    render_browser_model(&mut model, 80, 24);
    assert!(model.app.pending_image_fetches.is_empty());
    assert!(model.app.card_image_loading.is_empty());

    // Once idle, the same narrow shell draw queues every neighboring movie
    // in the cursor window. Saturating active fetches proves queued/busy
    // suppression is handled by the image seam rather than dropping work.
    model.app.last_nav_at = Instant::now() - Duration::from_millis(500);
    render_browser_model(&mut model, 80, 24);
    for i in [0, 2, 3, 4] {
        let key = format!("id{i}:cmp_primary");
        assert!(
            model.app.card_image_loading.contains(&key),
            "idle shell draw must reserve movie-{i}"
        );
        assert!(
            model
                .app
                .pending_image_fetches
                .iter()
                .any(|request| request.cache_key == key),
            "busy shell draw must queue movie-{i}"
        );
    }
}

/// keep-destination-components-mounted task 2.2: the Emby browser stays
/// mounted across tab switches (keep-mounted, D1), so switching away from
/// library A and back must leave A's `BrowserComponent` still `mounted()`
/// with its cursor preserved at the row it was moved to — not reset to 0 by
/// a switch-time unmount/remount.
#[test]
fn emby_browser_stays_mounted_and_preserves_cursor_across_switch() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(two_library_app());
    model.sync_emby_browser();
    let a_id = model.emby_browser_id.clone().expect("A browser mounted");

    // Move A's browser cursor to row 2 (component emits the typed index
    // request; the shell applies it to A's nav level, which retains it).
    let Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index })) =
        drive_browser_key(&mut model, &a_id, Key::Down, KeyModifiers::NONE)
    else {
        panic!("A Down must emit BrowserCursorIndex, got no typed request");
    };
    model.handle_browser_request(ShellRequest::BrowserCursorIndex { index });
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), index);
    assert_eq!(browser_component_cursor(&model, &a_id), index);

    // Switch to library B: A's component must stay mounted (keep-mounted).
    model.app.tab = TabSelection::EmbyLibrary(1);
    model.sync_emby_browser();
    assert!(
        model.application.mounted(&a_id),
        "A's browser must stay mounted after switching to B"
    );
    let b_id = model.emby_browser_id.clone().expect("B browser mounted");
    assert_ne!(a_id, b_id, "B must be a distinct browser");
    assert!(model.application.mounted(&b_id));

    // Switch back to A: still mounted, and the cursor is N (not 0).
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.sync_emby_browser();
    assert_eq!(model.emby_browser_id.as_ref(), Some(&a_id));
    assert!(
        model.application.mounted(&a_id),
        "A's browser must still be mounted after switching back"
    );
    assert_eq!(
        browser_component_cursor(&model, &a_id),
        index,
        "A's browser cursor must be preserved across the switch, not reset to 0"
    );
}

/// keep-destination-components-mounted task 2.3: with keep-mounted, content
/// is refreshed on re-point (D1 + risk mitigation). Switching away from
/// library A, mutating A's item list, and switching back must make the first
/// `render_emby_browser_component` frame reflect the new items — not stale
/// pre-switch content.
#[test]
fn emby_browser_refreshes_content_on_repoint_after_switch() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(two_library_app());
    model.sync_emby_browser();
    let a_id = model.emby_browser_id.clone().expect("A browser mounted");

    // Switch away to B (A stays mounted with its pre-mutation content).
    model.app.tab = TabSelection::EmbyLibrary(1);
    model.sync_emby_browser();
    assert!(model.application.mounted(&a_id));

    // Mutate A's item list while away: replace every item with a new one.
    let mut fresh = make_item("Fresh Movie", "Movie");
    fresh.id = "fresh-movie".into();
    model.app.libs[0].nav_stack[0].items = vec![fresh];
    model.app.libs[0].nav_stack[0].total_count = 1;

    // Switch back to A and paint the first frame.
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.sync_emby_browser();
    let backend = TestBackend::new(120, 40);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_base_frame(f, None);
        model.render_emby_browser_component(f);
    })
    .unwrap();
    let buffer = term.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(
        output.contains("Fresh Movie"),
        "the first frame after re-point must show the mutated item, got: {output:?}"
    );
    assert!(
        !output.contains("Item 1"),
        "the stale pre-switch content must not survive re-point"
    );
}

/// migrate-narrow-browse-to-components task 2.2: a feed/home-video
/// group-picker library (`is_feed_home_video_group_view` — here a podcast
/// channel) is owned by the mounted `BrowserComponent`. Its `[`/`]` chord
/// emits `BrowserCycleGroup` (not `BrowserCycleLetterPill`) because the shell
/// projects the group-pill flag onto the component's content, and routing it
/// through the shell moves `selected_group` via the previously-dead
/// `App::switch_feed_folder_group`.
fn feed_group_picker_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Podcast", "CollectionFolder");
    library.id = "lib-pod".into();
    library.is_folder = true;
    library.item_type = "Channel".into();

    let mut folder = make_item("Show A", "Folder");
    folder.id = "show-a".into();
    folder.is_folder = true;

    let mut e1 = make_item("E1", "Episode");
    e1.id = "e1".into();
    let mut e2 = make_item("E2", "Episode");
    e2.id = "e2".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-pod".into(),
            title: "Podcast".into(),
            items: vec![folder.clone()],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![e1, e2.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![e2],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    app
}

#[test]
fn feed_group_picker_bracket_keys_cycle_groups() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(feed_group_picker_app());
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    assert!(model.app.is_feed_home_video_group_view(0));
    model.sync_emby_browser();
    let id = model
        .emby_browser_id
        .clone()
        .expect("feed group-picker browser mounted");

    let msg = drive_browser_key(&mut model, &id, Key::Char(']'), KeyModifiers::NONE);
    assert!(
        matches!(
            msg,
            Some(Msg::Shell(ShellRequest::BrowserCycleGroup { delta: 1 }))
        ),
        "group-picker `]` must emit BrowserCycleGroup, got {msg:?}"
    );

    assert_eq!(
        model.app.libs[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .selected_group,
        0
    );
    model.handle_browser_request(ShellRequest::BrowserCycleGroup { delta: 1 });
    assert_eq!(
        model.app.libs[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .selected_group,
        1,
        "routing BrowserCycleGroup must advance the selected group"
    );
}

//! `migrate-narrow-browse-to-components` task 2.1: characterization +
//! regression coverage for the narrow Emby browse surfaces.
//!
//! The two characterization tests exercise the saved-position restore seam
//! (`LibEvent::RestoreLibraryPosition`) and must stay green across the
//! migration — restore still writes the resting `BrowseLevel` cursor that a
//! later content projection hands the owning component.
//!
//! The three regression markers are `#[ignore]`d (red until the named task)
//! and assert on the painted `TestBackend` buffer through `Model::draw_frame`
//! — the full draw path, since the narrow double-paint only exists when both
//! the legacy painter and the component `view` run.

use super::*;
use crate::app::components::BrowserComponent;
use crate::app::shell::Model;
use crate::app::tests::*;
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

fn saved_level(
    parent_id: &str,
    title: &str,
    focused_item_id: &str,
    item_types: Option<&str>,
) -> crate::config::LibraryPositionLevel {
    crate::config::LibraryPositionLevel {
        parent_id: parent_id.into(),
        title: title.into(),
        focused_item_id: Some(focused_item_id.into()),
        cursor_index: 0,
        item_types: item_types.map(Into::into),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        letter_filter_index: None,
        library_total: None,
    }
}

fn folder_items(prefix: &str, item_type: &str, n: usize) -> Vec<mbv_core::api::EmbyItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("{prefix} {i}"), item_type);
            item.id = format!("{prefix}-{i}");
            item.is_folder = true;
            item
        })
        .collect()
}

// ── Characterization: saved-position restore (green now, green after) ─────────

/// Entering a narrow Emby TV library restores its saved series position: the
/// restored top browse level lands its cursor on the saved `focused_item_id`,
/// not index 0.
#[test]
fn narrow_tv_library_restores_saved_series_position() {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.panel_focus = PanelFocus::Queue;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));

    let saved = saved_level("lib-shows", "Shows", "Series-3", Some("Series"));
    let position = crate::config::LibraryPosition {
        levels: vec![saved.clone()],
        ..Default::default()
    };
    app.replace_saved_library_position(0, position.clone());

    let level = BrowseLevel::from_position_level(&saved, folder_items("Series", "Series", 5), 5, 10);
    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: position.clone(),
        position,
        nav_stack: vec![level],
    });

    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 3,
        "entering the narrow TV library must restore the saved series (Series-3 at index 3)"
    );
}

/// Entering a narrow Emby grouped-Music library restores its saved album
/// position: the restored album (child) browse level lands its cursor on the
/// saved album `focused_item_id`.
#[test]
fn narrow_grouped_music_library_restores_saved_album_position() {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.panel_focus = PanelFocus::Queue;
    app.tab = TabSelection::EmbyLibrary(0);
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));

    let group_level = saved_level("lib-music", "Music", "group-1", None);
    let album_level = saved_level("group-1", "Beta", "album-2", None);
    let position = crate::config::LibraryPosition {
        levels: vec![group_level.clone(), album_level.clone()],
        ..Default::default()
    };
    app.replace_saved_library_position(0, position.clone());

    let groups = folder_items("group", "MusicArtist", 3);
    let albums: Vec<mbv_core::api::EmbyItem> = (0..4)
        .map(|i| {
            let mut album = make_item(&format!("Album {i}"), "MusicAlbum");
            album.id = format!("album-{i}");
            album
        })
        .collect();
    let nav_stack = vec![
        BrowseLevel::from_position_level(&group_level, groups, 3, 10),
        BrowseLevel::from_position_level(&album_level, albums, 4, 10),
    ];
    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: position.clone(),
        position,
        nav_stack,
    });

    assert_eq!(app.libs[0].nav_stack.len(), 2, "grouped path must survive restore");
    assert_eq!(
        app.libs[0].nav_stack[1].cursor, 2,
        "entering the narrow grouped Music library must restore the saved album (album-2 at index 2)"
    );
}

// ── Regression markers: red until the named task ─────────────────────────────

fn narrow_backend() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(60, 20)).unwrap()
}

fn buffer_text(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(model: &mut Model, term: &mut Terminal<TestBackend>) -> String {
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    buffer_text(term)
}

/// Feed one key into whatever component currently holds focus and route any
/// emitted `Msg` the way the run loop does. Pre-migration the narrow browse
/// surfaces have no owning component, so focus rests on `UiRoot` and the key
/// is dead — which is exactly what the ignored tests document.
fn press(model: &mut Model, code: Key) {
    let focused = model.application.focus().cloned();
    if let Some(id) = &focused {
        let msg = model
            .application
            .get_component_mut(id)
            .expect("focused component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }));
        if let Some(msg) = msg {
            let mut music_resize = false;
            let mut tv_resize = false;
            model.handle_terminal_message(msg, focused.as_ref(), &mut music_resize, &mut tv_resize);
        }
    }
    model.sync_mounted_surfaces();
}

fn tv_shows_app() -> App {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.collection_type = "tvshows".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: folder_items("Series", "Series", 5),
            total_count: 5,
            cursor: 0,
            scroll: 0,
            item_types: Some("Series".into()),
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

/// Regression 3: narrow TV `j` moves the painted selection. Post-task-3.4 the
/// mounted `BrowserComponent` owns the surface, so the painted selection lives
/// in its own layout (`test_layout`), keyed off its component-local cursor.
#[test]
fn narrow_tv_browse_j_moves_painted_selection() {
    let mut model = Model::new(tv_shows_app());
    model.sync_mounted_surfaces();
    let id = model.emby_browser_id.clone().expect("narrow TV browser mounted");
    let mut term = narrow_backend();

    // Seed past the selected-Series inline hero (which swallows its own row)
    // so both samples are plain rows.
    model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BrowserComponent>()
        .unwrap()
        .set_cursor_for_test(1);
    draw(&mut model, &mut term);
    let before = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .unwrap()
        .test_layout()
        .selected_item_rect;
    assert!(before.is_some(), "narrow TV browse must paint a selected row");

    press(&mut model, Key::Char('j'));
    draw(&mut model, &mut term);
    let after = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .unwrap()
        .test_layout()
        .selected_item_rect;

    assert_ne!(
        before, after,
        "j must move the painted selection down the narrow TV series list"
    );
}

/// Regression 4: narrow grouped Music `j` moves the painted selection.
#[test]
fn narrow_grouped_music_j_moves_painted_selection() {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;
    let album_level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 0..3 {
        let mut album = make_item(&format!("Extra Album {i}"), "MusicAlbum");
        album.id = format!("album-extra-{i}");
        album.artist = "Alpha".into();
        album_level.items.push(album);
    }
    album_level.total_count = album_level.items.len();

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    draw(&mut model, &mut term);
    let before = model.app.layout.main.selected_item_rect;
    assert!(
        before.is_some(),
        "narrow grouped Music must paint a selected album row"
    );

    press(&mut model, Key::Char('j'));
    draw(&mut model, &mut term);
    let after = model.app.layout.main.selected_item_rect;

    assert_ne!(
        before, after,
        "j must move the painted selection down the narrow grouped-album list"
    );
}

/// Characterization (task 3.3 template step a): pins the painted narrow
/// generic/Movies surface — inline movie hero + browse rows — through the
/// full `Model::draw_frame` path. The pre-migration buffer (committed with
/// this test) carried the regression-1 double paint: the legacy `render_list`
/// reserved rows for the inline hero while `BrowserComponent::view` painted
/// the same rows with no reservation, so the selected row and "Second Movie"
/// showed twice at different offsets. This is the post-migration buffer: the
/// `browser_narrow` composer is the sole painter, the selected row is
/// swallowed by the framed hero block, and every row paints once.
#[test]
fn narrow_movies_surface_snapshot() {
    let mut app = crate::app::render::make_movie_app();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    let expected = "                                                            \n   HOME  ▐ MOVIES                                           \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁ \n                                                            \n   Focused Movie                                            \n   Action  1988                                             \n                                                            \n   This overview should appear in the compact movie         \n   banner while the list remains visible underneath.        \n                                                            \n ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔ \n  Second Movie                                              \n                                                            \n                                                            \n 🔊  100                                             \u{f06b4} \u{ede2} ♥ \u{f1c0} ";
    assert_eq!(output, expected, "narrow Movies surface drifted:\n{output}");
}

/// Characterization (task 3.4 template step a): pins the painted narrow Emby
/// TV browse surface — inline series hero + series rows — through the full
/// `Model::draw_frame` path. Unlike narrow Movies (regression 1), narrow TV
/// carried no double-paint bug: legacy `render_list` and `BrowserComponent`
/// both used the `tvshows` shared-replacement plan, so they painted the same
/// rows at the same offsets. This buffer is byte-identical before and after
/// the task-3.4 migration — no rebake.
#[test]
fn narrow_tv_surface_snapshot() {
    let mut model = Model::new(tv_shows_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    let expected = "                                                            \n   HOME  ▐ SHOWS                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁ \n                                                            \n   Series 0                                                 \n                                                            \n                                                            \n ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔ \n  Series 1                                                  \n  Series 2                                                  \n  Series 3                                                  \n  Series 4                                                  \n                                                            \n                                                            \n 🔊  100                                             \u{f06b4} \u{ede2} ♥ \u{f1c0} ";
    assert_eq!(output, expected, "narrow TV surface drifted:\n{output}");
}

/// Regression (task 3.4 template step d): narrow Emby TV paints each visible
/// series/season row exactly once — the mounted `BrowserComponent` is the sole
/// painter now that the legacy `render_list` narrow branch early-returns for
/// `tvshows` too.
#[test]
fn narrow_tv_paints_each_browse_row_once() {
    let mut model = Model::new(tv_shows_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    for row in ["Series 0", "Series 1", "Series 2", "Series 3", "Series 4"] {
        assert_eq!(
            output.matches(row).count(),
            1,
            "narrow TV browse row {row:?} must be painted exactly once:\n{output}"
        );
    }
}

fn podcast_app() -> App {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-podcasts".into(),
            title: "Podcasts".into(),
            items: folder_items("Show", "Series", 5),
            total_count: 5,
            cursor: 0,
            scroll: 0,
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

/// Characterization (task 3.5a template step a): pins the painted narrow Emby
/// podcast browse surface through the full `Model::draw_frame` path. Narrow
/// podcast renders as generic list rows (`truncate_overview = true`), with no
/// podcast-specific layout.
#[test]
fn narrow_podcast_surface_snapshot() {
    let mut model = Model::new(podcast_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    let expected = "                                                            \n   HOME  ▐ PODCASTS                                         \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n▎ Show 0                                                    \n  Show 1                                                    \n  Show 2                                                    \n  Show 3                                                    \n  Show 4                                                    \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n 🔊  100                                             \u{f06b4} \u{ede2} ♥ \u{f1c0} ";
    assert_eq!(output, expected, "narrow podcast surface drifted:\n{output}");
}

/// Regression (task 3.5a template step d): narrow Emby podcast paints each
/// visible show row exactly once — the mounted `BrowserComponent` is the sole
/// painter now that the legacy `render_list` narrow branch early-returns for
/// podcast libraries too.
#[test]
fn narrow_podcast_paints_each_browse_row_once() {
    let mut model = Model::new(podcast_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    for row in ["Show 0", "Show 1", "Show 2", "Show 3", "Show 4"] {
        assert_eq!(
            output.matches(row).count(),
            1,
            "narrow podcast browse row {row:?} must be painted exactly once:\n{output}"
        );
    }
}

fn wide_podcast_app() -> App {
    let mut app = podcast_app();
    app.terminal_width = 140;
    app.terminal_height = 40;
    app
}

fn wide_backend() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(140, 40)).unwrap()
}

fn feed_home_video_group_app() -> App {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut first = make_item("Video One", "Movie");
    first.id = "video-one".into();
    let mut second = make_item("Video Two", "Movie");
    second.id = "video-two".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(), title: "YouTube".into(),
            items: vec![folder.clone()], total_count: 1, cursor: 0, scroll: 0,
            item_types: None, unplayed_only: false, sort_by: "SortName".into(),
            sort_order: "Ascending".into(), loading: false, all_items: None,
            letter_filter: None, music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![first.clone(), second.clone()],
            groups: vec![FeedHomeVideoGroup { folder, items: vec![first, second] }],
            loading: false, ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });
    app
}

fn feed_snapshot(width: u16, height: u16) -> String {
    let mut app = feed_home_video_group_app();
    app.terminal_width = width;
    app.terminal_height = height;
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    draw(&mut model, &mut term)
}

#[test]
fn feed_home_video_group_narrow_snapshot_matches_fbc6888e_baseline() {
    let output = feed_snapshot(60, 20);
    assert_eq!(output, FEED_NARROW_BASELINE, "feed narrow drifted");
}

#[test]
fn feed_home_video_group_wide_snapshot_matches_fbc6888e_baseline() {
    let output = feed_snapshot(140, 40);
    assert_eq!(output, FEED_WIDE_BASELINE, "feed wide drifted");
}

#[test]
fn feed_home_video_group_paints_each_row_once() {
    for (width, height, baseline) in [
        (60, 20, FEED_NARROW_BASELINE),
        (140, 40, FEED_WIDE_BASELINE),
    ] {
        let output = feed_snapshot(width, height);
        assert_eq!(output, baseline, "feed {width}x{height} is not a single-paint frame");
    }
}

const FEED_NARROW_BASELINE: &str = "                                                            \n   HOME  ▐ YOUTUBE                                          \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n  ⌘ ◢ All ◤◢ Channel A ◤                                    \n                                                            \n▎ Video One                                                 \n  Video Two                                                 \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n 🔊  100                                             \u{f06b4} \u{ede2} ♥ \u{f1c0} ";
const FEED_WIDE_BASELINE: &str = "                                                                                                                                            \n                                           HOME  ▐ YOUTUBE                                                                                  \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                    ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  \n                                                                                    ▎  Video One                                            \n                                                                                       Video Two                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                              Video One ○                                                                                   \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n     🖧  WOIMS                                                                                                                               \n                                                                                                                                            \n    Add items with p from Home or libr                                                                                                      \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n     🖭  none                                                                        ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁  \n                                                                                                                                            \n                                         🔊  100                                                                                     \u{f06b4} \u{ede2} ♥ \u{f1c0} ";

/// Characterization (task 3.5b template step a): pins the painted WIDE Emby
/// podcast browse surface through the full `Model::draw_frame` path, at a
/// wide+tall size where `shared_hero_presentation` returns `Some`. The
/// pre-change baseline (committed with this test) was BLANK: `render_list`'s
/// hero-presentation early return fired for podcast libraries and returned
/// before anything published `layout.left_area`, and no podcast wide-workspace
/// component exists. This is the post-change buffer: with the podcast disjunct
/// removed from that early return, wide podcast falls through to the
/// `component_owned` block and the mounted `BrowserComponent` composes the
/// generic browse body across the wide area (blank -> browse body, an expected
/// bug-fix diff).
#[test]
fn wide_podcast_surface_snapshot() {
    let mut model = Model::new(wide_podcast_app());
    model.sync_mounted_surfaces();
    let mut term = wide_backend();

    let output = draw(&mut model, &mut term);

    let expected = WIDE_PODCAST_SURFACE;
    assert_eq!(output, expected, "wide podcast surface drifted:\n{output}");
}

const WIDE_PODCAST_SURFACE: &str = "                                                                                                                                            \n                                           HOME  ▐ PODCASTS                                                                                 \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                          ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁  \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                          ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  \n                                           Show 2                                           Show 3                                          \n                                           Show 4                                                                                           \n     🖧  WOIMS                                                                                                                               \n                                                                                                                                            \n    Add items with p from Home or libr                                                                                                      \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n                                                                                                                                            \n     🖭  none                                                                                                                                \n                                                                                                                                            \n                                         🔊  100                                                                                     󰚴  ♥  ";

/// Regression (task 3.5b template step d): the WIDE Emby podcast browse surface
/// paints the generic browse body (mounted `BrowserComponent`, kind `Generic`)
/// across the wide area — it is no longer blank. The shared narrow composer
/// runs wide here (no podcast wide-specific layout, per the task): it reserves
/// a placeholder hero block and lays the show rows out in a multi-column grid,
/// so the earliest rows sit under the hero reservation and `Show 2`..`Show 4`
/// are the visible browse body. Matching the wide generic-collection case
/// (task 3.3 scope note), this shared-composer wide behavior is task 3.8
/// territory, not a 3.5b regression.
#[test]
fn wide_podcast_paints_browse_body() {
    let mut model = Model::new(wide_podcast_app());
    model.sync_mounted_surfaces();
    let mut term = wide_backend();

    let output = draw(&mut model, &mut term);

    for row in ["Show 2", "Show 3", "Show 4"] {
        assert_eq!(
            output.matches(row).count(),
            1,
            "wide podcast browse row {row:?} must be painted exactly once:\n{output}"
        );
    }
}

/// Regression 5: narrow Movies paints each browse row exactly once (currently
/// double-painted by legacy `render_list` + `BrowserComponent::view`).
#[test]
fn narrow_movies_paints_each_browse_row_once() {
    let mut app = crate::app::render::make_movie_app();
    app.terminal_width = 60;
    app.terminal_height = 20;
    app.mini_view_focus = PanelFocus::Library;

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    assert_eq!(
        output.matches("Second Movie").count(),
        1,
        "narrow Movies browse row must be painted exactly once, not double-painted:\n{output}"
    );
}

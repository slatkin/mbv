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

/// Regression 3: narrow TV `j` moves the painted selection.
#[test]
fn narrow_tv_browse_j_moves_painted_selection() {
    let mut model = Model::new(tv_shows_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    draw(&mut model, &mut term);
    let before = model.app.layout.main.selected_item_rect;
    assert!(before.is_some(), "narrow TV browse must paint a selected row");

    press(&mut model, Key::Char('j'));
    draw(&mut model, &mut term);
    let after = model.app.layout.main.selected_item_rect;

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
/// `Model::draw_frame` path. Baked pre-migration (legacy `render_list` narrow
/// branch still paints); task 3.4 step b/c rebakes if the migration changes
/// the buffer.
#[test]
fn narrow_tv_surface_snapshot() {
    let mut model = Model::new(tv_shows_app());
    model.sync_mounted_surfaces();
    let mut term = narrow_backend();

    let output = draw(&mut model, &mut term);

    let expected = "                                                            \n   HOME  ▐ SHOWS                                            \n                                                            \n                                                            \n                                                            \n                                                            \n                                                            \n ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁ \n                                                            \n   Series 0                                                 \n                                                            \n                                                            \n ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔ \n  Series 1                                                  \n  Series 2                                                  \n  Series 3                                                  \n  Series 4                                                  \n                                                            \n                                                            \n 🔊  100                                             \u{f06b4} \u{ede2} ♥ \u{f1c0} ";
    assert_eq!(output, expected, "narrow TV surface drifted:\n{output}");
}

/// Regression (task 3.4 template step a): narrow Emby TV paints each visible
/// series/season row exactly once. Red pre-migration (legacy `render_list` +
/// `BrowserComponent::view` double-paint); un-ignored in step d.
#[test]
#[ignore = "green after task 3.4"]
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

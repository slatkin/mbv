//! The mounted Music surface's projection contracts: what the shell paint
//! path, the legacy base frame, and the shared numbered-track labels present
//! at each fixture width.

use super::test_helpers::{
    buffer_to_string, draw_mounted_frame, make_music_group_app, make_music_tree_group_app,
    mounted_model_at, mounted_music_narrow_geometry, mounted_music_wide_geometry,
    music_tree_row_text,
};
use super::*;
use crate::app::components::list::tree_browser::TreeOperation;
use crate::app::components::music_content::MusicContent;
use crate::app::components::music_tree_target::MusicTreeTarget;
use crate::app::music_grouping::ArtistKey;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rstest::rstest;

/// Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
/// now (task 3.8), so route the narrow characterization renders through the
/// real `Model::draw_frame` shell path.
fn render_narrow_music(app: App, width: u16, height: u16) -> String {
    let mut model = mounted_model_at(app, width, height);
    draw_mounted_frame(&mut model, width, height)
}

#[rstest]
#[case::wide(160, 40)]
#[case::narrow_grouped_music(60, 30)]
fn numbered_tree_track_labels_are_painted_at_fixture_widths(
    #[case] width: u16,
    #[case] height: u16,
) {
    let mut indexed = crate::app::tests::make_item("Indexed Track", "Audio");
    indexed.id = "track-indexed".into();
    indexed.index_number = 7;
    let mut fallback = crate::app::tests::make_item("Fallback Track", "Audio");
    fallback.id = "track-fallback".into();
    fallback.index_number = 0;

    // Feed raw cached tracks through MusicContent::set_content, which is the
    // same projection path the shell uses. The assertion therefore fails if
    // set_content stops applying the shared numbered-row label.
    let mut album = crate::app::tests::make_item("Album", "MusicAlbum");
    album.id = "album".into();
    album.artist = "Artist".into();
    album.is_folder = true;
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album.clone()], 0),
        Some(album),
        String::new(),
        Vec::new(),
        0,
        vec![("Artist".into(), String::new(), "Album".into())],
        vec![ArtistKey::Fallback("Artist".into())],
        vec![0],
        Some(vec![indexed, fallback]),
    ));
    // The album adoption already revealed the artist root's path; expand the
    // album leaf so its cached track rows join the projection.
    let album_target = owner
        .browser
        .visible_targets()
        .into_iter()
        .find(|target| owner.browser.node(target).map(|node| node.title.as_str()) == Some("Album"))
        .expect("album leaf");
    if !owner.browser.is_expanded(&album_target) {
        owner
            .browser
            .apply(TreeOperation::ToggleExpansionTarget(album_target));
    }
    let indexed_target = owner
        .browser
        .visible_targets()
        .into_iter()
        .find(|target| {
            owner.browser.node(target).map(|node| node.title.as_str()) == Some("7. Indexed Track")
        })
        .expect("indexed track");

    let mut model = mounted_model_at(make_music_tree_group_app(), width, height);
    draw_mounted_frame(&mut model, width, height);
    let list_area = if width >= crate::app::TWO_COLUMN_THRESHOLD {
        mounted_music_wide_geometry(&model).list_area
    } else {
        mounted_music_narrow_geometry(&model).list_area
    };
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|frame| tuirealm::component::Component::view(&mut owner.browser, frame, list_area))
        .unwrap();
    let row_y_of = |target: &MusicTreeTarget| {
        (0..height).find(|y| {
            owner
                .browser
                .resolve_current_point(ratatui::layout::Position::new(list_area.x, *y))
                == Some(target)
        })
    };
    let indexed_y = row_y_of(&indexed_target).expect("indexed track row painted");
    let row = music_tree_row_text(&term, indexed_y, list_area.x, list_area.right());
    assert!(row.contains("7. Indexed Track"), "painted row: {row:?}");
    let fallback_target = owner
        .browser
        .visible_targets()
        .into_iter()
        .find(|target| {
            owner.browser.node(target).map(|node| node.title.as_str()) == Some("2. Fallback Track")
        })
        .expect("fallback track");
    let fallback_y = row_y_of(&fallback_target).expect("fallback row painted");
    assert!(
        music_tree_row_text(&term, fallback_y, list_area.x, list_area.right())
            .contains("2. Fallback Track"),
        "fallback label is painted"
    );
}

fn render_music_legacy(app: &mut App, width: u16, height: u16, _focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = Rect::default();
    terminal
        .draw(|f| {
            app.reserve_library_area(f, Rect::new(0, 0, width, height), &mut layout, None);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Wide grouped Music: the legacy base frame is now geometry-only — the
    // mounted `MusicWorkspaceComponent` is the sole painter (#613), so
    // `render_library` paints no grouped-album rows at the wide breakpoint.
    for (width, height, focused) in [(120, 30, true), (120, 30, false)] {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].set_resting_cursor(0);
        let output = render_music_legacy(&mut app, width, height, focused);
        assert!(
            !output.contains("First Album"),
            "wide legacy frame must not paint music rows in {width}x{height}: {output:?}"
        );
    }

    // Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
    // now (task 3.8), reached through the full `Model::draw_frame` path.
    for (width, height) in [(60, 30), (60, 20)] {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].set_resting_cursor(0);
        let output = render_narrow_music(app, width, height);
        assert!(
            output.contains("First Album"),
            "narrow music row missing in {width}x{height}: {output:?}"
        );
    }
}

#[test]
fn narrow_grouped_music_shows_group_pill_bar() {
    // Task 3.6a: the narrow branch reserves a group pill row above the album
    // rows, mirroring narrow TV and the narrow browser. Before the fix the
    // narrow branch rendered album rows straight into the full area and never
    // published `selector_tabs`.
    let app = make_music_group_app();
    let mut model = mounted_model_at(app, 60, 30);
    let output = draw_mounted_frame(&mut model, 60, 30);
    let selector_tabs = model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .expect("Library panel mounted")
        .test_selector_hits()
        .regions()
        .to_vec();
    assert!(
        !selector_tabs.is_empty(),
        "narrow grouped Music must publish group selector pills:\n{output}"
    );
    assert!(
        output.contains("Beta"),
        "group pill labels must paint:\n{output}"
    );
}

use super::test_helpers::{
    buffer_to_string, draw_mounted_frame, make_music_group_app, mounted_model_at,
    render_library_to_string_sized,
};
use super::*;
use crate::app::layout::LayoutMain;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
/// now (task 3.8), so route the narrow characterization renders through the
/// real `Model::draw_frame` shell path.
fn render_narrow_music(app: App, width: u16, height: u16) -> String {
    let mut model = mounted_model_at(app, width, height);
    draw_mounted_frame(&mut model, width, height)
}

fn render_music_legacy(app: &mut App, width: u16, height: u16, focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|f| {
            app.render_library(
                f,
                Rect::new(0, 0, width, height),
                focused,
                &mut layout,
                None,
            );
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
        app.libs[0].nav_stack[1].cursor = 0;
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
        app.libs[0].nav_stack[1].cursor = 0;
        let output = render_narrow_music(app, width, height);
        assert!(
            output.contains("First Album"),
            "narrow music row missing in {width}x{height}: {output:?}"
        );
    }
}

/// Task 3.1/3.2: the narrow grouped-album inline hero used to expand the
/// selected album's row into a track table + "^P: Play | ..." action hint
/// (`render_album_detail`, called from `AlbumInlineDetailStart`). It now
/// routes through the Model A hero (`render_album_hero_detail`): title +
/// meta + art only. The track list moved to the selection modal (task 3.3).
#[test]
fn narrow_grouped_music_hero_shows_only_title_meta_no_track_table_or_action_hint() {
    let mut app = make_music_group_app();
    let tracks: Vec<mbv_core::api::EmbyItem> = (0..2)
        .map(|i| {
            let mut track = make_item(&format!("Track {}", i + 1), "Audio");
            track.id = format!("track-{}", i + 1);
            track.index_number = (i + 1) as i64;
            track
        })
        .collect();
    app.album_tracks_cache.insert("album-1".into(), tracks);

    let output = render_narrow_music(app, 60, 30);

    assert!(
        output.contains("First Album"),
        "hero must still show the selected album's title:\n{output}"
    );
    assert!(
        !output.contains("Track 1") && !output.contains("Track 2"),
        "the inline hero must no longer show the track table:\n{output}"
    );
    assert!(
        !output.contains("Show tracks") && !output.contains("Play | "),
        "the inline hero must no longer show the action-hint row:\n{output}"
    );
}

#[test]
fn wide_grouped_music_publishes_same_frame_layout_geometry() {
    let mut app = make_music_group_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 120, 30);

    assert_eq!(layout.wide_music_area, Rect::new(0, 0, 120, 30));
    assert!(layout.is_wide_music_active());
    assert!(layout.left_area.width > 0);
    assert!(layout.hero_area.width > 0);
    assert!(layout.wide_music_right_area.width > 0);
}

/// D4 proof: at the wide breakpoint the legacy base frame publishes the
/// `wide_music_*` hand-off geometry but paints no grouped-album rows — the
/// mounted `MusicWorkspaceComponent` is the sole painter (#613). Mirrors
/// `tests_non_music::wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows`.
#[test]
fn wide_music_legacy_base_frame_publishes_geometry_but_paints_no_rows() {
    let mut app = make_music_group_app();
    app.libs[0].nav_stack[1].cursor = 0;
    let mut layout = LayoutMain::default();
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    term.draw(|f| {
        app.render_library(f, Rect::new(0, 0, 120, 40), true, &mut layout, None);
    })
    .unwrap();

    assert!(
        layout.wide_music_area.width > 0 && layout.wide_music_area.height > 0,
        "wide music area hand-off must still be reserved: {:?}",
        layout.wide_music_area
    );
    assert!(
        layout.wide_music_right_area.width > 0 && layout.wide_music_right_area.height > 0,
        "wide music right area hand-off must still be reserved: {:?}",
        layout.wide_music_right_area
    );
    let output = buffer_to_string(&term);
    assert!(
        !output.contains("First Album"),
        "legacy base frame must not paint grouped-album rows at the wide breakpoint: {output:?}"
    );
}

#[test]
fn narrow_grouped_music_publishes_no_wide_track_targets() {
    let mut app = make_music_group_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 30);

    assert!(!layout.is_wide_music_active());
    assert!(layout.wide_music_track_hitmap.is_empty());
}

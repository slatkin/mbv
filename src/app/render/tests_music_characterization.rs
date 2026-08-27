use super::test_helpers::{buffer_to_string, make_music_group_app, render_library_to_string_sized};
use super::*;
use crate::app::layout::LayoutMain;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_music(app: &mut App, width: u16, height: u16, focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|f| {
            app.render_library(f, Rect::new(0, 0, width, height), focused, &mut layout);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    let states = [(120, 30, true, 0), (120, 30, false, 0), (60, 30, true, 0)];
    for (width, height, focused, cursor) in states {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].cursor = cursor;
        let output = render_music(&mut app, width, height, focused);
        assert!(
            output.contains("First Album"),
            "music rows missing in {width}x{height}: {output:?}"
        );
    }

    let mut selected = make_music_group_app();
    selected.libs[0].nav_stack[1].cursor = 0;
    let output = render_music(&mut selected, 60, 8, true);
    assert!(
        output.contains("First Album"),
        "selected music row missing: {output:?}"
    );
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

    let output = render_music(&mut app, 60, 30, true);

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

#[test]
fn narrow_grouped_music_publishes_no_wide_track_targets() {
    let mut app = make_music_group_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 30);

    assert!(!layout.is_wide_music_active());
    assert!(layout.wide_music_track_hitmap.is_empty());
}

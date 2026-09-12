use super::test_helpers::{
    buffer_to_string, draw_mounted_frame, make_music_group_app, mounted_model_at,
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

fn render_music_legacy(app: &mut App, width: u16, height: u16, _focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
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

/// Task 9.3: the Narrow inline hero and the Wide header derive from one
/// `HeroContent` (design D3/D7), so one resolved artist/year/title paints at
/// both breakpoints and neither falls back to the raw folder name.
#[test]
fn narrow_and_wide_music_paint_the_same_resolved_album_hero() {
    let narrow = render_narrow_music(folder_hero_app(), 60, 30);
    let mut wide_model = mounted_model_at(folder_hero_app(), 160, 40);
    let wide = draw_mounted_frame(&mut wide_model, 160, 40);

    for expected in ["Folder Artist", "2024", "First Album"] {
        assert!(
            narrow.contains(expected),
            "narrow hero missing {expected}:\n{narrow}"
        );
        assert!(
            wide.contains(expected),
            "wide hero missing {expected}:\n{wide}"
        );
    }
    for output in [&narrow, &wide] {
        assert!(
            !output.contains("Folder Artist (2024) First Album"),
            "the raw folder name must not paint as the hero title:\n{output}"
        );
    }
}

fn folder_hero_app() -> App {
    let mut app = make_music_group_app();
    app.libs[0].nav_stack[1].set_resting_cursor(0);
    let album = &mut app.libs[0].nav_stack.last_mut().unwrap().items[0];
    album.artist.clear();
    album.name = "Folder Artist (2024) First Album".into();
    album.production_year = 0;
    app.album_artist_cache
        .insert(album.id.clone(), "Folder Artist".into());
    app
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

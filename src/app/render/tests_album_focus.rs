use super::test_helpers::*;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab, QueueScope, RemoteSlotState};

#[test]
fn selected_music_group_album_keeps_right_aligned_art_in_track_mode() {
    let mut app = make_power_music_group_app();
    app.image_protocol_enabled = true;
    app.libs[0].album_track_focus = Some(0);

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.player_tab.set_items(vec![track.clone()], 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.paused = false;
    }
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let art_rect = layout
        .inline_image_rect
        .expect("expected selected album art rect in track mode");

    assert!(
        out.contains("Openi") && out.contains("Track"),
        "expected inline track row (may be wrapped):\n{out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    let playing_line = lines
        .iter()
        .find(|line| line.contains("Openi"))
        .copied()
        .expect("expected active music track row");
    let icon = super::play_icon(app.use_nerd_fonts);
    assert!(
        playing_line.contains(&format!("1. {icon} Openi")),
        "expected the active track icon and following space after its number:\n{out}"
    );
    let track_y = lines
        .iter()
        .position(|line| line.contains("Openi"))
        .expect("expected inline track row");
    let hint_y = lines[..track_y]
        .iter()
        .rposition(|line| line.contains("^P: Play"))
        .expect("expected track-mode action hint row");
    assert!(
        lines[hint_y..track_y]
            .iter()
            .any(|line| line.contains("BACK: Exit")),
        "expected track-mode hint row to show the exit hint:\n{out}"
    );
    assert!(
        track_y > hint_y,
        "expected the track list below the track-mode hint:\n{out}"
    );
    let hint_x = lines[hint_y]
        .find("^P: Play")
        .expect("expected track-mode hint x position");
    assert_eq!(
        hint_x, 2,
        "track-mode detail hint has 2-column indent in grouped block"
    );
    assert!(
        art_rect.x + art_rect.width == 58,
        "album art should have two columns of right padding"
    );
    assert!(app.card_image_loading.contains("album-1:P"));
    assert!(!app.card_image_loading.contains("track-1:P"));
}

#[test]
fn album_folder_listing_preserves_inline_track_focus_cursor() {
    let mut app = make_power_music_group_app();
    app.libs[0].album_track_focus = Some(1);

    let mut first = make_item("Opening Track", "Audio");
    first.id = "track-1".into();
    first.album = "First Album".into();
    first.artist = "Alpha".into();
    first.index_number = 1;

    let mut second = make_item("Focused Track", "Audio");
    second.id = "track-2".into();
    second.album = "First Album".into();
    second.artist = "Alpha".into();
    second.index_number = 2;

    app.album_tracks_cache
        .insert("album-1".into(), vec![first, second]);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let focused_y = out
        .lines()
        .position(|line| line.contains("Focused Track"))
        .expect("expected focused track row");
    let lines: Vec<&str> = out.lines().collect();
    let hint_y = lines
        .iter()
        .position(|line| line.contains("BACK: Exit"))
        .expect("expected track-mode action hint row");
    assert!(
        lines[hint_y].contains("BACK: Exit"),
        "expected track-mode hint row to show the exit hint:\n{out}"
    );
    let album_y = lines
        .iter()
        .position(|line| line.contains("First Album"))
        .expect("expected album title row");
    assert!(album_y > hint_y, "expected album title after hint:\n{out}");
    let first_track_y = lines
        .iter()
        .position(|line| line.contains("Opening Track"))
        .expect("expected first track row");
    assert!(
        first_track_y > album_y,
        "expected tracks after album title:\n{out}"
    );
    assert_eq!(
        focused_y,
        first_track_y + 1,
        "expected second track after first track:\n{out}"
    );

    assert_eq!(
        layout.cursor_screen_y,
        Some(focused_y as u16),
        "expected layout cursor to follow the focused inline track row"
    );
}

#[test]
fn selected_album_item_follows_raw_cursor_not_display_order() {
    let mut app = make_power_music_group_app();

    let mut second_album = make_item("Zero Day", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Aaardvark".into();

    {
        let lvl = app.libs[0].nav_stack.last_mut().unwrap();
        lvl.items.push(second_album);
        lvl.cursor = 1;
    }

    let selected = app
        .selected_album_item(0)
        .expect("expected a selected album at cursor 1");
    assert_eq!(
        selected.id, "album-2",
        "expected the raw items[cursor] entry, not a sorted/display-order lookup"
    );

    app.libs[0].album_track_focus = Some(0);

    let mut layout = LayoutMain::default();
    let _ = render_power_library_to_string(&mut app, &mut layout);
    assert!(
        app.album_tracks_loading.contains("album-2"),
        "expected the fetch triggered by rendering to target the cursor-selected \
         album (album-2), not album-1"
    );
    assert!(
        !app.album_tracks_loading.contains("album-1"),
        "album-1 is no longer selected, so it should not be (re)fetched"
    );
}

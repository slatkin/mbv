use super::test_helpers::*;
use super::*;
use crate::app::tests::make_item;

#[test]
fn album_folder_listing_fetches_and_shows_loading_on_cache_miss() {
    let mut app = make_power_music_group_app_with_second_album();
    assert!(!app.album_tracks_cache.contains_key("album-1"));
    assert!(!app.album_tracks_loading.contains("album-1"));

    app.libs[0].album_track_focus = Some(0);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let lines: Vec<&str> = out.lines().collect();

    assert!(
        app.album_tracks_loading.contains("album-1"),
        "expected a cache miss to trigger fetch_album_tracks for the \
         selected album:\n{out}"
    );
    assert!(
        out.to_lowercase().contains("loading"),
        "expected a loading indicator in the detail pane while the \
         fetch is in flight:\n{out}"
    );
    let title_y = lines
        .iter()
        .position(|l| l.contains("First Album"))
        .expect("expected selected album row");
    let loading_y = lines
        .iter()
        .position(|l| l.to_lowercase().contains("loading"))
        .expect("expected an inline loading row");

    assert_inline_detail_frames_between_albums(&lines, &layout, title_y, loading_y);
}

#[test]
fn album_folder_inline_detail_is_hidden_until_track_selection_mode() {
    let mut app = make_power_music_group_app();

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();

    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("First Album"))
            .count(),
        1,
        "expected no duplicate inline album title row:\n{out}"
    );

    assert!(
        !out.contains("Opening Track"),
        "expected inline tracks to stay hidden until track-selection mode is entered \
         (Enter pressed):\n{out}"
    );

    let hint_y = lines
        .iter()
        .position(|line| line.contains("^P: Play"))
        .expect("expected inline action hint row");
    assert!(
        lines[hint_y].contains("ENTER: Show"),
        "expected the collapsed hint row to prompt Enter to show tracks:\n{out}"
    );
    let hint_x = lines[hint_y]
        .find("^P: Play")
        .expect("expected hint x position");
    let title_y = lines
        .iter()
        .position(|line| line.contains("First Album"))
        .expect("expected selected album title row");
    let title_x = lines[title_y]
        .find("First Album")
        .expect("expected selected album title position");
    assert_eq!(
        hint_x,
        lines[title_y][..title_x].chars().count(),
        "expected collapsed hint content to align with the selected album title:\n{out}"
    );
}

#[test]
fn selected_music_group_album_shows_right_aligned_art_before_track_mode() {
    let mut app = make_power_music_group_app();
    app.image_protocol_enabled = true;

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let art_rect = layout
        .inline_image_rect
        .expect("expected selected album art rect before track mode");

    assert!(
        !out.contains("Opening Track"),
        "tracks should stay hidden until track-selection mode:\n{out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    let header_y = lines
        .iter()
        .position(|line| line.trim() == "Alpha")
        .expect("expected the artist header row");
    assert_eq!(
        art_rect.y, header_y as u16,
        "album artwork should start on the selected block's artist row"
    );
    assert!(
        art_rect.x + art_rect.width == 58,
        "album art should have two columns of right padding"
    );
    assert!(app.card_image_loading.contains("album-1:P"));
    assert!(!app.card_image_loading.contains("track-1:P"));
}

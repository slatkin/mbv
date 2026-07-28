use super::test_helpers::*;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab, QueueScope, RemoteSlotState};

#[test]
fn album_folder_listing_fetches_and_shows_loading_on_cache_miss() {
    let mut app = make_power_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second_album);
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
    assert!(
        lines[title_y - 4].trim().is_empty(),
        "expected the colored top-padding row above the artist header to be blank:\n{out}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{out}"
    );

    let loading_y = lines
        .iter()
        .position(|l| l.to_lowercase().contains("loading"))
        .expect("expected an inline loading row");
    assert!(
        loading_y > title_y,
        "expected the loading row to render below the selected album title:\n{out}"
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y > loading_y,
        "expected the inline loading row to render before sibling albums:\n{out}"
    );

    let title_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(0))
        .expect("expected the selected album (index 0) in the row map");
    let second_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(1))
        .expect("expected the following album (index 1) in the row map");
    assert!(
        second_row_idx > title_row_idx,
        "expected the following album's row-map entry after the selected album's"
    );
    assert!(
        layout.left_row_map[title_row_idx + 1..second_row_idx]
            .iter()
            .all(Option::is_none),
        "expected every row between the two albums (borders, padding, loading row) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
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

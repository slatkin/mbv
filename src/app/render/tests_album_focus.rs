use super::test_helpers::*;
use super::*;
use crate::app::tests::make_item;

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

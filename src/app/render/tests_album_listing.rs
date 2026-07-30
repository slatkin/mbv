use super::test_helpers::*;
use super::*;
use crate::app::layout::LibraryRowTarget;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab};

#[test]
fn album_folder_listing_renders_list_and_inline_detail_together() {
    let mut app = make_power_music_group_app();
    assert_eq!(app.libs[0].nav_stack.len(), 2);

    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second_album);

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    app.libs[0].album_track_focus = Some(0);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let lines: Vec<&str> = out.lines().collect();

    assert!(
        out.contains("Alpha"),
        "expected the album list (grouped by artist) to still render:\n{out}"
    );
    assert!(
        out.contains("Opening Track"),
        "expected the selected album's cached tracks to render inline, \
         without any drilldown:\n{out}"
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

    let track_y = lines
        .iter()
        .position(|l| l.contains("Opening Track"))
        .expect("expected inline track row");
    assert!(
        track_y > title_y,
        "expected the track row to render below the selected album title:\n{out}"
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y > track_y,
        "expected the inline track detail to render before sibling albums:\n{out}"
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
        "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
    assert_eq!(
        app.libs[0].nav_stack.len(),
        2,
        "rendering the inline preview must not push a nav_stack level"
    );
}

#[test]
fn flat_album_folder_listing_renders_inline_detail_under_selected_album() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.music_levels = vec!["album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let mut album = make_item("First Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = "Alpha".into();
    album.is_folder = true;
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    second_album.is_folder = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-music".into(),
            title: "Music".into(),
            items: vec![album, second_album],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let lines: Vec<&str> = out.lines().collect();

    let title_y = lines
        .iter()
        .position(|l| l.contains("First Album"))
        .expect("expected selected album title row");
    assert!(
        lines[title_y - 1].trim().is_empty(),
        "expected the colored top-padding row directly above the title to be blank:\n{out}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{out}"
    );

    let track_y = lines
        .iter()
        .position(|l| l.contains("Opening Track"))
        .expect("expected inline track row");
    assert!(
        track_y > title_y,
        "expected the track row to render below the selected album title:\n{out}"
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y > track_y,
        "expected the following album to render after the inline track detail:\n{out}"
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
        "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
    assert!(
        layout
            .left_row_targets
            .iter()
            .all(|target| !matches!(target, Some(LibraryRowTarget::ArtistHeader(_)))),
        "flat/non-custom grouped album headers must remain non-selectable"
    );
}

#[test]
fn searched_album_listing_does_not_duplicate_artist_row_in_plain_framing() {
    let mut app = make_power_music_group_app();
    let items = app.libs[0].nav_stack.last().unwrap().items.clone();
    app.libs[0].search = Some(LibSearch {
        query: "First Album".into(),
        items,
        results: vec![0],
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    let out = render_power_library_to_string(&mut app, &mut LayoutMain::default());

    assert_eq!(
        out.lines().filter(|line| line.trim() == "Alpha").count(),
        1,
        "search-result album framing must not duplicate the artist name:\n{out}"
    );
}

use super::album::AlbumRowsCursorCtx;
use super::album_plan::{sorted_group_album_order, GroupedAlbumDisplayRow, HeaderFocusCtx};
use super::test_helpers::*;
use super::*;
use crate::app::layout::LibraryRowTarget;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn selectable_artist_headers_are_typed_row_targets() {
    let mut app = make_music_group_app();
    let mut alpha_album2 = make_item("Second Alpha Album", "MusicAlbum");
    alpha_album2.id = "album-1b".into();
    alpha_album2.artist = "Alpha".into();
    alpha_album2.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(alpha_album2);
    let mut beta_album = make_item("Beta Album", "MusicAlbum");
    beta_album.id = "album-2".into();
    beta_album.artist = "Beta".into();
    beta_album.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(beta_album);

    let mut layout = LayoutMain::default();
    let out = render_library_to_string(&mut app, &mut layout);

    assert!(
        out.contains("Alpha") && out.contains("Beta"),
        "expected both artist headers to render:\n{out}"
    );
    let header_row = layout
        .left_row_targets
        .iter()
        .position(|target| {
            matches!(
                target,
                Some(LibraryRowTarget::ArtistHeader(selection))
                    if selection.artist_label == "Alpha"
                        && selection.first_album_id == "album-1"
            )
        })
        .expect("expected the custom artist header to be a typed row target");
    assert_eq!(
        layout.left_row_map[header_row], None,
        "legacy row map must keep headers non-album rows"
    );
}

#[test]
fn artist_and_album_focus_share_one_selected_group_bounds() {
    let mut app = make_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    app.libs[0].nav_stack.last_mut().unwrap().items.push(second);
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let header = crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Alpha".into(),
    };

    let album_plan = {
        let album_info = app.group_album_info(&albums, None);
        let order = sorted_group_album_order(&album_info);
        app.build_grouped_album_display_plan(
            &albums,
            &album_info,
            &order,
            0,
            false,
            HeaderFocusCtx {
                selectable_headers: true,
                selected_artist_header: None,
                expand_selected: false,
            },
            Some((120, 0)),
            false, // hero_handles_detail
        )
    };
    let header_plan = {
        let album_info = app.group_album_info(&albums, None);
        let order = sorted_group_album_order(&album_info);
        app.build_grouped_album_display_plan(
            &albums,
            &album_info,
            &order,
            0,
            false,
            HeaderFocusCtx {
                selectable_headers: true,
                selected_artist_header: Some(&header),
                expand_selected: false,
            },
            Some((120, 0)),
            false, // hero_handles_detail
        )
    };

    assert_eq!(
        album_plan.selected_block_bounds, header_plan.selected_block_bounds,
        "header and album focus should use the same artist-scoped frame"
    );
    assert_eq!(
        album_plan
            .rows
            .iter()
            .filter(|row| matches!(row, GroupedAlbumDisplayRow::Album(_)))
            .count(),
        2,
        "the selected group should emit the complete discography"
    );
}

#[test]
fn focused_group_header_has_no_internal_spacer_when_hero_handles_detail() {
    let mut app = make_music_group_app();
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let album_info = app.group_album_info(&albums, None);
    let order = sorted_group_album_order(&album_info);
    let plan = app.build_grouped_album_display_plan(
        &albums,
        &album_info,
        &order,
        0,
        false,
        HeaderFocusCtx {
            selectable_headers: true,
            selected_artist_header: None,
            expand_selected: false,
        },
        Some((120, 0)),
        true,
    );

    let header_row = plan
        .rows
        .iter()
        .position(|row| matches!(row, GroupedAlbumDisplayRow::ArtistHeader(_)))
        .expect("focused artist header should render");
    assert!(matches!(
        plan.rows.get(header_row + 1),
        Some(GroupedAlbumDisplayRow::Album(0))
    ));
}

#[test]
fn grouped_hero_art_follows_album_focus() {
    let mut album_app = make_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    album_app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second);
    album_app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    album_app.image_protocol_enabled = true;
    let mut layout = LayoutMain::default();
    // 60x30 so the list below the album hero still shows both albums.
    let out = render_library_to_string_sized(&mut album_app, &mut layout, 60, 30);
    assert!(out.contains("First Album"));
    // The hero renders the *selected* album's art (portrait `:P`), never a
    // square collage tile (`:sq`).
    assert!(album_app.card_image_loading.contains("album-2:P"));
    assert!(!album_app.card_image_loading.contains("album-2:sq"));

    // With an artist header focused, the cursor album still anchors the hero,
    // so the same portrait art is fetched -- no square collage in the hero flow.
    let mut header_app = make_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    header_app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second);
    header_app.image_protocol_enabled = true;
    header_app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Alpha".into(),
    });
    let mut header_layout = LayoutMain::default();
    let _header_out = render_library_to_string_sized(&mut header_app, &mut header_layout, 60, 30);
    assert!(header_app.card_image_loading.contains("album-1:P"));
    assert!(!header_app.card_image_loading.contains("album-1:sq"));
}

#[test]
fn two_column_album_groups_keep_spacer_above_next_group() {
    let mut app = make_music_group_app();
    app.music_levels[0] = "artist".into();
    app.album_tracks_cache.insert("album-1".into(), Vec::new());

    for (id, artist, name) in [
        ("album-2", "Beta", "Beta Album"),
        ("album-3", "Gamma", "Gamma Album"),
    ] {
        let mut album = make_item(name, "MusicAlbum");
        album.id = id.into();
        album.artist = artist.into();
        app.libs[0].nav_stack.last_mut().unwrap().items.push(album);
    }

    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let mut layout = LayoutMain::default();
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|f| {
            app.render_grouped_album_rows(
                f,
                Rect::new(0, 0, 80, 20),
                0,
                &albums,
                AlbumRowsCursorCtx {
                    cursor: 0,
                    stored_scroll: 0,
                },
                true,
                false,
                2,
                &mut layout,
            );
        })
        .unwrap();

    let rendered = buffer_to_string(&terminal);
    let lines: Vec<&str> = rendered.lines().collect();
    for artist in ["Beta", "Gamma"] {
        let row = lines
            .iter()
            .position(|line| line.trim() == artist)
            .expect("artist header should render");
        assert!(
            row > 0 && lines[row - 1].trim().is_empty(),
            "expected a spacer above {artist}:\n{}",
            lines.join("\n")
        );
    }
}

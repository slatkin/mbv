use super::album_plan::{sorted_group_album_order, GroupedAlbumDisplayRow};
use super::test_helpers::*;
use super::*;
use crate::app::layout::LibraryRowTarget;
use crate::app::tests::make_item;

#[test]
fn selectable_artist_headers_are_typed_row_targets() {
    let mut app = make_power_music_group_app();
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
    let out = render_power_library_to_string(&mut app, &mut layout);

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
    let mut app = make_power_music_group_app();
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
            true,
            None,
            false,
            Some((120, 0)),
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
            true,
            Some(&header),
            false,
            Some((120, 0)),
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
fn grouped_target_marker_and_inline_art_follow_album_or_artist_focus() {
    let mut album_app = make_power_music_group_app();
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
    let out = render_power_library_to_string(&mut album_app, &mut layout);
    assert!(out.contains("First Album"));
    assert!(album_app.card_image_loading.contains("album-2:P"));
    assert!(!album_app.card_image_loading.contains("album-2:sq"));

    let mut header_app = make_power_music_group_app();
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
    let _header_out = render_power_library_to_string(&mut header_app, &mut header_layout);
    assert!(header_app.card_image_loading.contains("album-1:sq"));
}

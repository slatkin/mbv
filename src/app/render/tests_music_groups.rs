use super::album_plan::GroupedAlbumDisplayRow;
use super::test_helpers::*;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab, QueueScope, RemoteSlotState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

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
fn grouped_album_rows_use_styled_suffix_and_single_group_spacers() {
    let mut app = make_power_music_group_app();
    let mut alpha_album = make_item("Second Alpha Album", "MusicAlbum");
    alpha_album.id = "album-1b".into();
    alpha_album.artist = "Alpha".into();
    alpha_album.production_year = 2002;
    let mut beta_album = make_item("Beta Album", "MusicAlbum");
    beta_album.id = "album-2".into();
    beta_album.artist = "Beta".into();
    beta_album.production_year = 2003;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    level.items.extend([alpha_album, beta_album]);
    level.cursor = 2;

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();
    let alpha_y = lines
        .iter()
        .position(|line| line.contains("Alpha") && !line.contains("Album"))
        .expect("expected Alpha artist header");
    let beta_y = lines
        .iter()
        .position(|line| line.contains("Beta") && !line.contains("Album"))
        .expect("expected Beta artist header");
    let last_alpha_album_y = lines
        .iter()
        .rposition(|line| line.contains("Alpha Album"))
        .expect("expected the final Alpha album row");
    assert_eq!(
        beta_y,
        last_alpha_album_y + 4,
        "expected one spacer plus the selected group's frame before the next artist:\n{out}"
    );
    let last_selectable = layout
        .left_row_targets
        .iter()
        .rev()
        .find_map(Option::as_ref)
        .expect("expected a selectable album row");
    assert!(
        matches!(last_selectable, LibraryRowTarget::Album(_)),
        "expected no trailing artist spacer after the final album"
    );

    let album_y = lines
        .iter()
        .position(|line| line.contains("Second Alpha Album"))
        .expect("expected Alpha album row");
    let title_x = lines[album_y].find("Second Alpha Album").unwrap() as u16;
    let header_x = lines[alpha_y].find("Alpha").unwrap() as u16;
    assert_eq!(
        title_x, header_x,
        "album title should align with its header"
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

    let album_plan =
        app.build_grouped_album_display_plan(&albums, 0, false, true, None, false, Some((120, 0)));
    let header_plan = app.build_grouped_album_display_plan(
        &albums,
        0,
        false,
        true,
        Some(&header),
        false,
        Some((120, 0)),
    );

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

#[test]
fn long_inline_track_focus_keeps_the_detail_table_inside_the_selected_block() {
    let mut app = make_power_music_group_app();
    app.libs[0].album_track_focus = Some(29);
    let tracks: Vec<_> = (0..30)
        .map(|i| {
            let mut track = make_item(&format!("Track {i}"), "Audio");
            track.id = format!("track-{i}");
            track.album = "First Album".into();
            track.artist = "Alpha".into();
            track.index_number = i + 1;
            track
        })
        .collect();
    app.album_tracks_cache.insert("album-1".into(), tracks);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    assert!(
        out.contains("Track 29"),
        "expected focused track in the block:\n{out}"
    );
    assert!(layout.cursor_screen_y.is_some());
}

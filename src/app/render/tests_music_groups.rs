use super::album::AlbumRowsCursorCtx;
use super::album_plan::{sorted_group_album_order, GroupedAlbumDisplayRow, HeaderFocusCtx};
use super::test_helpers::*;
use super::*;
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
        .left_row_map
        .iter()
        .position(|item| item.is_none())
        .expect("expected a non-album row (artist header) in the row map");
    assert_eq!(
        layout.left_row_map[header_row], None,
        "legacy row map must keep headers non-album rows"
    );
    // Artist headers are display-only and must not appear as row targets.
    assert!(
        layout
            .left_row_targets
            .get(header_row)
            .is_none_or(|t| t.is_none()),
        "artist headers must not be navigation targets"
    );
}

#[test]
fn selected_group_has_block_bounds() {
    let mut app = make_music_group_app();
    let albums = app.libs[0].nav_stack.last_mut().unwrap();
    for idx in 2..=24 {
        let mut album = make_item(&format!("Album {idx}"), "MusicAlbum");
        album.id = format!("album-{idx}");
        album.artist = "Alpha".into();
        albums.items.push(album);
    }
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();

    let plan = {
        let album_info = app.group_album_info(&albums, None);
        let order = sorted_group_album_order(&album_info);
        app.build_grouped_album_display_plan(
            &albums,
            &album_info,
            &order,
            0,
            false,
            HeaderFocusCtx {
                in_music_group_view: true,
                expand_selected: false,
            },
            Some((120, 0)),
            false, // hero_handles_detail
        )
    };

    assert!(
        plan.selected_block_bounds.is_some(),
        "the selected group should have block bounds"
    );
    assert_eq!(
        plan.rows
            .iter()
            .filter(|row| matches!(row, GroupedAlbumDisplayRow::Album(_)))
            .count(),
        24,
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
            in_music_group_view: true,
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
fn hero_handles_detail_suppresses_all_inline_detail_rows() {
    let mut app = make_music_group_app();
    // Add tracks to the album so the plan would normally include detail rows.
    let tracks: Vec<mbv_core::api::EmbyItem> = (0..5)
        .map(|i| {
            let mut t = crate::app::tests::make_item(&format!("Track {}", i + 1), "Audio");
            t.id = format!("track-{}", i + 1);
            t.index_number = (i + 1) as i64;
            t
        })
        .collect();
    app.album_tracks_cache.insert("album-1".into(), tracks);

    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let album_info = app.group_album_info(&albums, None);
    let order = sorted_group_album_order(&album_info);

    // Without hero_handles_detail, detail rows should appear.
    let plan_without = app.build_grouped_album_display_plan(
        &albums,
        &album_info,
        &order,
        0,
        false,
        HeaderFocusCtx {
            in_music_group_view: true,
            expand_selected: true,
        },
        Some((120, 0)),
        false,
    );
    let has_detail = plan_without.rows.iter().any(|row| {
        matches!(
            row,
            GroupedAlbumDisplayRow::AlbumDetailStart(_)
                | GroupedAlbumDisplayRow::AlbumDetailContinuation
                | GroupedAlbumDisplayRow::AlbumDetailRule
                | GroupedAlbumDisplayRow::AlbumLoading
                | GroupedAlbumDisplayRow::AlbumActionHint
        )
    });
    assert!(
        has_detail,
        "without hero_handles_detail, inline detail rows should appear"
    );

    // With hero_handles_detail, no detail rows should appear.
    let plan_with = app.build_grouped_album_display_plan(
        &albums,
        &album_info,
        &order,
        0,
        false,
        HeaderFocusCtx {
            in_music_group_view: true,
            expand_selected: true,
        },
        Some((120, 0)),
        true,
    );
    let has_detail = plan_with.rows.iter().any(|row| {
        matches!(
            row,
            GroupedAlbumDisplayRow::AlbumDetailStart(_)
                | GroupedAlbumDisplayRow::AlbumDetailContinuation
                | GroupedAlbumDisplayRow::AlbumDetailRule
                | GroupedAlbumDisplayRow::AlbumLoading
                | GroupedAlbumDisplayRow::AlbumActionHint
        )
    });
    assert!(
        !has_detail,
        "with hero_handles_detail, inline detail rows should be suppressed"
    );
    assert!(
        plan_with.selected_block_bounds.is_none(),
        "selected_block_bounds should be None when hero_handles_detail is true"
    );
    assert!(
        plan_with.track_detail_bounds.is_none(),
        "track_detail_bounds should be None when hero_handles_detail is true"
    );
}

#[test]
fn collapsed_album_hero_does_not_stack_metadata_below_art() {
    assert_eq!(
        super::album_plan::album_hero_content_rows(
            0,
            super::album_art::INLINE_ALBUM_ART_ROWS,
            60,
            true,
        ),
        super::album_art::INLINE_ALBUM_ART_ROWS,
        "collapsed album art should reach the hero's bottom border without extra rows"
    );
}

#[test]
fn album_hero_sizing_grows_with_track_count() {
    let art_rows = super::album_art::INLINE_ALBUM_ART_ROWS;
    let panel_width = 40u16;
    let zero_tracks = super::album_plan::album_hero_content_rows(0, art_rows, panel_width, true);
    let twenty_tracks = super::album_plan::album_hero_content_rows(20, art_rows, panel_width, true);
    assert!(
        twenty_tracks > zero_tracks,
        "hero with 20 tracks ({twenty_tracks}) should be taller than with 0 ({zero_tracks})"
    );
    // Metadata rows (title + hint) are always included.
    assert!(
        zero_tracks >= 2,
        "even with no tracks, hero should include metadata rows"
    );
}

#[test]
fn album_hero_sizing_without_images_ignores_art() {
    let with_images = super::album_plan::album_hero_content_rows(3, 8, 60, true);
    let without_images = super::album_plan::album_hero_content_rows(3, 8, 60, false);
    assert!(
        without_images <= with_images,
        "without images, hero should not be taller"
    );
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
    assert!(
        album_app.card_image_loading.contains("album-1:P"),
        "the adjacent album should be pre-warmed while the selected album renders"
    );
    assert!(!album_app.card_image_loading.contains("album-2:sq"));
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

#[test]
fn two_column_grouped_rows_keep_absolute_columns_after_scroll() {
    let mut app = make_music_group_app();
    for i in 1..4 {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-{i}");
        album.artist = "Alpha".into();
        album.is_folder = true;
        app.libs[0].nav_stack.last_mut().unwrap().items.push(album);
    }
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 2;
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let mut layout = LayoutMain::default();
    let mut terminal = Terminal::new(TestBackend::new(82, 2)).unwrap();
    terminal
        .draw(|f| {
            app.render_grouped_album_rows(
                f,
                Rect::new(0, 0, 82, 2),
                0,
                &albums,
                AlbumRowsCursorCtx {
                    cursor: 2,
                    stored_scroll: 0,
                },
                true,
                true,
                2,
                &mut layout,
            );
        })
        .unwrap();

    let rendered = buffer_to_string(&terminal);
    let lines: Vec<&str> = rendered.lines().collect();
    assert!(
        lines[1].contains("Album 02"),
        "the row containing the selected album should render after the scrolled packed row:\n{rendered}\noffset={} rows={:?} cursor_y={:?}",
        layout.left_screen_offset,
        layout.left_item_rows,
        layout.cursor_screen_y,
    );
    assert!(
        lines[1].starts_with('▎'),
        "the selected-row marker should follow the selected album after scrolling:\n{rendered}"
    );
    assert_eq!(layout.left_screen_offset, 1);
    assert_eq!(layout.left_item_rows[1], vec![0, 1]);
    assert_eq!(layout.left_item_rows[2], vec![2, 3]);
    assert_eq!(layout.cursor_screen_y, Some(1));
}

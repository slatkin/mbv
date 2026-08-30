use super::components::album::AlbumRowsCursorCtx;
use super::components::album_detail::album_hero_detail_rows;
use super::components::hero::HERO_BLOCK_EXTRA_ROWS;
use super::screens::album_plan::{
    sorted_group_album_order, GroupedAlbumDisplayRow, HeaderFocusCtx,
};
use super::test_helpers::*;
use super::*;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
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
    // Artist headers are display-only and must not appear as row targets.
    // Music-group view renders through `render_wide_right_album_browser`
    // (shared with the wide hero-on-left layout), which populates
    // `left_row_targets` directly rather than the legacy `left_row_map`.
    assert!(
        layout.left_row_targets.iter().any(|t| t.is_none()),
        "expected a non-album row (artist header) in the row targets"
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
        23,
        "the selected album row is replaced while the rest of the discography remains"
    );
    assert!(
        plan.rows
            .iter()
            .any(|row| matches!(row, GroupedAlbumDisplayRow::AlbumInlineDetailStart(0))),
        "the selected album should own the inline detail block"
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
    // The shared replacement plan consumes the selected album source row,
    // which still directly follows the artist header with no spacer.
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
fn hero_handling_drops_hint_wrap_rows_but_keeps_album_title_rows() {
    let mut app = make_music_group_app();
    let mut albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    albums[0].name = "A deliberately long album title that wraps".into();
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
        Some((30, 0)),
        true,
    );

    assert_eq!(
        plan.rows
            .iter()
            .filter(|row| matches!(row, GroupedAlbumDisplayRow::AlbumWrappedContinuation))
            .count(),
        2,
        "album title wrapping must remain while hint wrapping is removed"
    );
    assert!(plan
        .rows
        .iter()
        .any(|row| matches!(row, GroupedAlbumDisplayRow::Album(0))));
}

#[test]
fn narrow_grouped_music_replaces_selected_album_row_with_hero_detail() {
    // Task 3.2: the selected album's row is replaced by the Model A hero
    // (title/meta/art), not an inline track table -- see
    // `tests_music_characterization.rs` for the text-level assertion that
    // the track table and action hint no longer render.
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
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 60, 30);

    assert!(
        output.contains("First Album"),
        "selected album hero must render its title"
    );
    assert_eq!(
        layout
            .left_row_targets
            .iter()
            .filter(|target| {
                matches!(target, Some(crate::app::layout::LibraryRowTarget::Album(0)))
            })
            .count(),
        1,
        "the selected album must publish one replacement parent target"
    );
    let hero_marker = layout
        .hero_area
        .y
        .checked_sub(0)
        .and_then(|y| output.lines().nth(y as usize))
        .and_then(|line| {
            line.chars()
                .nth(layout.left_area.x.saturating_sub(2) as usize)
        });
    assert_ne!(
        hero_marker,
        Some('\u{258e}'),
        "the shared replacement plan suppresses the ordinary marker over its hero"
    );
}

#[test]
fn narrow_grouped_music_does_not_repaint_album_hero_with_zero_row_shell() {
    let mut app = make_music_group_app();
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 60, 30);

    let top_row = output
        .lines()
        .nth(layout.hero_area.y as usize)
        .unwrap_or_default();
    let bottom_row = output
        .lines()
        .nth(layout.hero_area.bottom().saturating_sub(1) as usize)
        .unwrap_or_default();
    assert!(
        top_row.contains('▁'),
        "album hero top border missing: {top_row:?}"
    );
    assert!(
        bottom_row.contains('▔'),
        "album hero bottom border missing: {bottom_row:?}"
    );
}

#[test]
fn narrow_grouped_music_keeps_bottom_hero_fully_visible() {
    let mut app = make_music_group_app();
    for i in 2..=12 {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-{i}");
        album.artist = "Alpha".into();
        app.libs[0].nav_stack.last_mut().unwrap().items.push(album);
    }
    app.image_protocol_enabled = true;
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let cursor = albums.len() - 1;
    app.libs[0].nav_stack.last_mut().unwrap().cursor = cursor;
    let expected_height = album_hero_detail_rows(true) + HERO_BLOCK_EXTRA_ROWS as usize;
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 60, 26);

    assert_eq!(layout.hero_area.height as usize, expected_height);
    assert!(layout.hero_area.y > layout.left_area.y);
    assert_eq!(layout.hero_area.bottom(), layout.left_area.bottom());
    assert_eq!(layout.selected_item_rect, Some(layout.hero_area));
    let selected_row = layout
        .left_item_rows
        .iter()
        .position(|row| row == &vec![cursor])
        .expect("the selected source row becomes the parent hero row");
    assert_eq!(
        layout
            .left_row_targets
            .iter()
            .filter(|target| {
                matches!(target, Some(crate::app::layout::LibraryRowTarget::Album(idx)) if *idx == cursor)
            })
            .count(),
        1,
        "the admitted hero publishes exactly one selected parent target"
    );
    let continuation_end = selected_row + expected_height;
    assert!(layout.left_item_rows.len() >= continuation_end);
    assert!(layout.left_item_rows[selected_row + 1..continuation_end]
        .iter()
        .all(Vec::is_empty));
    let selected_screen_row = layout.hero_area.y.saturating_sub(layout.left_area.y) as usize;
    let target_end = selected_screen_row + expected_height;
    assert!(layout.left_row_targets.len() >= target_end);
    assert!(layout.left_row_targets[selected_screen_row + 1..target_end]
        .iter()
        .all(Option::is_none));

    let marker_col = layout.left_area.x.saturating_sub(2) as usize;
    for y in layout.hero_area.y..layout.hero_area.bottom() {
        let marker = output
            .lines()
            .nth(y as usize)
            .and_then(|line| line.chars().nth(marker_col));
        assert_ne!(
            marker,
            Some('\u{258e}'),
            "ordinary marker painted over hero at y={y}"
        );
    }
}

#[test]
fn narrow_grouped_music_persists_bottom_hero_scroll() {
    let mut app = make_music_group_app();
    for i in 2..=12 {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-{i}");
        album.artist = "Alpha".into();
        app.libs[0].nav_stack.last_mut().unwrap().items.push(album);
    }
    app.image_protocol_enabled = true;
    let cursor = app.libs[0].nav_stack.last().unwrap().items.len() - 1;
    app.libs[0].nav_stack.last_mut().unwrap().cursor = cursor;
    let mut layout = LayoutMain::default();
    render_library_to_string_sized(&mut app, &mut layout, 60, 26);

    let stored_scroll = app.libs[0].nav_stack.last().unwrap().scroll;
    assert!(stored_scroll > 0, "the admitted hero offset must persist");
    assert_eq!(layout.selected_item_rect, Some(layout.hero_area));
    assert!(layout.hero_area.bottom() <= layout.left_area.bottom());

    render_library_to_string_sized(&mut app, &mut layout, 60, 26);
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().scroll,
        stored_scroll,
        "the computed hero scroll remains persisted on the next render"
    );
}

#[test]
fn short_grouped_music_restores_the_ordinary_selected_album_row() {
    let mut app = make_music_group_app();
    app.image_protocol_enabled = true;
    let expected_height = album_hero_detail_rows(true) + HERO_BLOCK_EXTRA_ROWS as usize;
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(
        &mut app,
        &mut layout,
        60,
        expected_height.saturating_sub(1) as u16,
    );

    assert!(output.contains("First Album"));
    assert_eq!(layout.hero_area, Rect::default());
    let selected = layout
        .selected_item_rect
        .expect("the ordinary selected album row remains targetable");
    assert_ne!(selected, layout.hero_area);
    assert!(layout
        .left_row_targets
        .iter()
        .any(|target| matches!(target, Some(crate::app::layout::LibraryRowTarget::Album(0)))));
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
    // Music-group view's album rows now render through the same
    // `render_wide_right_album_browser` the wide hero-on-left layout uses
    // for its right pane, which does not pre-warm neighbouring albums'
    // art (only the selected album's hero art loads) -- matching wide's
    // existing behaviour rather than narrow's former bespoke prefetch.
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
fn grouped_music_shared_plan_keeps_one_parent_target() {
    let mut app = make_music_group_app();
    for i in 1..6 {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-{i}");
        album.artist = "Alpha".into();
        album.is_folder = true;
        app.libs[0].nav_stack.last_mut().unwrap().items.push(album);
    }
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 2;
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let mut layout = LayoutMain::default();
    let mut terminal = Terminal::new(TestBackend::new(82, 30)).unwrap();
    terminal
        .draw(|f| {
            app.render_grouped_album_rows(
                f,
                Rect::new(0, 0, 82, 30),
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

    assert!(
        layout
            .left_row_targets
            .iter()
            .filter(|target| {
                matches!(target, Some(crate::app::layout::LibraryRowTarget::Album(2)))
            })
            .count()
            == 1,
        "the shared plan publishes one selected parent target: {:?}",
        layout.left_row_targets,
    );
}

#[test]
fn grouped_music_falls_back_when_selected_album_source_is_absent() {
    let mut app = make_music_group_app_with_second_album();
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let absent_cursor = albums.len() + 3;
    let mut layout = LayoutMain::default();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();

    terminal
        .draw(|f| {
            app.render_grouped_album_rows(
                f,
                Rect::new(0, 0, 60, 20),
                0,
                &albums,
                AlbumRowsCursorCtx {
                    cursor: absent_cursor,
                    stored_scroll: 0,
                },
                true,
                true,
                1,
                &mut layout,
            );
        })
        .unwrap();

    let rendered = buffer_to_string(&terminal);
    assert!(rendered.contains("First Album"));
    assert!(rendered.contains("Second Album"));
    assert_eq!(layout.hero_area, Rect::default());
    assert_eq!(layout.selected_item_rect, None);
}

#[test]
fn grouped_music_maps_reordered_non_contiguous_album_source() {
    let mut app = make_music_group_app();
    let mut beta = make_item("Beta Album", "MusicAlbum");
    beta.id = "album-beta".into();
    beta.artist = "Beta".into();
    let mut alpha_other = make_item("Alpha Other", "MusicAlbum");
    alpha_other.id = "album-alpha-other".into();
    alpha_other.artist = "Alpha".into();
    let mut selected = make_item("Selected Album", "MusicAlbum");
    selected.id = "album-selected".into();
    selected.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .extend([beta, alpha_other, selected]);
    let cursor = 3;
    app.libs[0].nav_stack.last_mut().unwrap().cursor = cursor;
    let mut layout = LayoutMain::default();
    let rendered = render_library_to_string_sized(&mut app, &mut layout, 60, 20);

    assert!(rendered.contains("Selected Album"));
    assert_eq!(layout.selected_item_rect, Some(layout.hero_area));
    assert_eq!(
        layout
            .left_row_targets
            .iter()
            .filter(|target| {
                matches!(target, Some(crate::app::layout::LibraryRowTarget::Album(3)))
            })
            .count(),
        1
    );
    assert_eq!(
        layout
            .left_item_rows
            .iter()
            .filter(|row| row.as_slice() == [3])
            .count(),
        1
    );
}

#[test]
fn wide_music_frame_publishes_identical_geometry_from_publish_and_paint() {
    // The paint path must consume the arrangement returned by
    // `publish_geometry` rather than recomputing it: the pure arrangement
    // math runs once per wide frame and both passes produce the same
    // geometry.
    let app = make_music_group_app();
    let app2 = make_music_group_app();

    let mut publish_layout = LayoutMain::default();
    let mut paint_layout = LayoutMain::default();

    let ctx = app.wide_music_render_ctx(0, None);
    let published = ctx
        .publish_geometry(Rect::new(0, 0, 120, 24), &mut publish_layout)
        .expect("wide area publishes panes");

    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
    terminal
        .draw(|f| {
            render_wide_music_group_with_ctx(
                f,
                Rect::new(0, 0, 120, 24),
                &app2.wide_music_render_ctx(0, None),
                &mut paint_layout,
            );
        })
        .unwrap();

    let (published_panes, published_left) = published;
    assert_eq!(published_panes.left_area, paint_layout.left_area);
    assert_eq!(
        published_panes.right_area,
        paint_layout.wide_music_right_area
    );
    assert_eq!(published_left.hero_area, paint_layout.hero_area);
    assert_eq!(published_left.art_area, paint_layout.wide_music_art_area);
    assert_eq!(publish_layout.wide_music_area, paint_layout.wide_music_area);
    assert_eq!(publish_layout.left_area, paint_layout.left_area);
    assert_eq!(
        publish_layout.wide_music_right_area,
        paint_layout.wide_music_right_area
    );
    assert_eq!(
        publish_layout.wide_music_art_area,
        paint_layout.wide_music_art_area
    );
    assert_eq!(publish_layout.hero_area, paint_layout.hero_area);
}

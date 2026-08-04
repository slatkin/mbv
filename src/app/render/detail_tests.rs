use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn content_rows_is_never_shorter_than_the_rendered_image_height() {
    let short_text_layout = CompactBannerLayout {
        meta_line: None,
        show_playing: false,
        lines: vec!["A short overview.".to_string()],
        director_line_idx: None,
        img_actual_w: 18,
        img_height: 12,
        img_is_placeholder: false,
    };
    assert_eq!(
        short_text_layout.content_rows(),
        12,
        "banner must reserve at least the image's height even when the \
         wrapped text alone would need far fewer rows"
    );

    let tall_text_layout = CompactBannerLayout {
        meta_line: Some("Crime  1974  1h33m".to_string()),
        show_playing: false,
        lines: vec!["line".to_string(); 20],
        director_line_idx: None,
        img_actual_w: 18,
        img_height: 12,
        img_is_placeholder: false,
    };
    assert_eq!(
        tall_text_layout.content_rows(),
        21,
        "when the text is taller than the image, the image must not \
         clip the banner back down to its own height"
    );

    let no_image_layout = CompactBannerLayout {
        meta_line: None,
        show_playing: false,
        lines: vec!["A short overview.".to_string()],
        director_line_idx: None,
        img_actual_w: 0,
        img_height: 0,
        img_is_placeholder: false,
    };
    assert_eq!(
        no_image_layout.content_rows(),
        1,
        "with no image (e.g. images disabled), sizing stays text-only"
    );
}

fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render_power_compact_detail_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    render_power_compact_detail_to_string_sized(app, layout, 60, 16)
}

fn render_power_compact_detail_to_string_sized(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_compact_detail(f, Rect::new(0, 0, width, height), 0, true, layout);
    })
    .unwrap();
    buffer_to_string(&term)
}

fn push_movie_lib(app: &mut App, movie: mbv_core::api::MediaItem) {
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![movie],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
}

#[test]
fn compact_movie_detail_reserves_placeholder_space_while_image_loads() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let _out = render_power_compact_detail_to_string(&mut app, &mut layout);

    assert!(
        app.card_image_loading.contains("movie-1:cmp_primary"),
        "expected the poster fetch to have been triggered and still be in flight"
    );
    assert!(
        !app.card_image_states.contains_key("movie-1:cmp_primary"),
        "fetch must not have resolved yet for this assertion to be meaningful"
    );
    assert_eq!(
        layout.inline_image_rect.map(|r| (r.width, r.height)),
        Some((24, 14)),
        "expected the placeholder to reserve the banner's IMG_COLS x IMG_ROWS box"
    );
}

#[test]
fn compact_movie_detail_reserves_placeholder_space_even_during_the_nav_idle_window() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    app.last_power_library_nav_at = std::time::Instant::now();
    assert!(!app.power_right_panel_image_renders_allowed());

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let _out = render_power_compact_detail_to_string(&mut app, &mut layout);

    assert_eq!(
        layout.inline_image_rect.map(|r| (r.width, r.height)),
        Some((24, 14)),
        "expected the placeholder to be reserved on the same frame as the rest of \
         the banner's content, even while the nav-idle gate is still closed"
    );
}

#[test]
fn compact_movie_detail_placeholder_matches_typical_poster_aspect_ratio() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let _out = render_power_compact_detail_to_string(&mut app, &mut layout);

    assert_eq!(
        layout.inline_image_rect.map(|r| (r.width, r.height)),
        Some((19, 14)),
        "expected the placeholder to match a typical 2:3 poster's fitted \
         width at this font size, not the full IMG_COLS bounding box"
    );
}

use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// Regression test for a bug caught by manual review after #263 shipped:
// a movie with a short overview but a rendered poster image reserved
// only enough banner rows for the text, so the image (drawn at its own
// fixed height regardless of text length) spilled past the banner's row
// budget into the list rows below it. `content_rows()` must never
// return fewer rows than the image needs, even when the text alone
// would ask for less.
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

// Alt+M's full-screen detail view (`render_power_detail`) was removed in
// #204: the compact banner (`render_power_compact_detail`) is now the
// single movie-detail surface, so this exercises that instead. The
// "enter prompt" assertions predate both surfaces having ever shown one;
// kept as a regression guard.
#[test]
fn compact_movie_detail_shows_director_without_enter_prompt() {
    let mut app = make_app_stub();

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A long-form overview for the compact movie detail banner.".into();
    movie.director = "Jane Director".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let out = render_power_compact_detail_to_string(&mut app, &mut layout);

    assert!(
        out.contains("Director: Jane Director"),
        "expected director:\n{out}"
    );
    assert!(
        !out.contains("Press"),
        "enter prompt should be removed:\n{out}"
    );
    assert!(
        !out.contains("[ENTER]"),
        "enter prompt should be removed:\n{out}"
    );
}

// #263: a short overview must render fully with no scrollbar, using only
// as many rows as its wrapped content actually needs.
#[test]
fn compact_movie_detail_shows_full_short_overview_with_no_scrollbar() {
    let mut app = make_app_stub();

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    movie.director = "Jane Director".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let out = render_power_compact_detail_to_string(&mut app, &mut layout);

    assert!(
        out.contains("A short overview."),
        "expected full overview text:\n{out}"
    );
    assert!(
        out.contains("Director: Jane Director"),
        "expected director:\n{out}"
    );
    assert!(
        !out.contains('\u{2590}'),
        "no banner scrollbar should be drawn:\n{out}"
    );
}

// The poster fetch is triggered synchronously inside `compact_banner_layout`
// but resolves asynchronously on a background thread; nothing drains that
// result in this test, so right after the render the fetch is still "in
// flight" (`card_image_loading` contains the key, `card_image_states`
// does not yet). The banner must reserve the same IMG_COLS x IMG_ROWS box
// the loaded image would use, not collapse to zero width.
#[test]
fn compact_movie_detail_reserves_placeholder_space_while_image_loads() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let out = render_power_compact_detail_to_string(&mut app, &mut layout);

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
        "expected the placeholder to reserve the banner's IMG_COLS x IMG_ROWS box:\n{out}"
    );
}

// The rest of the banner's content (meta line, overview text) is never
// gated on `last_power_library_nav_at` -- it renders at its final layout on the very
// first frame after navigating to a movie. The poster placeholder must
// match that: reserved on the same first frame, not held back until
// `power_right_panel_image_renders_allowed()`'s 150ms nav-idle window has passed.
// Gating the placeholder behind that timer (inherited from the timer's
// original purpose -- avoiding real-image flicker while rapidly
// scrolling through many different posters) produced a small but real
// desync where the description text appeared immediately but the grey
// box visibly lagged behind it by a beat.
#[test]
fn compact_movie_detail_reserves_placeholder_space_even_during_the_nav_idle_window() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    // Simulate having just navigated: the nav-idle gate is still closed.
    app.last_power_library_nav_at = std::time::Instant::now();
    assert!(!app.power_right_panel_image_renders_allowed());

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let out = render_power_compact_detail_to_string(&mut app, &mut layout);

    assert_eq!(
        layout.inline_image_rect.map(|r| (r.width, r.height)),
        Some((24, 14)),
        "expected the placeholder to be reserved on the same frame as the rest of \
         the banner's content, even while the nav-idle gate is still closed:\n{out}"
    );
}

// With no `image_picker` set up (as in every other test in this file --
// `make_app_stub` leaves it `None`), the placeholder falls back to the
// full IMG_COLS x IMG_ROWS bounding box, since there's no real font
// metrics yet to fit a poster's aspect ratio against. Once a picker is
// available, the placeholder should narrow to match what a real 2:3
// poster would actually resolve to at that font size -- reserving the
// full bounding box was 2 columns wider than any real poster ever
// renders at, causing a second, smaller reflow when the real image
// swapped in even after the nav-idle-gate fix above.
#[test]
fn compact_movie_detail_placeholder_matches_typical_poster_aspect_ratio() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    // `halfblocks()` needs no real terminal query and uses a fixed,
    // documented 10x20px font size -- exactly what the width math below
    // assumes.
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "A short overview.".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    let out = render_power_compact_detail_to_string(&mut app, &mut layout);

    // IMG_COLS x IMG_ROWS = 24 x 14 cells at a 10x20px font is a
    // 240x280px box. Fitting a 2:3 poster into that box is
    // height-constrained (280/3 < 240/2), giving a fitted
    // 187x280px image -> ceil(187/10) x ceil(280/20) = 19 x 14 cells.
    assert_eq!(
        layout.inline_image_rect.map(|r| (r.width, r.height)),
        Some((19, 14)),
        "expected the placeholder to match a typical 2:3 poster's fitted \
         width at this font size, not the full IMG_COLS bounding box:\n{out}"
    );
}

// #263: a long overview (well past what any fixed-height budget could
// show) must still render its full text and full director in one pass,
// with no scrollbar and no truncation, given a tall enough panel.
#[test]
fn compact_movie_detail_shows_full_long_overview_with_no_scrollbar() {
    let mut app = make_app_stub();

    let mut movie = make_item("Focused Movie", "Movie");
    movie.id = "movie-1".into();
    movie.overview = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. "
        .repeat(12);
    movie.director = "Very Distinctive Unique Director Name".into();
    push_movie_lib(&mut app, movie);

    let mut layout = LayoutMain::default();
    // Tall enough that the whole grown banner fits in the test buffer.
    let out = render_power_compact_detail_to_string_sized(&mut app, &mut layout, 60, 80);

    assert!(
        out.contains("Very Distinctive Unique Director Name"),
        "expected full director text with no scrolling:\n{out}"
    );
    assert!(
        !out.contains('\u{2590}'),
        "no banner scrollbar should be drawn:\n{out}"
    );
}

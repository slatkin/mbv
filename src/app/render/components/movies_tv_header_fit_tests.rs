use crate::app::layout::LayoutMain;
use crate::app::palette;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::App;
use crate::app::{BrowseLevel, LibraryTab, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Terminal;

fn render_list_term(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            Rect::new(0, 0, width, height),
        );
        app.render_list(f, Rect::new(0, 0, width, height), true, layout);
    })
    .unwrap();
    term
}

fn make_media_list_app(titles: Vec<&str>, collection_type: &str, item_type: &str) -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = collection_type.into();

    let items: Vec<_> = titles
        .into_iter()
        .enumerate()
        .map(|(i, title)| {
            let mut item = make_item(title, item_type);
            item.id = format!("movie-{i}");
            if title.contains("Selected") {
                item.overview = "This is the compact movie banner overview text.".into();
                item.production_year = 2024;
                item.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
            }
            item
        })
        .collect();
    let total = items.len();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items,
            total_count: total,
            cursor: total.saturating_sub(1),
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
        ..LibraryTab::new(library)
    });
    app
}

fn letter_grouped_titles() -> Vec<&'static str> {
    vec![
        "A Series 0",
        "A Series 1",
        "A Series 2",
        "A Series 3",
        "A Series 4",
        "A Series 5",
        "A Series 6",
        "A Series 7",
        "B Series 0",
        "B Series Selected",
    ]
}

fn assert_bottom_hero_fits_mini_render_list(app: &mut App) {
    let mut full_layout = LayoutMain::default();
    let _ = render_list_term(app, &mut full_layout, 60, 40);
    let hero_rows = full_layout.hero_area.height;
    assert!(
        hero_rows > 0,
        "the selected item should admit an inline hero"
    );

    // Letter-grouped libraries reserve two rows for their pill bar before the
    // list. Keep the list itself at hero_rows + one ordinary row.
    let mini_height = hero_rows + 3;
    let mut mini_layout = LayoutMain::default();
    let _ = render_list_term(app, &mut mini_layout, 60, mini_height);

    assert_eq!(
        mini_layout.hero_area.bottom(),
        mini_layout.left_area.bottom(),
        "the complete hero must remain visible at the bottom of the Mini list"
    );
    assert!(
        mini_layout.hero_area.y >= mini_layout.left_area.y,
        "the hero must not start above the Mini list"
    );
}

fn assert_movies_tv_pill_contract(collection_type: &str, item_type: &str, width: u16) {
    let mut app = make_media_list_app(
        vec!["Movie 0", "Movie 1 Selected"],
        collection_type,
        item_type,
    );
    app.libs[0].library_total = Some(1000);

    let mut layout = LayoutMain::default();
    let term = render_list_term(&mut app, &mut layout, width, 40);
    assert!(!layout.selector_tabs.is_empty());

    let pill_y = layout.selector_tabs[0].0.y;
    assert!(
        layout
            .selector_tabs
            .iter()
            .all(|(rect, _)| rect.y == pill_y && rect.height == 1),
        "{collection_type} pills must occupy one shared row"
    );
    assert_eq!(
        layout.is_wide_movies_active() || layout.is_wide_tv_active(),
        width >= 82,
        "{collection_type} must use the explicit narrow/wide presentation gate"
    );
    let is_wide = layout.is_wide_movies_active() || layout.is_wide_tv_active();
    if is_wide {
        assert_eq!(
            layout.left_area.y,
            pill_y + 3,
            "wide {collection_type} keeps one spacer row plus pane padding"
        );
    } else {
        assert_eq!(
            layout.left_area.y,
            pill_y + 2,
            "narrow {collection_type} must leave exactly one spacer row"
        );
    }
    assert_eq!(
        term.backend().buffer()[(layout.left_area.x, pill_y + 1)].bg,
        palette::SURFACE_BACKDROP,
        "{collection_type} spacer must retain the parent panel background"
    );
}

#[test]
fn plain_movies_bottom_hero_stays_inside_mini_render_list() {
    let mut app = make_media_list_app(
        vec![
            "Movie 0",
            "Movie 1",
            "Movie 2",
            "Movie 3",
            "Movie 4",
            "Movie Selected",
        ],
        "movies",
        "Movie",
    );
    assert_bottom_hero_fits_mini_render_list(&mut app);
}

#[test]
fn letter_grouped_movies_bottom_hero_stays_inside_mini_render_list() {
    let mut app = make_media_list_app(letter_grouped_titles(), "movies", "Movie");
    app.libs[0].library_total = Some(250);

    assert_bottom_hero_fits_mini_render_list(&mut app);
}

#[test]
fn plain_tv_bottom_hero_stays_inside_mini_render_list() {
    let mut app = make_media_list_app(
        vec![
            "Series 0",
            "Series 1",
            "Series 2",
            "Series 3",
            "Series 4",
            "Series Selected",
        ],
        "tvshows",
        "Series",
    );
    assert_bottom_hero_fits_mini_render_list(&mut app);
}

#[test]
fn letter_grouped_tv_bottom_hero_stays_inside_mini_render_list() {
    let mut app = make_media_list_app(letter_grouped_titles(), "tvshows", "Series");
    app.libs[0].library_total = Some(250);

    assert_bottom_hero_fits_mini_render_list(&mut app);
}

#[test]
fn movies_and_tv_pills_keep_one_row_and_spacer_in_narrow_and_wide_presentations() {
    // Note: wide Movies layout is now handled by BrowserComponent (5.3d.17a),
    // so this test only checks narrow Movies and both narrow/wide TV.
    for &(collection_type, item_type) in [("movies", "Movie"), ("tvshows", "Series")].iter() {
        assert_movies_tv_pill_contract(collection_type, item_type, 81);
        // Skip wide movies (width 120) — component handles it now
        if collection_type != "movies" {
            assert_movies_tv_pill_contract(collection_type, item_type, 120);
        }
    }
}

use super::test_helpers::{buffer_to_string, render_library_to_string_sized};
use super::*;
use crate::app::tests::make_app_stub;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
use crate::app::{PanelFocus, TabSelection};
use mbv_core::audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfShow,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn podcast_app() -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "podcasts".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = AudiobookshelfBrowseState::new(library.clone());
    state.append_page(
        0,
        20,
        3,
        vec![
            AudiobookshelfShow {
                library_item_id: "show-a".into(),
                title: "Alpha Podcast".into(),
                author: Some("Host Alpha".into()),
                description: Some("The Alpha show description.".into()),
                cover_path: Some("/cover/show-a".into()),
            },
            AudiobookshelfShow {
                library_item_id: "show-b".into(),
                title: "Beta Podcast".into(),
                author: Some("Host Beta".into()),
                description: None,
                cover_path: None,
            },
            AudiobookshelfShow {
                library_item_id: "show-c".into(),
                title: "Gamma Podcast".into(),
                author: None,
                description: None,
                cover_path: None,
            },
        ],
    );
    state.episodes = Some(vec![AudiobookshelfDownloadedEpisode {
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        title: "First Episode".into(),
        published_at: Some("2026-08-12".into()),
        duration_seconds: Some(1800.0),
    }]);
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

fn render_podcasts(app: &mut App, width: u16, height: u16) -> (Terminal<TestBackend>, LayoutMain) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|frame| {
            app.render_library(frame, Rect::new(0, 0, width, height), true, &mut layout);
        })
        .unwrap();
    (terminal, layout)
}

#[test]
fn podcast_hero_is_full_width_above_the_show_list_at_both_column_widths() {
    for width in [60, crate::app::TWO_COLUMN_THRESHOLD] {
        let mut app = podcast_app();
        let (_, layout) = render_podcasts(&mut app, width, 40);
        assert_eq!(layout.hero_area.x, 0);
        assert_eq!(layout.hero_area.width, width);
        assert!(layout.hero_area.height > 0);
        assert_eq!(layout.left_area.x, layout.hero_area.x);
        assert_eq!(layout.left_area.width, layout.hero_area.width);
        assert!(layout.left_area.y > layout.hero_area.y + layout.hero_area.height);
        assert_eq!(
            layout.left_item_rows.first().map(Vec::len),
            Some(if width < crate::app::TWO_COLUMN_THRESHOLD {
                1
            } else {
                2
            })
        );
    }
}

#[test]
fn podcast_hero_tracks_show_selection_without_moving() {
    let mut app = podcast_app();
    let (first, first_layout) = render_podcasts(&mut app, 82, 40);
    let first_output = buffer_to_string(&first);
    assert!(first_output.contains("Alpha Podcast"));
    assert!(first_output.contains("Host Alpha"));
    assert!(first_output.contains("The Alpha show description."));

    app.select_audiobookshelf_show(1);
    let (second, second_layout) = render_podcasts(&mut app, 82, 40);
    let second_output = buffer_to_string(&second);
    assert_eq!(first_layout.hero_area.x, second_layout.hero_area.x);
    assert_eq!(first_layout.hero_area.y, second_layout.hero_area.y);
    assert_eq!(first_layout.hero_area.width, second_layout.hero_area.width);
    assert!(first_layout.hero_area.height >= second_layout.hero_area.height);
    assert!(second_output.contains("Beta Podcast"));
    assert!(second_output.contains("Host Beta"));
    assert!(second_output.contains("Alpha Podcast"));
}

#[test]
fn episode_selection_uses_filter_pills_and_tv_style_episode_rows_in_the_hero() {
    let mut app = podcast_app();
    app.enter_audiobookshelf_episode_selection();
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 82, 40);
    assert!(output.contains("All"));
    assert!(output.contains("Played"));
    assert!(output.contains("Unplayed"));
    assert!(output.contains("First Episode"));
    assert!(!output.contains("1. First Episode"));
    assert_eq!(layout.selector_tabs.len(), 3);
    assert!(layout
        .selector_tabs
        .iter()
        .all(|(rect, _)| rect.y >= layout.hero_area.y
            && rect.y < layout.hero_area.y + layout.hero_area.height));
}

#[test]
fn tiny_height_suppresses_the_hero_and_leaves_the_show_list_usable() {
    let mut app = podcast_app();
    let (_, layout) = render_podcasts(&mut app, 60, 4);
    assert_eq!(layout.hero_area.height, 0);
    assert_eq!(layout.left_area.height, 4);
    assert!(!layout.left_item_rows.is_empty());
}

#[test]
fn selected_cover_reserves_the_tv_series_image_slot_while_loading() {
    let mut app = podcast_app();
    app.image_protocol_enabled = true;
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("https://books.example"),
    );
    let key = crate::app::images::audiobookshelf_cover_cache_key(
        "https://books.example",
        "show-a",
        app.current_protocol_suffix(),
    );
    app.card_image_loading.insert(key.clone());

    let (_, layout) = render_podcasts(&mut app, 82, 40);
    let image = layout.inline_image_rect.expect("cover placeholder");
    assert_eq!(image.width, super::detail_series::SERIES_IMAGE_COLS);
    assert_eq!(
        image.height,
        super::detail_series::SERIES_IMAGE_PLACEHOLDER_ROWS
    );
    assert!(layout.hero_area.contains(image.as_position()));
    assert!(app.card_image_loading.contains(&key));

    app.image_protocol_enabled = false;
    app.card_image_loading.clear();
    let (_, layout) = render_podcasts(&mut app, 82, 40);
    assert_eq!(layout.inline_image_rect, None);
    assert!(app.card_image_loading.is_empty());
}

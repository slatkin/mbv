use super::test_helpers::*;
use super::*;
use crate::app::render::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter;
use mbv_core::audiobookshelf::AudiobookshelfProgress;
use mbv_core::audiobookshelf::AudiobookshelfShow;

#[test]
fn wide_podcasts_use_a_left_hero_and_right_show_workspace() {
    let mut app = audiobookshelf_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 100, 30);

    assert!(
        layout.hero_area.width < 100,
        "wide hero must own a left pane"
    );
    assert!(
        layout.left_area.x > layout.hero_area.x,
        "show workspace belongs in the right pane"
    );
}

#[test]
fn narrow_podcasts_replace_selected_show_row_with_detail() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].shows[0].author = Some("Author A".into());
    let mut layout = LayoutMain::default();
    let terminal = render_library_to_terminal_focused(&mut app, &mut layout, true);

    assert!(
        layout.hero_area.height > 0,
        "selected show detail should render"
    );
    assert!(
        layout.hero_area.y >= layout.left_area.y,
        "narrow selected detail must own the show list row: hero={:?}, list={:?}",
        layout.hero_area,
        layout.left_area
    );

    let buffer = terminal.backend().buffer();
    let hero = layout.hero_area;
    assert_eq!(buffer[(hero.x, hero.y)].symbol(), "▁");
    assert_eq!(
        buffer[(hero.x, hero.y)].style().fg,
        Some(palette::SEEK_TRACK)
    );
    assert_eq!(
        buffer[(hero.x, hero.y + 1)].style().bg,
        Some(palette::resolve_surface_focus(true))
    );
    assert_eq!(
        buffer[(hero.x + SELECTED_BLOCK_SIDE_PADDING, hero.y + 2)].symbol(),
        "A",
        "podcast hero content must start two rows below the top border"
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].symbol(),
        "▔",
        "podcast hero bottom border must remain below the content"
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].style().fg,
        Some(palette::SEEK_TRACK)
    );
}

#[test]
fn narrow_podcast_detail_is_suppressed_when_the_viewport_is_too_short() {
    let mut app = audiobookshelf_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 3);

    assert_eq!(layout.hero_area.height, 0);
}

#[test]
fn narrow_podcast_replacement_owns_one_parent_target() {
    let mut app = audiobookshelf_app();
    let state = &mut app.audiobookshelf_browse[0];
    state.shows.extend((0..5).map(|index| AudiobookshelfShow {
        library_item_id: format!("show-{index}"),
        title: format!("Show {index}"),
        author: None,
        description: None,
        cover_path: None,
    }));
    state.select(2);
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 20);

    let selected_row = layout
        .left_item_rows
        .iter()
        .position(|row| row == &vec![2])
        .expect("selected replacement row should be mapped");
    assert_eq!(
        layout.left_item_rows[selected_row + 1..]
            .iter()
            .find_map(|row| row.first()),
        Some(&3)
    );
    assert_eq!(layout.hero_area.y as usize, selected_row);
}

#[test]
fn wide_podcast_detail_preserves_episode_rows_and_played_filtering() {
    let mut app = audiobookshelf_app();
    let state = &mut app.audiobookshelf_browse[0];
    state.episode_selection = Some(0);
    state.episode_filter = AudiobookshelfEpisodeFilter::Played;
    state.progress.insert(
        ("show-a".into(), "episode-a".into()),
        AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: true,
        },
    );
    let mut layout = LayoutMain::default();
    let out = render_library_to_string_sized(&mut app, &mut layout, 100, 30);

    assert!(layout.hero_area.x < layout.left_area.x);
    assert!(!layout.audiobookshelf_episode_rows.is_empty());
    assert!(out.contains("Episode A"));
}

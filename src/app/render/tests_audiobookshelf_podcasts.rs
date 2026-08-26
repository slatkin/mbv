use super::test_helpers::*;
use super::*;
use crate::app::components::AudiobookshelfPodcastComponent;
use crate::app::shell::Model;
use crate::app::PanelFocus;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Local podcast shell render harness (task 5.3d.10e, Unit A): size the
/// terminal and set both panel-focus fields so `effective_panel_focus()`
/// reports `focused` at any width (mini-widths read `mini_view_focus`, 80+
/// read `panel_focus`), mount and content-project the podcast component,
/// then — in one draw — render the legacy library into a separate local
/// `LayoutMain` (avoids borrowing `model.app` and `model.app.layout.main`
/// simultaneously) before painting the mounted component. Returns the model
/// (so a future unit can assert projected content/layout) and the terminal.
fn render_podcast_shell(
    app: crate::app::App,
    width: u16,
    height: u16,
    focused: bool,
) -> (Model, Terminal<TestBackend>) {
    let mut app = app;
    app.terminal_width = width;
    app.terminal_height = height;
    let focus = if focused {
        PanelFocus::Library
    } else {
        PanelFocus::Queue
    };
    // The effective focus is width-dependent; setting both fields keeps it
    // equal to `focused` at normal/wide and Mini sizes alike.
    app.panel_focus = focus;
    app.mini_view_focus = focus;

    let mut model = Model::new(app);
    model.sync_audiobookshelf_podcast();
    model.push_audiobookshelf_podcast_content();

    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|frame| {
        let mut layout = LayoutMain::default();
        model
            .app
            .render_library(frame, Rect::new(0, 0, width, height), focused, &mut layout);
        model.app.layout.main = layout;
        model.render_audiobookshelf_podcast_component(frame);
    })
    .unwrap();

    (model, term)
}
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter;
use mbv_core::audiobookshelf::AudiobookshelfProgress;
use mbv_core::audiobookshelf::AudiobookshelfShow;

#[test]
fn wide_podcasts_use_a_left_hero_and_right_show_workspace() {
    let app = audiobookshelf_app();
    let (model, _terminal) = render_podcast_shell(app, 100, 30, true);
    let layout = &model.app.layout.main;

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
    let (mut model, terminal) = render_podcast_shell(app, 60, 20, true);
    let layout = &model.app.layout.main;

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
        Some(palette::PROGRESS_TRACK)
    );
    assert_eq!(
        buffer[(hero.x, hero.y + 1)].style().bg,
        Some(palette::resolve_surface_focus(true))
    );
    assert_eq!(
        buffer[(hero.x + SELECTED_BLOCK_SIDE_PADDING, hero.y + 2)].symbol(),
        "S",
        "podcast hero title must start two rows below the top border"
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].symbol(),
        "▔",
        "podcast hero bottom border must remain below the content"
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].style().fg,
        Some(palette::PROGRESS_TRACK)
    );
    assert!(!layout.is_wide_podcast_active());

    // Repoint from the legacy `LayoutMain.audiobookshelf_episode_rows` to the
    // mounted component's painted geometry (task 5.3d.10, Unit D). Narrow
    // podcast details paint no episode rows, so the component owns an empty
    // `episode_rows`.
    let component_id = model
        .abs_podcast_id
        .as_ref()
        .expect("podcast component mounted");
    let episode_rows = model
        .application
        .get_component_mut(component_id)
        .and_then(|comp| {
            comp.as_any_mut()
                .downcast_mut::<AudiobookshelfPodcastComponent>()
        })
        .map(|component| component.geometry().episode_rows.clone())
        .expect("podcast component mounted");
    assert!(episode_rows.is_empty());
}

#[test]
fn narrow_podcast_panel_shows_one_alphabetical_pill_row() {
    let app = audiobookshelf_app();
    let (model, terminal) = render_podcast_shell(app, 60, 20, true);
    let layout = &model.app.layout.main;
    let buffer = terminal.backend().buffer();

    assert_surface_pills(
        &terminal,
        &layout,
        Rect::new(0, 0, 60, 20),
        1,
        ratatui::style::Color::Reset,
        &[0],
        &["⌘", "S–U"],
        0,
    );

    // "Show A" buckets under "S\u{2013}U".
    let pills_row = 0u16;
    let mut row_text = String::new();
    for x in 0..buffer.area().width {
        row_text.push_str(buffer[(x, pills_row)].symbol());
    }
    assert!(
        row_text.contains('\u{2318}'),
        "narrow panel's pill row must show the '⌘' prefix: {row_text:?}"
    );
    assert!(
        row_text.contains("S\u{2013}U"),
        "narrow panel's pill row must show the show's alphabetical bucket: {row_text:?}"
    );
    // Exactly one pill row: the row below it is the gap row before the list.
    let mut next_row_text = String::new();
    for x in 0..buffer.area().width {
        next_row_text.push_str(buffer[(x, pills_row + 1)].symbol());
    }
    assert!(
        !next_row_text.contains('\u{2318}'),
        "only one pill row should render in the panel: {next_row_text:?}"
    );
}

#[test]
fn narrow_podcast_detail_is_suppressed_when_the_viewport_is_too_short() {
    let app = audiobookshelf_app();
    let (model, _terminal) = render_podcast_shell(app, 60, 3, true);
    let layout = &model.app.layout.main;

    assert_eq!(layout.hero_area.height, 0);
}

#[test]
fn narrow_podcast_hero_reserves_description_rows_at_actual_width() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].shows[0].description = Some(
        "A deliberately long description that wraps beyond the old fixed estimator width.".into(),
    );
    let (model, _terminal) = render_podcast_shell(app, 30, 20, true);
    let layout = &model.app.layout.main;

    assert!(
        layout.hero_area.height >= 9,
        "narrow hero must reserve the wrapped description before painting: {:?}",
        layout.hero_area
    );
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
    let (mut model, _terminal) = render_podcast_shell(app, 60, 20, true);
    let layout = &model.app.layout.main;

    // Repoint from the legacy `LayoutMain.left_item_rows` to the mounted
    // component's painted geometry (task 5.3d.10e, Unit C). The selected
    // replacement no longer occupies a painted show-row: it owns exactly one
    // parent target -- the inline hero -- so it is absent from `geometry().
    // show_rows`, while the following show remains a painted source row below
    // the replacement hero.
    let component_id = model
        .abs_podcast_id
        .as_ref()
        .expect("podcast component mounted");
    let show_rows = model
        .application
        .get_component_mut(component_id)
        .and_then(|comp| {
            comp.as_any_mut()
                .downcast_mut::<AudiobookshelfPodcastComponent>()
        })
        .map(|component| component.geometry().show_rows.clone())
        .expect("podcast component mounted");

    let selected = 2usize;
    let following = 3usize;

    // The selected replacement owns exactly one parent target (the inline
    // hero), so it must not appear among the painted source rows.
    assert!(
        !show_rows.iter().any(|(_, index)| *index == selected),
        "selected replacement owns the hero, not a painted show row"
    );
    // The following show remains correctly mapped as a painted source row
    // below the replacement hero.
    let following_entry = show_rows
        .iter()
        .find(|(_, index)| *index == following)
        .expect("following show must remain mapped as a source row");
    assert!(
        following_entry.0.y > layout.hero_area.y,
        "following show must be mapped below the replacement hero: {:?} vs hero {:?}",
        following_entry.0,
        layout.hero_area
    );
    // The narrow panel reserves a one-row alphabetical bucket-pill row plus
    // a gap row above the show list (task 4.3), so the hero's absolute
    // screen row is offset from its list-relative `selected_row` index by
    // that reservation. Derive the list-relative selected row from the painted
    // source rows above the hero.
    let selected_row = show_rows
        .iter()
        .filter(|(rect, _)| rect.y < layout.hero_area.y)
        .count();
    assert_eq!(layout.hero_area.y as usize, selected_row + 2);
}

#[test]
fn audiobook_podcast_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for focused in [false, true] {
        let app = audiobookshelf_app();
        let (_, terminal) = render_podcast_shell(app, 60, 20, focused);
        let output = buffer_to_string(&terminal);
        assert!(
            output.contains("▁"),
            "hero shell missing in focused={focused}"
        );
    }

    let wide_app = audiobookshelf_app();
    let (_, wide_terminal) = render_podcast_shell(wide_app, 100, 30, true);
    let wide_output = buffer_to_string(&wide_terminal);
    assert!(
        wide_output.contains("Show A"),
        "selected show missing in wide output"
    );

    {
        let (width, height) = (40, 20);
        let app = audiobookshelf_app();
        let (_model, terminal) = render_podcast_shell(app, width, height, true);
        let output = buffer_to_string(&terminal);
        assert!(
            output.contains("▁"),
            "selected hero shell missing at {width}x{height}"
        );
    }
}

#[test]
fn narrow_podcast_detail_shows_author_description_no_pills_or_table() {
    // Before this migration (task 4.1's characterization), this same setup
    // rendered the author/description as a hand-painted block and showed
    // the in-hero " ⌘ " filter pill bar + episode table whenever
    // `episode_selection` was set. Task 4.2 makes author/description plain
    // `HeroLine`s and gates the pill bar + table on `persistent` (wide
    // only), so the narrow hero never shows them, even if
    // `episode_selection` is set (simulated here as a stale value -- Enter
    // no longer sets it in narrow mode, see `open_podcast_selection_modal`).
    let mut app = audiobookshelf_app();
    let state = &mut app.audiobookshelf_browse[0];
    state.shows[0].author = Some("Author A".into());
    state.shows[0].description = Some("A description of the show.".into());
    state.episode_selection = Some(0);
    let (model, terminal) = render_podcast_shell(app, 60, 20, true);
    let layout = &model.app.layout.main;
    let output = buffer_to_string(&terminal);

    assert!(
        output.contains("Author A"),
        "narrow hero renders the author as a standard hero line"
    );
    assert!(
        output.contains("description of the show"),
        "narrow hero renders the description as standard hero lines"
    );
    assert!(
        !output.contains("Played") && !output.contains("Unplayed"),
        "narrow hero must not show the in-hero filter pill bar"
    );
    assert!(
        !output.contains("Episode A"),
        "narrow hero must not show the episode table"
    );

    // The narrow panel's alphabetical bucket pills (task 4.3) legitimately
    // show '⌘' elsewhere on screen; the requirement is that none of it
    // renders inside the hero rect itself.
    let buffer = terminal.backend().buffer();
    let hero = layout.hero_area;
    for y in hero.y..hero.bottom() {
        for x in hero.x..hero.right() {
            assert_ne!(
                buffer[(x, y)].symbol(),
                "\u{2318}",
                "no pills may render inside the hero rect"
            );
        }
    }
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
    let (mut model, terminal) = render_podcast_shell(app, 100, 30, true);
    let layout = &model.app.layout.main;
    let out = buffer_to_string(&terminal);

    assert!(layout.hero_area.x < layout.left_area.x);

    // Repoint from the legacy `LayoutMain.audiobookshelf_episode_rows` to the
    // mounted component's painted geometry (task 5.3d.10, Unit D). Wide podcast
    // detail preserves the painted episode rows through component-owned
    // geometry; the played filter governs which episodes the component paints.
    let component_id = model
        .abs_podcast_id
        .as_ref()
        .expect("podcast component mounted");
    let episode_rows = model
        .application
        .get_component_mut(component_id)
        .and_then(|comp| {
            comp.as_any_mut()
                .downcast_mut::<AudiobookshelfPodcastComponent>()
        })
        .map(|component| component.geometry().episode_rows.clone())
        .expect("podcast component mounted");
    assert!(!episode_rows.is_empty());
    assert!(out.contains("Episode A"));
}

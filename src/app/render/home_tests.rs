use super::super::home_video::power_home_panel_scroll;
use crate::app::layout::AppLayout;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{BrowseLevel, FeedHomeVideoGroup, FeedHomeVideoState, LibraryTab};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

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

fn make_home_video_panel_app() -> crate::app::App {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    app.library_tab = 1;

    let mut library = make_item("Home Videos", "CollectionFolder");
    library.id = "lib-homevideos".into();
    library.is_folder = true;
    library.collection_type = "homevideos".into();

    let mut selected = make_item("Selected Home Video", "Video");
    selected.id = "video-selected".into();
    selected.overview = "Selected home-video overview.".into();
    selected.runtime_ticks = 25 * 60 * TICKS_PER_SECOND;
    let mut other = make_item("Other Home Video", "Video");
    other.id = "video-other".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-homevideos".into(),
            title: "Home Videos".into(),
            items: vec![selected.clone(), other],
            total_count: 2,
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
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![make_item("Other Feed Video", "Video"), selected],
            groups: vec![FeedHomeVideoGroup {
                folder: make_item("Feed", "Folder"),
                items: Vec::new(),
            }],
            loading: false,
            selected_group: 0,
            video_cursor: 0,
            video_scroll: 0,
        }),
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

#[test]
fn renders_home_pills_and_only_selected_section() {
    let mut app = make_app_stub();

    let mut cont = make_items(3);
    for (i, it) in cont.iter_mut().enumerate() {
        it.name = ["Taskmaster", "QI XL", "8 Diagram Pole Fighter"][i].to_string();
        it.runtime_ticks = (2820 + i as i64 * 600) * TICKS_PER_SECOND;
    }
    app.home.continue_items = cont;

    let music = {
        let mut v = make_items(3);
        for (i, it) in v.iter_mut().enumerate() {
            it.name = ["King Of America", "Either/Or", "Too-Rye-Ay"][i].to_string();
        }
        v[0].overview = "Newest metadata overview appears in the shared Home hero.".into();
        v
    };
    let youtube = {
        let mut v = make_items(2);
        for (i, it) in v.iter_mut().enumerate() {
            it.name = ["NXL Not-E3 Showcase", "Comedians Taking Over"][i].to_string();
        }
        v
    };
    app.home.latest = vec![
        ("Music".into(), "l1".into(), music, 0),
        ("YouTube".into(), "l2".into(), youtube, 0),
    ];

    let backend = TestBackend::new(80, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        let area = Rect::new(0, 0, 80, 30);
        app.render_power_home_list(f, area, true, &mut layout.main);
    })
    .unwrap();

    let out = buffer_to_string(&term);

    assert!(out.contains("Taskmaster"));
    assert!(out.contains("QI XL"));
    assert!(out.contains("8 Diagram Pole Fighter"));
    assert!(out.contains("Continue Watching"));
    assert!(out.contains("Music"));
    assert!(out.contains("YouTube"));
    assert!(!out.contains("King Of America"));
    assert!(!out.contains("Either/Or"));
    assert!(!out.contains("NXL Not-E3 Showcase"));
    assert!(out.contains("47m"));
    assert!(out.contains("67m"));
    assert!(!out.contains("1h"));
    assert_eq!(layout.main.home.hitmap.len(), 3);
    assert_eq!(layout.main.selector_tabs.len(), 3);
    let unselected_pill = layout.main.selector_tabs[1];
    let unselected_cell = &term.backend().buffer()[(unselected_pill.0.x + 1, unselected_pill.0.y)];
    assert_eq!(unselected_cell.bg, crate::app::palette::PILL_SELECTOR_BG);
    assert_eq!(unselected_cell.fg, crate::app::palette::PILL_SELECTOR_FG);

    app.power_home_select_section(1);
    let backend = TestBackend::new(80, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        let area = Rect::new(0, 0, 80, 30);
        app.render_power_home_list(f, area, true, &mut layout.main);
    })
    .unwrap();

    let out = buffer_to_string(&term);

    assert!(!out.contains("Taskmaster"));
    assert!(out.contains("King Of America"));
    assert!(out.contains("Newest metadata overview"));
    assert!(out.contains("appears in the shared Home"));
    assert!(out.contains("hero."));
    assert!(out.contains("Either/Or"));
    assert!(!out.contains("NXL Not-E3 Showcase"));
    assert_eq!(layout.main.home.hitmap.len(), 3);
}

#[test]
fn wide_continue_watching_places_pills_and_list_right_of_image_first_hero() {
    let mut app = make_app_stub();
    let mut item = make_item("Hero Episode", "Episode");
    item.series_name = "Hero Show".into();
    item.runtime_ticks = 45 * 60 * TICKS_PER_SECOND;
    item.overview = "The selected episode metadata belongs below its artwork.".into();
    app.home.continue_items = vec![item];

    let backend = TestBackend::new(120, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_home_list(f, Rect::new(0, 0, 120, 30), true, &mut layout.main);
    })
    .unwrap();

    let output = buffer_to_string(&term);
    let lines: Vec<_> = output.lines().collect();
    let hero_text_y = lines
        .iter()
        .position(|line| line[..32].contains("Hero Episode"))
        .expect("hero metadata should render in the left column");

    assert!(
        layout.main.selector_tabs.iter().all(|tab| tab.0.x >= 33),
        "wide Continue Watching pills belong at the top of the right panel"
    );
    assert!(
        lines[3].contains("Hero Episode"),
        "the right-panel list should follow the pills and panel top inset"
    );
    let right_panel_x = layout.main.selector_tabs[0].0.x;
    assert!(
        (right_panel_x..120).all(|x| term.backend().buffer()[(x, 1)].symbol() == " "),
        "Home should keep one blank row below the pill bar"
    );
    assert_eq!(
        term.backend().buffer()[(right_panel_x, 1)].bg,
        crate::app::palette::LIBRARY_SIDE_BG,
        "the spacer below the Home pill bar should use the library background"
    );
    assert!(
        (right_panel_x..120).all(|x| term.backend().buffer()[(x, 2)].symbol() == "▔"),
        "Home should use a full-width top border below the pill bar"
    );
    assert_eq!(
        term.backend().buffer()[(right_panel_x, 2)].bg,
        crate::app::palette::BG_GREEN,
        "the list panel top row should use the green panel background"
    );
    assert_eq!(
        hero_text_y, 15,
        "the left hero metadata should follow the top-aligned image after one blank row"
    );
    assert_eq!(
        term.backend().buffer()[(64, 3)].fg,
        crate::app::palette::WHITE,
        "wide Home right-panel titles should remain white when the panel is focused"
    );
    assert!(
        (right_panel_x..120).all(|x| term.backend().buffer()[(x, 2)].symbol() == "▔"),
        "wide Home right-panel top border should span the green row"
    );
    assert!(
        (right_panel_x..120).all(|x| term.backend().buffer()[(x, 28)].symbol() == "▁"),
        "wide Home right-panel bottom border should span the green row"
    );
    assert_eq!(
        term.backend().buffer()[(right_panel_x, 2)].fg,
        crate::app::palette::SEEK_TRACK
    );
    assert_eq!(
        term.backend().buffer()[(right_panel_x, 28)].fg,
        crate::app::palette::SEEK_TRACK
    );
    assert_eq!(
        term.backend().buffer()[(54, 3)].fg,
        crate::app::palette::YELLOW,
        "wide Home show labels should use the yellow accent"
    );
    assert_eq!(
        term.backend().buffer()[(0, 15)].fg,
        crate::app::palette::YELLOW,
        "wide Home hero titles should use the yellow accent"
    );
    let selected_pill = layout.main.selector_tabs[0].0;
    assert_eq!(
        selected_pill.y, 0,
        "wide Home pills should start on the top row"
    );
    let selected_cell = &term.backend().buffer()[(selected_pill.x + 1, selected_pill.y)];
    assert_eq!(
        selected_cell.bg,
        crate::app::palette::PILL_SELECTOR_SELECTED_BG
    );
    assert_eq!(
        selected_cell.fg,
        crate::app::palette::PILL_SELECTOR_SELECTED_FG
    );

    let backend = TestBackend::new(120, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_home_list(f, Rect::new(0, 0, 120, 30), false, &mut layout.main);
    })
    .unwrap();

    assert_eq!(
        term.backend().buffer()[(64, 3)].fg,
        crate::app::palette::MUTED,
        "wide Home right-panel titles should dim when the panel is unfocused"
    );
}

#[test]
fn playing_home_video_indents_aqua_play_icon_one_cell() {
    let mut app = make_app_stub();
    let mut item = make_item("Playing Home Video", "Video");
    item.id = "playing-home-video".into();
    app.home.continue_items = vec![item.clone()];
    app.player_tab.set_items(vec![item], 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
    }

    let backend = TestBackend::new(120, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_home_list(f, Rect::new(0, 0, 120, 30), true, &mut layout.main);
    })
    .unwrap();

    let throbber_symbols = ["▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    let buf = term.backend().buffer();
    let icon_cell = (0..120)
        .find_map(|x| {
            let cell = &buf[(x, 3)];
            if throbber_symbols.contains(&cell.symbol()) {
                Some(cell)
            } else {
                None
            }
        })
        .expect("expected throbber symbol somewhere in the home row");
    assert_eq!(
        icon_cell.fg,
        crate::app::palette::AQUA,
        "rendered Home panel:\n{}",
        buffer_to_string(&term)
    );
}

#[test]
fn selected_home_row_uses_aqua_bar_marker() {
    let mut app = make_app_stub();
    app.home.continue_items = vec![make_item("Selected Home Item", "Video")];

    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_home_list(f, Rect::new(0, 0, 60, 20), true, &mut layout.main);
    })
    .unwrap();

    let cell = &term.backend().buffer()[(0, 5)];
    assert_eq!(
        cell.symbol(),
        "▍",
        "rendered Home panel:\n{}",
        buffer_to_string(&term)
    );
    assert_eq!(cell.fg, crate::app::palette::AQUA);
}

#[test]
fn row_continue_watching_places_list_one_row_after_hero() {
    let mut app = make_app_stub();
    let mut item = make_item("Row Hero Episode", "Episode");
    item.series_name = "Row Hero Show".into();
    item.runtime_ticks = 45 * 60 * TICKS_PER_SECOND;
    item.overview = "Row-mode hero metadata.".into();
    app.home.continue_items = vec![item];

    let backend = TestBackend::new(60, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_home_list(f, Rect::new(0, 0, 60, 30), true, &mut layout.main);
    })
    .unwrap();

    let output = buffer_to_string(&term);
    let lines: Vec<_> = output.lines().collect();
    assert!(
        lines[13].contains("Row Hero Episode"),
        "the list should begin one row after the row-mode hero"
    );
}

#[test]
fn selected_regular_home_video_keeps_detail_below_title() {
    let mut app = make_home_video_panel_app();
    app.libs[0].feed_home_video = None;

    let backend = TestBackend::new(60, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_home_video_list(f, Rect::new(0, 0, 60, 30), 0, true, &mut layout.main);
    })
    .unwrap();

    let out = buffer_to_string(&term);
    let title = out
        .find("Selected Home Video")
        .expect("selected home-video title should render");
    let overview = out
        .find("Selected home-video overview.")
        .unwrap_or_else(|| panic!("selected home-video detail should render:\n{out}"));
    let other = out
        .find("Other Home Video")
        .expect("following home-video row should render");
    assert!(
        title < overview && overview < other,
        "unexpected render order:\n{out}"
    );
    assert_eq!(layout.main.cursor_screen_y, Some(2));
    assert_eq!(layout.main.left_row_map[0], Some(0));
    assert_eq!(
        term.backend().buffer()[(0, 2)].bg,
        crate::app::palette::MEDIA_SELECTED_BG,
        "selected home-video block should retain its leading inset"
    );
    let other_line = out
        .lines()
        .find(|line| line.contains("Other Home Video"))
        .expect("following home-video row should render");
    assert!(
        other_line.starts_with("Other Home Video"),
        "unselected home-video rows should not be indented:\n{out}"
    );
    let other_row = layout
        .main
        .left_row_map
        .iter()
        .position(|row| *row == Some(1))
        .expect("unselected home-video row should map to the display");
    assert!(other_row + 1 < layout.main.left_row_map.len());
    assert_eq!(
        layout
            .main
            .left_row_map
            .get(other_row + 1)
            .copied()
            .flatten(),
        None,
        "unselected home-video rows should occupy one line"
    );
}

#[test]
fn selected_grouped_feed_home_video_keeps_detail_and_scroll_state() {
    let mut app = make_home_video_panel_app();
    app.client
        .lock()
        .unwrap()
        .config
        .feed_view_libraries
        .push("home videos".into());
    let feed_state = app.libs[0].feed_home_video.as_mut().unwrap();
    feed_state.video_cursor = 1;
    feed_state.video_scroll = 1;

    let backend = TestBackend::new(60, 30);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_power_feed_home_video_group_view(
            f,
            Rect::new(0, 0, 60, 30),
            0,
            true,
            &mut layout.main,
        );
    })
    .unwrap();

    let out = buffer_to_string(&term);
    assert!(out.contains("All"), "feed selector should render:\n{out}");
    let title = out
        .find("Selected Home Video")
        .expect("selected feed home-video title should render");
    let overview = out
        .find("Selected home-video overview.")
        .unwrap_or_else(|| panic!("selected feed home-video detail should render:\n{out}"));
    assert!(title < overview, "unexpected render order:\n{out}");
    assert_eq!(
        app.libs[0].feed_home_video.as_ref().unwrap().video_scroll,
        1
    );
    assert_eq!(layout.main.left_row_map[0], Some(1));
}

#[test]
fn keeps_current_offset_when_row_already_visible() {
    assert_eq!(power_home_panel_scroll(0, 2, 6, 20, 10), 0);
}

#[test]
fn scrolls_down_to_reveal_row_below_viewport() {
    assert_eq!(power_home_panel_scroll(0, 14, 20, 30, 10), 10);
}

#[test]
fn scrolls_up_to_reveal_row_above_viewport() {
    assert_eq!(power_home_panel_scroll(8, 2, 6, 30, 10), 2);
}

#[test]
fn never_scrolls_past_end() {
    assert_eq!(power_home_panel_scroll(99, 11, 15, 15, 10), 5);
}

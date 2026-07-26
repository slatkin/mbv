#[cfg(test)]
mod tests {
    use super::power_home_panel_scroll;
    use crate::app::layout::AppLayout;
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{palette, BrowseLevel, FeedHomeVideoGroup, FeedHomeVideoState, LibraryTab};
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

    fn assert_selected_home_video_panel(term: &Terminal<TestBackend>, title: &str) {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let (title_y, title_x) = (0..area.height)
            .find_map(|y| {
                let line: String = (0..area.width).map(|x| buf[(x, y)].symbol()).collect();
                line.find(title).map(|x| (y, x as u16))
            })
            .expect("selected home-video title should be present in the buffer");
        assert_eq!(
            buf[(title_x, title_y)].fg,
            palette::YELLOW,
            "selected home-video title should be yellow"
        );

        let row_is = |y: u16, glyph: &str| (0..area.width).all(|x| buf[(x, y)].symbol() == glyph);
        let top_y = (0..area.height)
            .find(|&y| row_is(y, "▁"))
            .expect("selected home-video top border should render");
        let bottom_y = (0..area.height)
            .find(|&y| row_is(y, "▔"))
            .expect("selected home-video bottom border should render");
        assert!(top_y < title_y && title_y < bottom_y);
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
        println!("\n{out}");

        assert!(out.contains("Taskmaster"));
        assert!(out.contains("QI XL"));
        assert!(out.contains("8 Diagram Pole Fighter"));
        assert!(out.contains("Continue Watching"));
        assert!(out.contains("Music"));
        assert!(out.contains("YouTube"));
        assert!(!out.contains("King Of America"));
        assert!(!out.contains("Either/Or"));
        assert!(!out.contains("NXL Not-E3 Showcase"));
        // Durations render as minutes only, never hours (67m for 4020s, not 1h07m).
        assert!(out.contains("47m"));
        assert!(out.contains("67m"));
        assert!(!out.contains("1h"));
        assert_eq!(layout.main.home.hitmap.len(), 3);
        assert_eq!(layout.main.selector_tabs.len(), 3);

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
        println!("\n{out}");

        assert!(!out.contains("Taskmaster"));
        assert!(out.contains("King Of America"));
        assert!(out.contains("Newest metadata overview appears"));
        assert!(out.contains("Either/Or"));
        assert!(!out.contains("NXL Not-E3 Showcase"));
        assert_eq!(layout.main.home.hitmap.len(), 3);
    }

    #[test]
    fn home_list_does_not_draw_selected_media_box() {
        let mut app = make_app_stub();
        let mut cont = make_items(3);
        for (i, it) in cont.iter_mut().enumerate() {
            it.name = format!("Continue {i}");
        }
        app.home.continue_items = cont;
        app.home.latest = vec![
            ("Music".into(), "l1".into(), make_items(2), 0),
            ("YouTube".into(), "l2".into(), make_items(2), 0),
        ];

        let backend = TestBackend::new(26, 16);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            app.render_power_home_list(f, Rect::new(2, 2, 20, 14), true, &mut layout.main);
        })
        .unwrap();

        let out = buffer_to_string(&term);
        assert!(!out.contains('\u{2581}'), "unexpected top border:\n{out}");
        assert!(
            !out.contains('\u{2594}'),
            "unexpected bottom border:\n{out}"
        );
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
        assert_selected_home_video_panel(&term, "Selected Home Video");
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
        assert_selected_home_video_panel(&term, "Selected Home Video");
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
        // Row [2,6) fits inside viewport [0,10); offset unchanged.
        assert_eq!(power_home_panel_scroll(0, 2, 6, 20, 10), 0);
    }

    #[test]
    fn scrolls_down_to_reveal_row_below_viewport() {
        // Row [14,20) is below viewport [0,10); scroll so its bottom is visible.
        assert_eq!(power_home_panel_scroll(0, 14, 20, 30, 10), 10);
    }

    #[test]
    fn scrolls_up_to_reveal_row_above_viewport() {
        // Row [2,6) is above current offset 8; snap up to its top.
        assert_eq!(power_home_panel_scroll(8, 2, 6, 30, 10), 2);
    }

    #[test]
    fn never_scrolls_past_end() {
        // Cursor is the last row [11,15); offset clamped to total_h - view_h = 5.
        assert_eq!(power_home_panel_scroll(99, 11, 15, 15, 10), 5);
    }
}

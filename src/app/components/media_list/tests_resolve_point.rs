mod resolve_point {
    use super::super::{
        test_helpers::lifecycle_item, InlineMediaBrowser, InlineMediaBrowserPaintPolicy,
        MediaListRow, SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
    };
    use ratatui::backend::TestBackend;
    use ratatui::layout::{Position, Rect};
    use ratatui::Terminal;
    use tuirealm::component::Component;

    fn wide() -> WideMediaList<String> {
        let mut list = WideMediaList::new();
        list.set_content(vec![
            MediaListRow::Heading { text: "A".into() },
            lifecycle_item("a"),
            lifecycle_item("b"),
            lifecycle_item("c"),
            lifecycle_item("d"),
            lifecycle_item("e"),
        ]);
        list
    }

    /// Complete one retained view so point resolution reads only the current
    /// frame's geometry (design.md D6).
    fn paint_wide(list: &mut WideMediaList<String>, area: Rect) {
        list.set_geometry(area, area);
        list.set_paint_policy(WideMediaListPaintPolicy::new(
            false,
            SelectedRowSurface::ListBackdrop,
            None,
        ));
        let mut terminal =
            Terminal::new(TestBackend::new(area.right().max(1), area.bottom().max(1))).unwrap();
        terminal.draw(|f| list.view(f, area)).unwrap();
    }

    #[test]
    fn wide_resolves_against_a_scrolled_viewport() {
        let mut list = wide();
        list.select_last();
        let area = Rect {
            x: 2,
            y: 5,
            width: 10,
            height: 3,
        };
        // offset is 3 (6 rows, height 3): screen rows 5,6,7 -> c,d,e.
        assert_eq!(list.resolve_viewport(3).offset, 3);
        paint_wide(&mut list, area);
        assert_eq!(
            list.resolve_current_point(Position { x: 4, y: 5 }),
            Some(&"c".to_string())
        );
        assert_eq!(
            list.resolve_current_point(Position { x: 4, y: 7 }),
            Some(&"e".to_string())
        );
        // Past the painted height / below the area.
        assert_eq!(list.resolve_current_point(Position { x: 4, y: 8 }), None);
        // Left of the area.
        assert_eq!(list.resolve_current_point(Position { x: 1, y: 5 }), None);
    }

    #[test]
    fn wide_heading_row_and_past_last_row_resolve_none() {
        let mut list = wide();
        list.select_first();
        let area = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 6,
        };
        paint_wide(&mut list, area);
        assert_eq!(list.resolve_current_point(Position { x: 1, y: 0 }), None); // heading
        assert!(list.claims_current_point(Position { x: 1, y: 0 }));
        assert!(!list.claims_current_point(Position { x: 10, y: 0 }));
        assert_eq!(
            list.resolve_current_point(Position { x: 1, y: 1 }),
            Some(&"a".to_string())
        );
        // Row past the last content row but still inside the area.
        assert_eq!(list.resolve_current_point(Position { x: 1, y: 6 }), None);
    }

    fn inline() -> InlineMediaBrowser<String> {
        let mut browser = InlineMediaBrowser::new();
        browser.set_content(vec![
            MediaListRow::Heading { text: "A".into() },
            lifecycle_item("a"),
            lifecycle_item("b"),
            lifecycle_item("c"),
            lifecycle_item("d"),
        ]);
        browser.select_target(&"b".to_string());
        browser
    }

    fn paint_inline(browser: &mut InlineMediaBrowser<String>, area: Rect, detail_rows: usize) {
        browser.set_geometry(area, area);
        browser.set_paint_policy(InlineMediaBrowserPaintPolicy::new(
            false,
            SelectedRowSurface::ListBackdrop,
            detail_rows,
        ));
        let mut terminal =
            Terminal::new(TestBackend::new(area.right().max(1), area.bottom().max(1))).unwrap();
        terminal.draw(|f| browser.view(f, area)).unwrap();
    }

    #[test]
    fn inline_resolves_rows_around_the_detail_block() {
        let mut browser = inline();
        let area = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 6,
        };
        paint_inline(&mut browser, area, 2);
        // flow: 0 Heading, 1 a, 2 detail(b), 3 detail-cont, 4 c, 5 d
        assert_eq!(browser.resolve_current_point(Position { x: 1, y: 0 }), None);
        assert_eq!(
            browser.resolve_current_point(Position { x: 1, y: 1 }),
            Some(&"a".to_string())
        );
        assert_eq!(
            browser.resolve_current_point(Position { x: 1, y: 2 }),
            Some(&"b".to_string())
        );
        // The admitted detail block maps every continuation row to the
        // retained selected target, so an inline hero click or drag
        // continuation carries the row that was painted (design.md D6).
        assert_eq!(
            browser.resolve_current_point(Position { x: 1, y: 3 }),
            Some(&"b".to_string())
        );
        assert_eq!(
            browser.resolve_current_point(Position { x: 1, y: 4 }),
            Some(&"c".to_string())
        );
        // Outside the area horizontally.
        assert_eq!(
            browser.resolve_current_point(Position { x: 40, y: 2 }),
            None
        );
        assert!(browser.claims_current_point(Position { x: 1, y: 3 }));
        assert!(!browser.claims_current_point(Position { x: 40, y: 2 }));
    }
}

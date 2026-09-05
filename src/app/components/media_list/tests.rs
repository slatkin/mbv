use super::{MediaKind, MediaListRow, MediaSemanticState, WideMediaList};

#[test]
fn wide_list_maps_display_rows_to_selectable_indices_and_viewport() {
    let mut list: WideMediaList<String> = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        MediaListRow::Item {
            target: "ordinary".into(),
            primary: "Ordinary".into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Spacer,
        MediaListRow::Item {
            target: "active".into(),
            primary: "Active".into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::active(Some(125)),
        },
        MediaListRow::Item {
            target: "played".into(),
            primary: "Played".into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Played,
        },
        MediaListRow::Item {
            target: "disabled".into(),
            primary: "Disabled".into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Disabled,
        },
    ]);

    assert_eq!(list.selectable_len(), 4);
    assert_eq!(list.selected_display_row(), Some(1));
    assert_eq!(
        list.rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item { semantic_state, .. } => Some(semantic_state),
                MediaListRow::Heading { .. } | MediaListRow::Spacer => None,
            })
            .collect::<Vec<_>>(),
        vec![
            &MediaSemanticState::Ordinary,
            &MediaSemanticState::active(Some(100)),
            &MediaSemanticState::Played,
            &MediaSemanticState::Disabled,
        ]
    );
    assert_eq!(
        match &list.rows()[3] {
            MediaListRow::Item { semantic_state, .. } => semantic_state,
            _ => panic!("expected active item"),
        },
        &MediaSemanticState::active(Some(100))
    );
    list.move_selection(3);
    assert_eq!(list.selected_display_row(), Some(5));
    let viewport = list.resolve_viewport(3);
    assert_eq!(
        (viewport.offset, viewport.height, viewport.total_rows),
        (3, 3, 6)
    );
    assert_eq!(list.selected_row_offset(3), Some(2));
}

mod resolve_point {
    use super::super::{
        InlineMediaBrowser, MediaKind, MediaListRow, MediaSemanticState, WideMediaList,
    };
    use ratatui::layout::{Position, Rect};

    fn item(target: &str) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: target.into(),
            trailing: None,
            duration: None,
            kind: MediaKind::Media,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    fn wide() -> WideMediaList<String> {
        let mut list = WideMediaList::new();
        list.set_content(vec![
            MediaListRow::Heading { text: "A".into() },
            item("a"),
            item("b"),
            item("c"),
            item("d"),
            item("e"),
        ]);
        list
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
        assert_eq!(
            list.resolve_point(area, Position { x: 4, y: 5 }),
            Some(&"c".to_string())
        );
        assert_eq!(
            list.resolve_point(area, Position { x: 4, y: 7 }),
            Some(&"e".to_string())
        );
        // Past the painted height / below the area.
        assert_eq!(list.resolve_point(area, Position { x: 4, y: 8 }), None);
        // Left of the area.
        assert_eq!(list.resolve_point(area, Position { x: 1, y: 5 }), None);
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
        assert_eq!(list.resolve_point(area, Position { x: 1, y: 0 }), None); // heading
        assert_eq!(
            list.resolve_point(area, Position { x: 1, y: 1 }),
            Some(&"a".to_string())
        );
        // Row past the last content row but still inside the area.
        assert_eq!(list.resolve_point(area, Position { x: 1, y: 6 }), None);
    }

    fn inline() -> InlineMediaBrowser<String> {
        let mut browser = InlineMediaBrowser::new();
        browser.set_content(vec![
            MediaListRow::Heading { text: "A".into() },
            item("a"),
            item("b"),
            item("c"),
            item("d"),
        ]);
        browser.select_target(&"b".to_string());
        browser
    }

    #[test]
    fn inline_resolves_rows_around_the_detail_block() {
        let browser = inline();
        let area = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 6,
        };
        // flow: 0 Heading, 1 a, 2 detail(b), 3 detail-cont, 4 c, 5 d
        assert_eq!(
            browser.resolve_point(area, 2, Position { x: 1, y: 0 }),
            None
        );
        assert_eq!(
            browser.resolve_point(area, 2, Position { x: 1, y: 1 }),
            Some(&"a".to_string())
        );
        assert_eq!(
            browser.resolve_point(area, 2, Position { x: 1, y: 2 }),
            Some(&"b".to_string())
        );
        // Detail-block continuation row: no target.
        assert_eq!(
            browser.resolve_point(area, 2, Position { x: 1, y: 3 }),
            None
        );
        assert_eq!(
            browser.resolve_point(area, 2, Position { x: 1, y: 4 }),
            Some(&"c".to_string())
        );
        // Outside the area horizontally.
        assert_eq!(
            browser.resolve_point(area, 2, Position { x: 40, y: 2 }),
            None
        );
    }
}

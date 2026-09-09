use super::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, MediaKind, MediaListRow, MediaSemanticState,
    SelectedRowSurface, ViewportAnchor, WideMediaList, WideMediaListPaintPolicy,
};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tuirealm::component::Component;

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
        assert!(list.claims_point(area, Position { x: 1, y: 0 }));
        assert!(!list.claims_point(area, Position { x: 10, y: 0 }));
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
        assert!(browser.claims_point(area, Position { x: 1, y: 3 }));
        assert!(!browser.claims_point(area, Position { x: 40, y: 2 }));
    }
}

fn lifecycle_item(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

#[test]
fn media_lists_preserve_refresh_clamp_and_transfer_grouped_anchor() {
    let grouped_rows = || {
        vec![
            MediaListRow::Heading { text: "A".into() },
            lifecycle_item("a"),
            MediaListRow::Spacer,
            lifecycle_item("b"),
            lifecycle_item("c"),
        ]
    };
    let mut wide = WideMediaList::new();
    wide.set_content(grouped_rows());
    wide.select_target(&"b".to_string());
    wide.set_scroll(2);

    // An ordinary projection preserves the stable selected target.
    wide.set_content(grouped_rows());
    assert_eq!(wide.selected_target(), Some(&"b".to_string()));
    assert_eq!(wide.scroll(), 2, "ordinary refresh preserves local scroll");

    // A smaller projection clamps only locally when the target disappeared.
    wide.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        lifecycle_item("a"),
    ]);
    assert_eq!(wide.selected_target(), Some(&"a".to_string()));

    // A responsive handoff transfers one display-row anchor, including the
    // heading and spacer structural rows, to the other persistent control.
    wide.set_content(grouped_rows());
    wide.select_target(&"b".to_string());
    wide.set_scroll(2);
    let anchor = wide
        .viewport_anchor(3)
        .expect("selected grouped row anchors");
    assert_eq!(anchor.selected_row_offset, 1);

    let mut inline = InlineMediaBrowser::new();
    inline.set_content(grouped_rows());
    inline.apply_viewport_anchor(&anchor, 3);
    assert_eq!(inline.selected_target(), Some(&"b".to_string()));
    assert_eq!(inline.resolve_viewport(3).offset, 2);

    // The explicit anchor, unlike an ordinary refresh, transfers the row
    // offset. Verify the reverse direction uses the same contract.
    let reverse = inline.viewport_anchor(3).expect("inline row anchors");
    let mut wide_again = WideMediaList::new();
    wide_again.set_content(grouped_rows());
    wide_again.apply_viewport_anchor(&reverse, 3);
    assert_eq!(wide_again.selected_target(), Some(&"b".to_string()));
    assert_eq!(wide_again.resolve_viewport(3).offset, 2);
}

#[test]
fn browser_refresh_preserves_local_target_until_explicit_anchor_reanchors() {
    let rows = vec![
        lifecycle_item("a"),
        lifecycle_item("b"),
        lifecycle_item("c"),
    ];
    let mut wide = WideMediaList::new();
    wide.set_content(rows.clone());
    wide.select_target(&"b".to_string());
    wide.set_scroll(1);

    wide.set_content(rows.clone());
    assert_eq!(wide.selected_target(), Some(&"b".to_string()));
    assert_eq!(wide.scroll(), 1);

    let anchor = ViewportAnchor {
        selected_target: "c".to_string(),
        selected_row_offset: 0,
    };
    wide.apply_viewport_anchor(&anchor, 2);
    assert_eq!(wide.selected_target(), Some(&"c".to_string()));
    assert_eq!(wide.scroll(), 1);

    wide.set_content(vec![lifecycle_item("a")]);
    assert_eq!(wide.selected_target(), Some(&"a".to_string()));
}

#[test]
fn wide_component_retains_only_completed_current_frame_facts() {
    let mut list = WideMediaList::new();
    list.set_content(vec![lifecycle_item("one"), lifecycle_item("two")]);
    let area = Rect::new(2, 1, 12, 2);
    let claim_rect = Rect::new(1, 1, 16, 2);
    let content_rect = Rect::new(4, 1, 8, 2);
    list.set_geometry(claim_rect, content_rect);

    assert!(list.current_claim_rect().is_none());
    list.begin_view();
    assert!(!list.claims_current_point(Position { x: 0, y: 1 }));

    let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
    terminal
        .draw(|frame| {
            list.set_paint_policy(WideMediaListPaintPolicy::new(
                true,
                SelectedRowSurface::ListBackdrop,
                None,
            ));
            Component::view(&mut list, frame, area);
        })
        .unwrap();
    assert_eq!(list.current_claim_rect(), Some(claim_rect));
    assert_eq!(list.current_content_rect(), Some(content_rect));
    assert_eq!(list.current_selected_target(), Some(&"one".to_string()));
    assert_eq!(
        list.resolve_current_point(Position { x: 2, y: 1 }),
        Some(&"one".to_string())
    );
    assert_eq!(list.resolve_current_point(Position { x: 0, y: 1 }), None);
    list.set_geometry(Rect::new(1, 1, 16, 2), Rect::new(5, 1, 6, 2));
    assert!(list.current_claim_rect().is_none());
    list.set_geometry(Rect::new(1, 1, 0, 2), Rect::new(5, 1, 6, 2));
    terminal
        .draw(|frame| Component::view(&mut list, frame, area))
        .unwrap();
    assert!(list.current_claim_rect().is_none());

    list.set_content(vec![]);
    assert!(list.current_claim_rect().is_none());
    terminal
        .draw(|frame| Component::view(&mut list, frame, area))
        .unwrap();
    assert!(list.current_claim_rect().is_none());

    terminal
        .draw(|frame| Component::view(&mut list, frame, Rect::new(2, 1, 0, 2)))
        .unwrap();
    assert!(list.current_claim_rect().is_none());
}

#[test]
fn inline_component_retains_detail_and_resolves_from_the_current_view() {
    let mut browser = InlineMediaBrowser::new();
    browser.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        lifecycle_item("one"),
        lifecycle_item("two"),
    ]);
    browser.select_target(&"two".to_string());
    let area = Rect::new(0, 0, 24, 5);
    let claim_rect = Rect::new(0, 0, 24, 5);
    let content_rect = Rect::new(3, 0, 18, 5);
    browser.set_geometry(claim_rect, content_rect);
    let mut terminal = Terminal::new(TestBackend::new(28, 6)).unwrap();

    terminal
        .draw(|frame| {
            browser.set_paint_policy(InlineMediaBrowserPaintPolicy::new(
                true,
                SelectedRowSurface::ListBackdrop,
                2,
            ));
            Component::view(&mut browser, frame, area);
        })
        .unwrap();

    assert_eq!(browser.current_claim_rect(), Some(claim_rect));
    assert_eq!(browser.current_content_rect(), Some(content_rect));
    assert_eq!(browser.current_detail_rect(), Some(Rect::new(3, 2, 18, 2)));
    assert_eq!(
        browser.resolve_current_point(Position { x: 1, y: 2 }),
        Some(&"two".to_string())
    );
    assert_eq!(
        browser.resolve_current_point(Position { x: 24, y: 2 }),
        None
    );
    browser.set_content(vec![]);
    assert!(browser.current_claim_rect().is_none());
    assert!(browser.current_detail_rect().is_none());
}

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
        InlineMediaBrowser, InlineMediaBrowserPaintPolicy, MediaKind, MediaListRow,
        MediaSemanticState, SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
    };
    use ratatui::backend::TestBackend;
    use ratatui::layout::{Position, Rect};
    use ratatui::Terminal;
    use tuirealm::component::Component;

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
            item("a"),
            item("b"),
            item("c"),
            item("d"),
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

#[test]
fn grid_preserves_two_column_cells_and_resolves_stable_targets() {
    use super::{GridMediaList, GridPaintPolicy};
    let mut grid = GridMediaList::new();
    grid.set_columns(2, 8, 2);
    grid.set_content(vec![
        lifecycle_item("a"),
        lifecycle_item("b"),
        lifecycle_item("c"),
    ]);
    grid.set_geometry(Rect::new(0, 0, 20, 2), Rect::new(1, 1, 18, 2));
    let mut terminal = Terminal::new(TestBackend::new(24, 5)).unwrap();
    terminal
        .draw(|frame| {
            grid.set_paint_policy(GridPaintPolicy::new(true));
            Component::view(&mut grid, frame, Rect::new(0, 0, 20, 2));
        })
        .unwrap();
    let cells = grid.current_cells().expect("completed grid frame");
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[0].rect.y, cells[1].rect.y);
    assert_eq!(cells[1].rect.x, cells[0].rect.right() + 2);
    assert_eq!(
        grid.resolve_current_point(Position::new(cells[1].rect.x, cells[1].rect.y)),
        Some(&"b".to_string())
    );
    assert_eq!(
        grid.resolve_current_point(Position::new(cells[2].rect.x, cells[2].rect.y)),
        Some(&"c".to_string())
    );
}

#[test]
fn grid_movement_keeps_selected_target_in_the_painted_window() {
    use super::{GridMediaList, GridPaintPolicy};
    let mut grid = GridMediaList::new();
    grid.set_columns(2, 8, 2);
    grid.set_content((0..30).map(|n| lifecycle_item(&n.to_string())).collect());
    let area = Rect::new(0, 0, 20, 5);
    let mut terminal = Terminal::new(TestBackend::new(24, 7)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut grid, frame, area))
        .unwrap();
    let outcome = grid.delegate(super::RowLocalInput::Move(25), None);
    assert!(matches!(
        outcome,
        super::RowLocalOutcome::SelectedTargetChanged(_)
    ));
    grid.set_paint_policy(GridPaintPolicy::new(true));
    terminal
        .draw(|frame| Component::view(&mut grid, frame, area))
        .unwrap();
    let selected = grid.selected_target().cloned().expect("selection");
    assert!(grid.current_flow_offset().unwrap() > 0);
    assert!(grid
        .current_cells()
        .unwrap()
        .iter()
        .any(|cell| cell.target.as_ref() == Some(&selected)));
}

/// Task 4.1: the Grid presentation executes the established bucketed
/// two-column catalog policy — a `Heading`/`Spacer` occupies a full painted
/// line, a bucket's items pack into fresh item rows (so a ragged trailing row
/// stays inside its bucket), scroll/viewport facts count display lines, and
/// `move_item_rows` preserves the column across a header with the ragged
/// trailing-row clamp.
#[test]
fn grid_structural_rows_take_full_lines_and_ragged_traversal_clamps() {
    use super::{GridMediaList, GridPaintPolicy};
    let mut grid = GridMediaList::new();
    grid.set_columns(2, 8, 2);
    // A bucket: heading + 3 items (rows [a,b],[c] ragged); then a spacer +
    // heading; B bucket: [d,e].
    grid.set_content(vec![
        super::MediaListRow::Heading { text: "A".into() },
        lifecycle_item("a"),
        lifecycle_item("b"),
        lifecycle_item("c"),
        super::MediaListRow::Spacer,
        super::MediaListRow::Heading { text: "B".into() },
        lifecycle_item("d"),
        lifecycle_item("e"),
    ]);
    assert_eq!(
        grid.display_lines().len(),
        6,
        "heading/spacer take a line each; items pack two per line"
    );
    let area = Rect::new(0, 0, 20, 6);
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal
        .draw(|frame| {
            grid.set_paint_policy(GridPaintPolicy::new(true));
            Component::view(&mut grid, frame, area);
        })
        .unwrap();
    let cells = grid.current_cells().expect("completed grid frame");
    // Heading lines paint no cells; the B bucket starts on its own line after
    // the spacer + heading, never sharing a row with A's ragged item.
    assert_eq!(cells.len(), 5);
    let c = cells
        .iter()
        .find(|cell| cell.target.as_ref() == Some(&"c".to_string()))
        .expect("ragged item c painted");
    let d = cells
        .iter()
        .find(|cell| cell.target.as_ref() == Some(&"d".to_string()))
        .expect("next-bucket item d painted");
    assert_eq!(
        d.rect.y,
        c.rect.y + 3,
        "spacer + heading lines separate buckets"
    );
    assert!(
        grid.claims_current_point(Position::new(c.rect.x, c.rect.y + 1)),
        "a heading line is inside the claim but resolves no target"
    );
    assert_eq!(
        grid.resolve_current_point(Position::new(c.rect.x, c.rect.y + 1)),
        None,
        "a heading line resolves no cell target"
    );

    // Down from the ragged item c clamps into its own row (tree order: c is
    // its row's last item), then the next item row is the following bucket.
    assert!(grid.select_target(&"c".to_string()));
    grid.move_item_rows(1);
    assert_eq!(grid.selected_target(), Some(&"d".to_string()));
    grid.move_item_rows(-1);
    assert_eq!(grid.selected_target(), Some(&"c".to_string()));
}

#[test]
fn row_local_delegation_covers_movement_selection_and_external_intents() {
    use super::{RowIntent, RowLocalInput, RowLocalOutcome};
    let mut list = super::MediaList::new();
    list.set_content(vec![
        lifecycle_item("a"),
        lifecycle_item("b"),
        lifecycle_item("c"),
    ]);
    assert_eq!(
        list.delegate(RowLocalInput::Move(1), None),
        RowLocalOutcome::SelectedTargetChanged("b".into())
    );
    assert_eq!(
        list.delegate(RowLocalInput::Page(1), None),
        RowLocalOutcome::SelectedTargetChanged("c".into())
    );
    assert_eq!(
        list.delegate(RowLocalInput::Last, None),
        RowLocalOutcome::Consumed
    );
    assert_eq!(
        list.delegate(RowLocalInput::First, None),
        RowLocalOutcome::SelectedTargetChanged("a".into())
    );
    assert_eq!(
        list.delegate(
            RowLocalInput::Wheel {
                at: Position::new(0, 0),
                delta: 1
            },
            None
        ),
        RowLocalOutcome::SelectedTargetChanged("b".into())
    );
    assert_eq!(
        list.delegate(RowLocalInput::Click(Position::new(0, 0)), None),
        RowLocalOutcome::Unhandled
    );
    assert_eq!(
        list.delegate(RowLocalInput::DoubleClick(Position::new(0, 0)), None),
        RowLocalOutcome::Unhandled
    );
    assert_eq!(
        list.delegate(RowLocalInput::ContextClick(Position::new(0, 0)), None),
        RowLocalOutcome::Unhandled
    );
    assert_eq!(
        list.delegate(RowLocalInput::Activate, None),
        RowLocalOutcome::External(RowIntent::Activate("b".into()))
    );
    assert_eq!(
        list.delegate(RowLocalInput::Context, None),
        RowLocalOutcome::External(RowIntent::Context("b".into()))
    );
    assert_eq!(
        list.delegate(RowLocalInput::Click(Position::new(0, 0)), Some("a".into())),
        RowLocalOutcome::SelectedTargetChanged("a".into())
    );
    assert_eq!(
        list.delegate(
            RowLocalInput::DoubleClick(Position::new(0, 0)),
            Some("a".into())
        ),
        RowLocalOutcome::External(RowIntent::Activate("a".into()))
    );
    assert_eq!(
        list.delegate(
            RowLocalInput::ContextClick(Position::new(0, 0)),
            Some("a".into())
        ),
        RowLocalOutcome::External(RowIntent::Context("a".into()))
    );
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
fn responsive_presentations_reuse_one_media_list_owner() {
    let mut wide = WideMediaList::new();
    wide.set_content(vec![
        lifecycle_item("one"),
        lifecycle_item("two"),
        lifecycle_item("three"),
    ]);
    wide.select_target(&"two".to_string());
    wide.set_scroll(1);
    let rows_before = wide.rows().to_vec();
    let selected_before = wide.selected_target().cloned();
    let scroll_before = wide.scroll();

    // Moving the owner between closed presentations is a reconfiguration, not
    // a state transfer: the owner itself is moved without cloning its rows.
    let owner = wide.into_media_list();
    let inline = InlineMediaBrowser::from_media_list(owner);
    assert_eq!(inline.rows(), rows_before.as_slice());
    assert_eq!(inline.selected_target(), selected_before.as_ref());
    assert_eq!(inline.scroll(), scroll_before);

    let owner = inline.into_media_list();
    let wide_again = WideMediaList::from_media_list(owner);
    assert_eq!(wide_again.rows(), rows_before.as_slice());
    assert_eq!(wide_again.selected_target(), selected_before.as_ref());
    assert_eq!(wide_again.scroll(), scroll_before);
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
fn wide_same_scroll_invalidates_retained_claims_for_new_geometry() {
    let mut list = WideMediaList::new();
    list.set_content(vec![lifecycle_item("one"), lifecycle_item("two")]);
    list.set_geometry(Rect::new(0, 0, 20, 2), Rect::new(0, 0, 20, 2));
    let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut list, frame, frame.area()))
        .unwrap();
    assert!(list.current_claim_rect().is_some());

    // A responsive re-split can resolve the same offset while requiring a
    // fresh paint; the shared control must not retain the old frame's claim.
    list.set_scroll(list.scroll());
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

#[test]
fn inline_delegate_keeps_the_completed_frame_for_pointer_continuity() {
    let mut browser = InlineMediaBrowser::new();
    browser.set_content(vec![
        lifecycle_item("one"),
        lifecycle_item("two"),
        lifecycle_item("three"),
    ]);
    browser.select_target(&"two".to_string());
    let area = Rect::new(0, 0, 24, 5);
    browser.set_geometry(area, area);
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
    let detail = browser
        .current_detail_rect()
        .expect("admitted detail block");
    // A selection-only delegation must not invalidate the frame the pointer is
    // still resolving against; the whole admitted detail block maps to the
    // retained selected target (design.md D6, ADR 0024).
    browser.delegate(
        super::RowLocalInput::Click(Position::new(detail.x, detail.y)),
        Some("two".to_string()),
    );
    assert_eq!(
        browser.resolve_current_point(Position::new(detail.x, detail.y + 1)),
        Some(&"two".to_string())
    );
    assert!(browser.current_detail_rect().is_some());
}

#[test]
fn wide_delegate_keeps_the_completed_frame_for_pointer_continuity() {
    let mut list = WideMediaList::new();
    list.set_content(vec![lifecycle_item("one"), lifecycle_item("two")]);
    let area = Rect::new(0, 0, 12, 2);
    list.set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(16, 4)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut list, frame, area))
        .unwrap();
    // A click selecting another row is selection-only; the painted frame
    // remains valid so a following drag/wheel still resolves (design.md D6).
    list.delegate(
        super::RowLocalInput::Click(Position::new(0, 1)),
        Some("two".to_string()),
    );
    assert_eq!(
        list.resolve_current_point(Position::new(0, 1)),
        Some(&"two".to_string())
    );
}

use super::test_helpers::lifecycle_item;
use super::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, MediaKind, MediaListRow, MediaSemanticState,
    SelectedRowSurface, ViewportAnchor, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::palette::{surface_colors_for_column_focus, Surface};
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

/// `unify-surface-colour` 4.2 (review fix): the selected-row highlight is
/// painted only while the paint policy's focus bit is set. With the bit clear,
/// a selected row in an unfocused queue panel or library rail leaves the
/// containing panel body showing through unchanged (there is no resting hole
/// painted); the resting punch-through value itself is pinned by
/// `render::components::media_list::wide::tests::selected_row_surface_color_follows_the_column_focus`.
#[test]
fn wide_selected_row_highlight_is_painted_only_while_focused() {
    use ratatui::style::{Color, Style};

    // Stands in for the containing panel body, so a skipped highlight is
    // observable rather than masked by a same-coloured panel fill.
    const BODY: Color = Color::Red;

    for surface in [
        SelectedRowSurface::OwningQueueColumn,
        SelectedRowSurface::OwningLibraryPane,
    ] {
        for focused in [true, false] {
            let mut list = WideMediaList::<String>::new();
            list.set_content(vec![
                MediaListRow::Heading { text: "A".into() },
                lifecycle_item("a"),
            ]);
            list.select_first();
            let area = Rect {
                x: 0,
                y: 0,
                width: 12,
                height: 2,
            };
            list.set_geometry(area, area);
            list.set_paint_policy(WideMediaListPaintPolicy::new(focused, surface, None));
            let mut terminal = Terminal::new(TestBackend::new(12, 2)).unwrap();
            terminal
                .draw(|f| {
                    f.render_widget(
                        ratatui::widgets::Block::default().style(Style::default().bg(BODY)),
                        area,
                    );
                    list.view(f, area);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            // Row 0 is the heading; the selected item is row 1.
            let selected = buffer[(0, 1)].style().bg;
            if focused {
                let expected = match surface {
                    SelectedRowSurface::OwningQueueColumn => Surface::SelectedRowOnQueueColumn,
                    SelectedRowSurface::OwningLibraryPane => Surface::SelectedRowOnLibraryPane,
                    SelectedRowSurface::ListBackdrop => Surface::SelectedRow,
                };
                assert_eq!(
                    selected,
                    Some(surface_colors_for_column_focus(expected, true).fill),
                    "{surface:?} focused selected-row fill"
                );
            } else {
                assert_eq!(
                    selected,
                    Some(BODY),
                    "{surface:?} unfocused selected row leaves the panel body showing"
                );
            }
        }
    }
}

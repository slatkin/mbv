use super::{MediaListRow, MediaSemanticState, WideMediaList};

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
            semantic_state: MediaSemanticState::Ordinary,
        },
        MediaListRow::Spacer,
        MediaListRow::Item {
            target: "active".into(),
            primary: "Active".into(),
            trailing: None,
            duration: None,
            semantic_state: MediaSemanticState::active(Some(125)),
        },
        MediaListRow::Item {
            target: "played".into(),
            primary: "Played".into(),
            trailing: None,
            duration: None,
            semantic_state: MediaSemanticState::Played,
        },
        MediaListRow::Item {
            target: "disabled".into(),
            primary: "Disabled".into(),
            trailing: None,
            duration: None,
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

use super::{MediaList, MediaListCarrier, MediaListOperation, WideMediaList};
use crate::list::{ViewportAnchor, Viewported};
use mbv_render::components::media_list::{
    MediaKind, MediaListRow, MediaListTitleReveal, MediaSemanticState, WideMediaListPaintPolicy,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use tuirealm::component::Component;

fn item(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

fn paint(list: &mut WideMediaList<String>, area: Rect) {
    list.set_geometry(area, area);
    list.set_paint_policy(WideMediaListPaintPolicy::new(false));
    let mut terminal =
        Terminal::new(TestBackend::new(area.right().max(1), area.bottom().max(1))).unwrap();
    terminal.draw(|f| list.view(f, area)).unwrap();
}

#[test]
fn wide_list_maps_structural_rows_and_clamps_viewport() {
    let mut list = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a"),
        MediaListRow::Spacer,
        item("b"),
        item("c"),
        item("d"),
    ]);
    list.select_last();
    assert_eq!(list.resolve_viewport(3).offset, 3);
    paint(&mut list, Rect::new(2, 5, 10, 3));
    assert_eq!(
        list.resolve_current_point(Position { x: 4, y: 5 }),
        Some(&"b".into())
    );
    assert_eq!(
        list.resolve_current_point(Position { x: 4, y: 7 }),
        Some(&"d".into())
    );
    assert_eq!(list.resolve_current_point(Position { x: 4, y: 8 }), None);
    assert_eq!(list.resolve_current_point(Position { x: 1, y: 5 }), None);
}

#[test]
fn refresh_preserves_target_and_resolves_a_missing_target_to_the_first_row() {
    let rows = vec![item("a"), item("b"), item("c"), item("d")];
    let mut list = WideMediaList::new();
    list.set_content(rows.clone());
    list.select_target(&"c".to_string());
    list.set_scroll(2);
    list.set_content(rows);
    assert_eq!(list.selected_target(), Some(&"c".to_string()));
    assert_eq!(list.scroll(), 2);
    list.set_content(vec![item("a"), item("b")]);
    assert_eq!(list.selected_target(), Some(&"a".to_string()));
    assert!(list.scroll() <= 1);
}

#[test]
fn title_reveal_defaults_to_always_and_survives_a_content_refresh() {
    let mut carrier = MediaListCarrier::new();
    assert_eq!(
        carrier.wide().title_reveal(),
        MediaListTitleReveal::Always,
        "a list that declares nothing reveals every row's title"
    );
    carrier.set_title_reveal(MediaListTitleReveal::OnSelection);
    carrier.set_content(vec![item("one"), item("two")]);
    assert_eq!(
        carrier.wide().title_reveal(),
        MediaListTitleReveal::OnSelection,
        "an ordinary refresh keeps the list's declared policy"
    );
}

#[test]
fn fixed_row_claims_only_completed_current_frame() {
    let mut list = WideMediaList::new();
    list.set_content(vec![item("one"), item("two")]);
    let area = Rect::new(2, 1, 12, 2);
    list.set_geometry(area, area);
    assert!(!list.claims_current_point(Position { x: 2, y: 1 }));
    paint(&mut list, area);
    assert_eq!(
        list.resolve_current_point(Position { x: 2, y: 1 }),
        Some(&"one".into())
    );
    list.set_geometry(Rect::new(0, 0, 0, 2), area);
    assert!(!list.claims_current_point(Position { x: 2, y: 1 }));
}

/// The one canonical state derivation every list uses: `played` wins, a
/// positive resume position yields `Active` with the bounded percentage (no
/// percentage when the runtime is unknown), and no position is `Ordinary`.
#[test]
fn from_progress_is_the_one_state_derivation() {
    assert_eq!(
        MediaSemanticState::from_progress(true, 50, 100),
        MediaSemanticState::active(Some(50)),
        "a resume position yields Active even when played"
    );
    assert_eq!(
        MediaSemanticState::from_progress(true, 0, 100),
        MediaSemanticState::Played,
        "played without a resume position yields Played"
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 25, 100),
        MediaSemanticState::active(Some(25))
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 50, 0),
        MediaSemanticState::active(None),
        "an unknown runtime keeps Active without a percentage"
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 0, 100),
        MediaSemanticState::Ordinary
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 150, 100),
        MediaSemanticState::active(Some(100)),
        "the percentage is bounded at 100"
    );
}

/// D2 (shared-list-components): a keyboard move after a free scroll
/// re-anchors the viewport to the selection — the key advances the selection
/// by one and the resolved offset pulls that row back into view.
#[test]
fn a_key_after_a_free_scroll_bring_the_selection_back_into_view() {
    let mut list = MediaList::new();
    list.set_content(vec![item("a"), item("b"), item("c"), item("d"), item("e")]);
    let flow = list.row_flow();
    Viewported::scroll_viewport(&mut list, &flow, 3, 100);
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(
        list.viewport_offset(),
        2,
        "the scroll clamped past 'a', so it is out of view"
    );

    let transition = list.delegate_operation(MediaListOperation::Move(1));

    assert_eq!(transition.selected_target, Some("b".to_string()));
    assert_eq!(list.viewport_anchor(), ViewportAnchor::FollowSelection);
    assert_eq!(list.resolve_viewport(3).offset, 1);
}

/// D2 (shared-list-components): refreshed content that still holds the
/// selected target keeps a freely scrolled viewport exactly where it is.
#[test]
fn refreshed_content_keeps_a_freely_scrolled_viewport() {
    let rows = vec![item("a"), item("b"), item("c"), item("d"), item("e")];
    let mut list = MediaList::new();
    list.set_content(rows.clone());
    let flow = list.row_flow();
    Viewported::scroll_viewport(&mut list, &flow, 3, 2);

    list.set_content(rows);

    assert_eq!(list.selected_target(), Some(&"a".to_string()));
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(list.scroll(), 2);
}

/// The item-level derivation: music (track, album, artist) never carries its
/// stored played/resume facts into a row — music is fire-and-forget, so the
/// row stays `Ordinary`. Every other item keeps the canonical derivation.
#[test]
fn item_level_derivation_makes_music_rows_ordinary() {
    let played_music = |item_type: &str| {
        let mut item = mbv_emby_model::test_support::make_item("Music", item_type);
        item.played = true;
        item.runtime_ticks = 1000;
        item.playback_position_ticks = 500;
        item
    };
    for item_type in ["Audio", "MusicAlbum", "MusicArtist"] {
        let item = played_music(item_type);
        assert_eq!(
            MediaSemanticState::from_emby(&item),
            MediaSemanticState::Ordinary,
            "a {item_type} row ignores both its played flag and its resume position"
        );
        assert_eq!(
            MediaSemanticState::from_queue_item(&mbv_queue::QueueItem::Emby(Box::new(item))),
            MediaSemanticState::Ordinary,
            "the queue's music row is ordinary too"
        );
    }

    let mut film = mbv_emby_model::test_support::make_item("The Film", "Movie");
    film.played = true;
    assert_eq!(
        MediaSemanticState::from_emby(&film),
        MediaSemanticState::Played,
        "non-music keeps the canonical derivation"
    );
    film.played = false;
    film.runtime_ticks = 1000;
    film.playback_position_ticks = 500;
    assert_eq!(
        MediaSemanticState::from_emby(&film),
        MediaSemanticState::active(Some(50))
    );
    assert_eq!(
        MediaSemanticState::from_queue_item(&mbv_queue::QueueItem::Emby(Box::new(film))),
        MediaSemanticState::active(Some(50))
    );
}

/// #745: a content-preserving reset keeps the projected rows but returns the
/// list to its normal initial presentation — first selectable row, top of the
/// viewport, no marks, and no retained painted frame.
#[test]
fn reset_presentation_keeps_rows_and_restores_initial_selection_and_viewport() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(vec![item("a"), item("b"), item("c")]);
    carrier.select_target(&"c".to_string());
    carrier.set_scroll(2);
    carrier.toggle_selection(&"b".to_string());
    let area = Rect::new(2, 1, 12, 3);
    paint(carrier.wide_mut(), area);
    assert!(carrier.claims_current_point(Position { x: 2, y: 1 }));

    carrier.reset_presentation();

    assert_eq!(carrier.rows().len(), 3, "rows are retained");
    assert_eq!(carrier.selected_target(), Some(&"a".to_string()));
    assert_eq!(carrier.scroll(), 0);
    assert_eq!(carrier.multi_selection().len(), 0);
    assert!(
        !carrier.claims_current_point(Position { x: 2, y: 1 }),
        "the pre-reset painted geometry is invalidated"
    );
}

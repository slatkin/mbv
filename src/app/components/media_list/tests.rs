use super::{
    MediaKind, MediaList, MediaListCarrier, MediaListRow, MediaListTitleReveal, MediaSemanticState,
    ViewportAnchor, WideMediaList, WideMediaListPaintPolicy,
};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
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
    assert_eq!(list.selected_row_offset(3), Some(2));
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
fn raising_the_window_keeps_the_selections_group_heading_visible() {
    let mut list = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a"),
        item("b"),
        MediaListRow::Heading { text: "B".into() },
        item("c"),
        item("d"),
    ]);

    // Scroll away from the top, then walk the cursor back up into view.
    list.select_last();
    list.set_scroll(4);
    assert_eq!(list.resolve_viewport(2).offset, 4); // bottom branch: B's label + c + d
    list.move_selection(-1); // cursor on "c" (display row 4) — still inside the window
    assert_eq!(list.resolve_viewport(2).offset, 4);
    list.move_selection(-1); // cursor on "b" (display row 2), above the window
    assert_eq!(list.resolve_viewport(2).offset, 2);
    list.move_selection(-1); // cursor on "a" (display row 1)
    assert_eq!(list.resolve_viewport(2).offset, 0); // raised over A's Heading

    // The walk stops at the previous selectable row: only the selection's own
    // group label rides along, never the previous group's rows.
    list.select_index(3); // "c", first item of group B
    list.set_scroll(5);
    assert_eq!(list.resolve_viewport(2).offset, 4); // B's label + c

    // A raise with no label above the selection lands exactly on its row.
    let mut plain = WideMediaList::new();
    plain.set_content(vec![item("a"), item("b"), item("c"), item("d")]);
    plain.select_index(1);
    plain.set_scroll(2);
    assert_eq!(plain.resolve_viewport(2).offset, 1);
}

#[test]
fn refresh_preserves_target_and_locally_clamps_missing_target() {
    let rows = vec![item("a"), item("b"), item("c"), item("d")];
    let mut list = WideMediaList::new();
    list.set_content(rows.clone());
    list.select_target(&"c".to_string());
    list.set_scroll(2);
    list.set_content(rows);
    assert_eq!(list.selected_target(), Some(&"c".to_string()));
    assert_eq!(list.scroll(), 2);
    list.set_content(vec![item("a"), item("b")]);
    assert_eq!(list.selected_target(), Some(&"b".to_string()));
    assert!(list.scroll() <= 1);
}

#[test]
fn fixed_row_owner_survives_wide_narrow_wide_geometry_changes() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content((0..8).map(|i| item(&i.to_string())).collect());
    carrier.select_target(&"5".to_string());
    carrier.set_scroll(5);
    let target = carrier.selected_target().cloned();

    // Both breakpoint arms configure the same fixed-row owner. The smaller
    // viewport clamps on view; no presentation-specific state is transferred.
    carrier.clamp_viewport(3);
    paint(carrier.wide_mut(), Rect::new(0, 0, 20, 3));
    let narrow_offset = carrier.wide().current_flow_offset().unwrap();
    assert_eq!(carrier.selected_target(), target.as_ref());
    assert!(narrow_offset <= 5);

    carrier.clamp_viewport(7);
    paint(carrier.wide_mut(), Rect::new(0, 0, 20, 7));
    assert_eq!(carrier.selected_target(), target.as_ref());
    assert!(carrier.wide().current_flow_offset().unwrap() <= narrow_offset);
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
fn explicit_anchor_clamps_without_changing_fixed_row_owner() {
    let mut list = WideMediaList::new();
    list.set_content((0..6).map(|i| item(&i.to_string())).collect());
    list.select_target(&"3".to_string());
    let anchor = ViewportAnchor {
        selected_target: "3".to_string(),
        selected_row_offset: 5,
    };
    list.apply_viewport_anchor(&anchor, 2);
    assert_eq!(list.selected_target(), Some(&"3".to_string()));
    assert_eq!(list.resolve_viewport(2).offset, 2);
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

#[test]
fn carrier_preserves_multi_selection_across_geometry_changes() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(vec![item("one"), item("two"), item("three")]);
    carrier.toggle_selection(&"one".to_string());
    carrier.toggle_selection(&"three".to_string());
    carrier.clamp_viewport(2);
    assert_eq!(carrier.multi_selection(), &["one", "three"]);
}

#[test]
fn media_list_selection_summary_stays_provider_neutral() {
    let mut list = MediaList::new();
    list.set_content(vec![item("one"), item("two")]);
    list.enter_visual_mode();
    let transition = list.delegate_operation(super::MediaListOperation::Context("two".into()));
    assert!(transition.external_intent.is_some());
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

/// The item-level derivation: music (track, album, artist) never carries its
/// stored played/resume facts into a row — music is fire-and-forget, so the
/// row stays `Ordinary`. Every other item keeps the canonical derivation.
#[test]
fn item_level_derivation_makes_music_rows_ordinary() {
    let played_music = |item_type: &str| {
        let mut item = crate::app::tests::make_item("Music", item_type);
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
            MediaSemanticState::from_queue_item(&mbv_core::playback_queue::QueueItem::Emby(
                Box::new(item)
            )),
            MediaSemanticState::Ordinary,
            "the queue's music row is ordinary too"
        );
    }

    let mut film = crate::app::tests::make_item("The Film", "Movie");
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
        MediaSemanticState::from_queue_item(&mbv_core::playback_queue::QueueItem::Emby(Box::new(
            film
        ))),
        MediaSemanticState::active(Some(50))
    );
}

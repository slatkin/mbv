use super::*;
use crate::library_panel::LibraryPanel;
use mbv_queue::TvContentMode;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use tuirealm::component::Component;

/// Task 4.3: the TV show tree's wheel claims its painted region, scrolls the
/// viewport (`TreeOperation::Scroll`), and leaves the selection where it was.
/// The shared Scroll contract (selection kept, offset clamped) is owned by
/// `list/tree_browser/tests`; this surface test owns the tree wheel routing.
#[test]
fn show_tree_wheel_keeps_the_selection_and_reports_the_claim() {
    let show = tv_show("Alpha", "show-a");
    let other_show = tv_show("Beta", "show-b");
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let episodes: Vec<EmbyItem> = (0..20)
        .map(|i| {
            let mut episode = tv_episode(&format!("Episode {i}"), &format!("episode-{i}"));
            episode.series_id = show.id.clone();
            episode
        })
        .collect();
    let detail = mbv_ui_model::browse::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-1".into(), episodes)].into_iter().collect(),
    };
    let mut owner = TvContent::new();
    owner.set_is_wide(false);
    owner.set_content(tv_tree_context(
        vec![show.clone(), other_show.clone()],
        Some("show-a"),
        Some(detail),
        false,
    ));
    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    let first_episode = TvTreeTarget::Episode {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        season_occurrence: 0,
        episode: "episode-0".into(),
        occurrence: 0,
    };
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_target));
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(season_target));
    owner
        .browser
        .apply(TreeOperation::Select(first_episode.clone()));

    let key = mbv_ui_model::library::LibraryKey::Service {
        service: mbv_queue::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: mbv_ui_model::library::LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key.clone()));
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, 80, 20)))
        .unwrap();

    let owner = panel
        .owner_mut(&key)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<TvContent>())
        .unwrap();
    let painted = |target: &TvTreeTarget| {
        (0..20)
            .flat_map(|y| (0..80).map(move |x| Position::new(x, y)))
            .find(|point| owner.browser.resolve_current_point(*point) == Some(target))
            .expect("the selected episode row is painted in the test frame")
    };
    let at = painted(&first_episode);
    let message = owner.handle_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at,
        delta: 3,
    }));

    assert!(matches!(
        message,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(
        owner.browser.selected_target(),
        Some(&first_episode),
        "the wheel never selects",
    );
}

/// Task 4.3: the flat series rail's wheel scrolls the shared viewport
/// (`MediaListOperation::Scroll`) instead of the old `move_rows` selection
/// move, claims the gesture, and leaves the selection where it was. The
/// shared Scroll contract is owned by `media_list/tests.rs`.
#[test]
fn flat_series_rail_wheel_keeps_the_selection_and_reports_the_claim() {
    let items: Vec<EmbyItem> = (0..8)
        .map(|i| tv_episode(&format!("Episode {i}"), &format!("episode-{i}")))
        .collect();
    let mut owner = TvContent::new();
    let mut context = tv_tree_context(items, Some("episode-0"), None, false);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    owner.set_content(context);

    // Complete one painted frame with a three-row viewport so the wheel's
    // claim gate resolves against retained geometry (the 4.1 test shape).
    let list = Rect::new(0, 0, 40, 3);
    let geometry = owner.carrier.wide().row_geometry(3);
    owner
        .carrier
        .wide_mut()
        .finish_view(list, list, &geometry, None);

    let message = owner.handle_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position::new(1, 1),
        delta: 3,
    }));

    assert!(matches!(
        message,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(
        owner.carrier.selected_target().map(String::as_str),
        Some("episode-0"),
        "the wheel never selects",
    );
    assert_eq!(owner.carrier.scroll(), 3);
}

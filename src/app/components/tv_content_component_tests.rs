//! TV embedded-owner tests (tasks 8.1–8.4). These exercise `TvContent`
//! directly for its content projection, local key interpretation and typed
//! message translation; pointer resolution and painting are exercised through
//! the mounted `LibraryPanel` that hosts the owner (task 8.4 deleted the
//! mounted component, its `ComponentId` and its hit stores).

use super::inline_search::SearchPool;
use super::library_panel::{LibraryContentOwner, LibraryKey, LibraryPanel};
use super::media_list::MediaSemanticState;
use super::msg::{Msg, ShellRequest, TerminalObserverEvent, TvHit};
use super::tv_content::TvContent;
use crate::app::components::LibraryKind;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_item;
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute};

/// The TV owner's `LibraryKey` under the mounted panel: one `Service` key for
/// a `tvshows` library (task 8.4, design D2).
fn tv_key() -> LibraryKey {
    LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    }
}

/// A mounted-panel harness hosting one TV owner: the shape production uses
/// (the panel is the event boundary; the owner never mounts).
struct TvPanel {
    panel: LibraryPanel,
    owner: TvContent,
}

fn panel_with(owner: TvContent, focused: bool) -> LibraryPanel {
    let mut panel = LibraryPanel::new();
    panel.insert_owner(tv_key(), Box::new(owner));
    panel.set_active(Some(tv_key()));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(focused));
    panel
}

fn paint(panel: &mut LibraryPanel, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(panel, frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

fn tv(panel: &LibraryPanel) -> &TvContent {
    panel
        .owner(&tv_key())
        .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
        .expect("TV owner installed")
}

fn tv_mut(panel: &mut LibraryPanel) -> &mut TvContent {
    panel
        .owner_mut(&tv_key())
        .and_then(|owner| owner.as_any_mut().downcast_mut::<TvContent>())
        .expect("TV owner installed")
}

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn down(owner: &mut TvContent, code: Key) -> Option<Msg> {
    owner.on_key(&key(code))
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<super::UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn narrow_tv_dims_watched_and_in_progress_rows_but_wide_never_does() {
    let mut watched = make_item("Watched Series", "Series");
    watched.id = "series-watched".into();
    watched.played = true;

    let mut in_progress = make_item("In Progress Series", "Series");
    in_progress.id = "series-in-progress".into();
    in_progress.runtime_ticks = 1000;
    in_progress.playback_position_ticks = 500;

    let content = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![watched.clone(), in_progress.clone()], 0),
        None,
        None,
        0,
        None,
        false,
    );

    let mut narrow = TvContent::new();
    narrow.set_is_wide(false);
    narrow.set_content(content.clone());
    let narrow_states = narrow.test_row_semantic_states();
    assert!(narrow_states
        .iter()
        .any(|state| matches!(state, MediaSemanticState::Played)));
    assert!(narrow_states
        .iter()
        .any(|state| matches!(state, MediaSemanticState::Active { .. })));

    let mut wide = TvContent::new();
    wide.set_is_wide(true);
    wide.set_content(content);
    let wide_states = wide.test_row_semantic_states();
    assert!(wide_states
        .iter()
        .all(|state| matches!(state, MediaSemanticState::Ordinary)));
}

#[test]
fn tv_series_clicks_use_the_rendered_series_row_for_left_and_right_clicks() {
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(
            vec![
                make_item("Series A", "Series"),
                make_item("Series B", "Series"),
            ],
            0,
        ),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let list_area = panel.test_wide_geometry().unwrap().list_area;
    let row = list_area.y + 1;
    let col = list_area.x;

    let left = panel.on(&mouse(MouseEventKind::Down(MouseButton::Left), col, row));
    assert!(matches!(
        left,
        Some(Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target),
        })) if target == "id"
    ));

    let right = panel.on(&mouse(MouseEventKind::Down(MouseButton::Right), col, row));
    assert!(matches!(
        right,
        Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(ref items), _))) if items.len() == 1 && items[0].id == "id"
    ));
}

#[test]
fn tv_keyboard_context_menu_uses_all_selected_rows_and_single_row_without_selection() {
    let mut first = make_item("Series A", "Series");
    first.id = "series-a".into();
    let mut second = make_item("Series B", "Series");
    second.id = "series-b".into();
    let content = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    );

    let mut selected = TvContent::new();
    selected.set_content(content.clone());
    selected.select_targets_for_test(&["series-a".into(), "series-b".into()]);
    assert!(matches!(
        down(&mut selected, Key::Char('.')),
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            None
        ))) if items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>() == vec!["series-a", "series-b"]
    ));

    let mut single = TvContent::new();
    single.set_content(content);
    assert!(matches!(
        down(&mut single, Key::Char('.')),
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            None
        ))) if items.len() == 1 && items[0].id == "series-a"
    ));
}

#[test]
fn tv_context_click_outside_selection_forwards_cleared_selection() {
    let mut first = make_item("Series A", "Series");
    first.id = "series-a".into();
    let mut second = make_item("Series B", "Series");
    second.id = "series-b".into();
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    tv_mut(&mut panel).select_targets_for_test(&["series-b".into()]);
    let count = tv_mut(&mut panel).context_click_for_test("series-a".into());
    assert_eq!(count, Some(0));
}

#[test]
fn tv_series_hits_use_retained_rows_and_wheel_moves_the_control() {
    let mut first = make_item("Series A", "Series");
    first.id = "series-a".into();
    let mut second = make_item("Series B", "Series");
    second.id = "series-b".into();
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let (col, row) = {
        let list_area = panel.test_wide_geometry().unwrap().list_area;
        (list_area.x, list_area.y)
    };

    let click = panel.on(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        col,
        row + 1,
    ));
    assert!(matches!(
        click,
        Some(Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target),
        })) if target == "series-b"
    ));
    assert_eq!(tv(&panel).selected_item_id(), Some("series-b".into()));

    let blank = panel.on(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        col,
        row.saturating_sub(1),
    ));
    assert!(blank.is_none());

    let wheel = panel.on(&mouse(MouseEventKind::ScrollDown, col, row));
    assert!(matches!(
        wheel,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(tv(&panel).selected_item_id(), Some("series-b".into()));
}

#[test]
fn tv_enter_selects_first_episode_for_activation() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-id".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-id".into(), vec![episode])].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    // Enter on the selected series is the only way into the Episodes pane:
    // arrows never move focus between wide-library panes.
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvActivate { .. }))
    ));
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("episode-id".into())
    );
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvEpisodeActivate { .. }))
    ));
}

#[test]
fn tv_right_does_not_move_focus_between_panes() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-id".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-id".into(), vec![episode])].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(owner.on_key(&key(Key::Right)), None);
    assert_eq!(owner.on_key(&key(Key::Left)), None);
    // Enter still resolves the Series-pane arm — the pane never moved.
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvActivate { .. }))
    ));
}

#[test]
fn tv_content_refresh_clamps_episode_cursor_and_handles_empty_season() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let episode = |name: &str, id: &str| {
        let mut item = make_item(name, "Episode");
        item.id = id.into();
        item
    };
    let detail = |episodes| crate::app::SeriesDetail {
        seasons: vec![season.clone()],
        episodes: [("season-id".into(), episodes)].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        Some(detail(vec![
            episode("Episode 1", "episode-1"),
            episode("Episode 2", "episode-2"),
            episode("Episode 3", "episode-3"),
        ])),
        0,
        None,
        false,
    ));
    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    owner.on_key(&key(Key::Enter));
    owner.on_key(&key(Key::Down));
    owner.on_key(&key(Key::Down));
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("episode-3".into())
    );

    // An unavailable detail refresh must not erase the mounted component's
    // local episode cursor while the data is loading.
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        None,
        0,
        Some(2),
        false,
    ));
    assert_eq!(
        owner.episode_cursor(),
        2,
        "an unavailable detail refresh preserves the episode owner's cursor"
    );

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        Some(detail(vec![episode("Episode 1", "episode-1")])),
        0,
        Some(2),
        false,
    ));
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("episode-1".into())
    );
    assert!(matches!(
        owner.on_key(&KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE
        }),
        Some(Msg::Shell(ShellRequest::TvEpisodeActivate { .. }))
    ));

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail(Vec::new())),
        0,
        Some(0),
        false,
    ));
    assert_eq!(owner.selected_episode_item(), None);
}

#[test]
fn tv_keyboard_leaves_key_unclaimed_when_queue_is_focused() {
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(
            vec![
                make_item("Series A", "Series"),
                make_item("Series B", "Series"),
            ],
            0,
        ),
        None,
        None,
        0,
        None,
        true,
    ));
    // Focus lives on the panel (task 8.4): an unfocused panel forwards
    // nothing to its owner, so the key is unclaimed.
    let mut panel = panel_with(owner, false);
    paint(&mut panel, 100, 20);

    let message = panel.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(message, None);
    assert_eq!(tv(&panel).cursor(), 0);
}

#[test]
fn tv_episode_brackets_with_modifiers_are_unclaimed() {
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0),
        None,
        None,
        0,
        Some(0),
        false,
    ));

    for (code, modifiers) in [
        (Key::Char('['), KeyModifiers::CONTROL),
        (Key::Char(']'), KeyModifiers::ALT),
    ] {
        let message = owner.on_key(&KeyEvent { code, modifiers });
        assert_eq!(message, None);
    }
    assert_eq!(
        owner.on_key(&KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::NONE,
        }),
        None
    );
}

#[test]
fn tv_episode_brackets_wrap_season_selection() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let seasons = (0..3)
        .map(|index| {
            let mut season = make_item(&format!("Season {index}"), "Season");
            season.id = format!("season-{index}");
            season
        })
        .collect();
    let detail = crate::app::SeriesDetail {
        seasons,
        episodes: Default::default(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    owner.on_key(&key(Key::Enter));

    assert!(matches!(
        owner.on_key(&key(Key::Char('['))),
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: -1 }))
    ));
    assert_eq!(
        owner.selected_season(),
        Some(("series-id".into(), "season-2".into()))
    );

    assert!(matches!(
        owner.on_key(&key(Key::Char(']'))),
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));
    assert_eq!(
        owner.selected_season(),
        Some(("series-id".into(), "season-0".into()))
    );
}

#[test]
fn tv_first_mount_seeds_the_stable_target_and_renders_sorted_rows() {
    let mut items = vec![
        make_item("Zulu", "Series"),
        make_item("Alpha", "Series"),
        make_item("Beta", "Series"),
    ];
    items.extend((3..50).map(|index| make_item(&format!("Series {index}"), "Series")));

    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 1),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let owner = tv(&panel);
    assert_eq!(
        owner.selected_item().map(|item| item.display_name()),
        Some("Alpha".to_string()),
        "first mount must resolve the stable target in natural-sort order"
    );
    // First mount seeds the stable target at the shell's item cursor
    // (`items[1]` = Alpha, the first sorted row), not the shell's numeric
    // index as the removed cursor mirror did (design.md D4/D5).
    assert_eq!(owner.cursor(), 0);

    let message = tv_mut(&mut panel).on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    assert_eq!(tv(&panel).cursor(), 1);
}

#[path = "tv_content_component_tests_search.rs"]
mod tv_content_component_tests_search;

use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use super::audiobookshelf_podcast_test_support::{narrow_grid_component_state, view_narrow};
use super::msg::{
    Msg, PodcastEpisodeIntent, PodcastEpisodeTarget, ShellRequest, TerminalObserverEvent,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_core::audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfShow,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// 7.2: a click on a painted episode row resolves through the episode owner's
/// retained current-frame geometry and becomes its show-qualified selected
/// target; a double-click selects through the owner and activates the
/// owner-resolved target.
#[test]
fn abs_podcast_episode_row_click_uses_retained_shared_geometry() {
    let library = AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = AudiobookshelfBrowseState::new(library);
    state.append_page(
        0,
        20,
        1,
        vec![AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    state.select(0);
    state.episodes = Some(vec![
        AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: None,
        },
        AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    // The wide episode rows paint only while the episode pane holds focus.
    component.enter_episode_focus();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let content = component
        .episode_content_rect_for_test()
        .expect("the focused wide episode pane paints its rows");

    // A single click on the second painted row selects that episode through
    // the owner's retained geometry and claims the event.
    let click = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: content.x,
        row: content.y + 1,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.episode_cursor(), 1);
    assert_eq!(
        component.episode_target(),
        Some(PodcastEpisodeTarget::new(
            "show-a".into(),
            "episode-b".into()
        ))
    );
    assert!(matches!(
        click,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));

    // A double-click on the first painted row selects it through the owner
    // and activates its show-qualified target.
    component.reset_mouse_gestures_for_test();
    component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: content.x,
        row: content.y,
        modifiers: KeyModifiers::NONE,
    }));
    let activate = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: content.x,
        row: content.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        activate,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                Some(PodcastEpisodeTarget::new(
                    "show-a".into(),
                    "episode-a".into()
                ))
            ))
        ))
    );
}

#[test]
fn abs_podcast_narrow_one_column_navigation_uses_page_rows() {
    let state = narrow_grid_component_state();
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    view_narrow(&mut component, 100, 6);
    assert!(matches!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE
        })),
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: _
        }))
    ));
    assert!(matches!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::NONE
        })),
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: _
        }))
    ));
    let mut page_component = AudiobookshelfPodcastComponent::new();
    page_component.set_content(&state, false);
    page_component.set_focused(true);
    view_narrow(&mut page_component, 100, 6);
    let page_rows = page_component
        .geometry()
        .list_area
        .height
        .saturating_sub(1)
        .max(1) as usize;
    page_component.on(&Event::Keyboard(KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(page_component.cursor(), 2 + page_rows);
}

#[test]
fn abs_podcast_wheel_moves_one_visual_row_and_ignores_outside_list() {
    let state = narrow_grid_component_state();
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    view_narrow(&mut component, 100, 6);
    let list = component.geometry().list_area;
    let inside = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: list.x,
        row: list.y,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        component.on(&Event::Mouse(inside)),
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: Some("show-11".into())
        }))
    );
    // The wheel throttle lives in the private gesture state (ADR 0024, D3);
    // reset it so the synchronous test loop's second wheel step is recognized.
    component.reset_mouse_gestures_for_test();
    view_narrow(&mut component, 100, 6);
    let up = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: list.x,
        row: list.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        up,
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: Some("show-10".into())
        })),
        "unexpected upward wheel message: {up:?}"
    );
    assert_eq!(component.cursor(), 2);
    assert_eq!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        })),
        None
    );
    assert_eq!(component.cursor(), 2);
}

#[test]
fn abs_podcast_placeholder_does_not_claim_stale_wheel_area_at_any_breakpoint() {
    let library = AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut shows = AudiobookshelfBrowseState::new(library.clone());
    shows.append_page(
        0,
        2,
        2,
        vec![
            AudiobookshelfShow {
                library_item_id: "show-a".into(),
                title: "Show A".into(),
                author: None,
                description: None,
                cover_path: None,
            },
            AudiobookshelfShow {
                library_item_id: "show-b".into(),
                title: "Show B".into(),
                author: None,
                description: None,
                cover_path: None,
            },
        ],
    );
    let empty = AudiobookshelfBrowseState::new(library);

    for (width, height) in [(100, 40), (60, 40)] {
        let mut component = AudiobookshelfPodcastComponent::new();
        component.set_content(&shows, false);
        component.set_focused(true);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| component.view(frame, Rect::new(0, 0, width, height)))
            .unwrap();
        let stale_area = component.geometry().list_area;
        assert!(!stale_area.is_empty());

        component.set_content(&empty, false);
        terminal
            .draw(|frame| component.view(frame, Rect::new(0, 0, width, height)))
            .unwrap();
        let cursor = component.cursor();
        let message = component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: stale_area.x,
            row: stale_area.y,
            modifiers: KeyModifiers::NONE,
        }));

        assert_eq!(
            message, None,
            "placeholder claimed a stale area at width {width}"
        );
        assert_eq!(
            component.cursor(),
            cursor,
            "placeholder changed cursor at width {width}"
        );
    }
}

fn abs_podcast_row_mouse_selects_the_clicked_show_and_bucket_start() {
    let state = narrow_grid_component_state();
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    view_narrow(&mut component, 100, 6);
    let list = component.geometry().list_area;
    let msg = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: list.x,
        row: list.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        msg,
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: Some("show-2".into())
        }))
    );
    let bucket = component.geometry().selector_tabs[0].0;
    let msg = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: bucket.x,
        row: bucket.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        msg,
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: _
        }))
    ));
}

/// Task 5.3d.10c: the component owns its painted geometry (list/right/hero/
/// inline-hero/selected-item rects), so the shell can read it after render
/// ownership moved off `App`. The same mounted component is rendered wide then
/// narrow; the wide right panel must be coherent, and a narrow re-render must
/// not leak the wide `right_area`. A no-show narrow render resets every hero
/// field.
#[test]
fn abs_podcast_component_geometry_is_wide_coherent_and_narrow_resets_wide() {
    let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    });
    state.append_page(
        0,
        10,
        10,
        vec![AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: Some("Author".into()),
            description: Some("An audacious podcast about everything worth hearing.".into()),
            cover_path: None,
        }],
    );
    state.select(0);

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);

    let wide = Rect::new(0, 0, 100, 40);
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal.draw(|frame| component.view(frame, wide)).unwrap();
    let geometry = component.geometry();
    assert!(
        geometry.hero_area.width > 0 && geometry.hero_area.height > 0,
        "wide hero must be painted"
    );
    assert!(
        geometry.right_area.width > 0 && geometry.right_area.height > 0,
        "wide right panel must be painted"
    );
    assert_eq!(
        geometry.list_area, geometry.right_area,
        "wide list == right panel"
    );
    assert_eq!(
        geometry.right_area.x, wide.x,
        "wide browser/list is the left pane"
    );
    assert!(
        geometry.right_area.right() <= geometry.hero_area.x,
        "wide hero sits right of the browser/list pane"
    );
    assert!(geometry.hero_area.bottom() <= wide.bottom());
    assert!(geometry.right_area.right() <= wide.right());
    assert_eq!(
        geometry.inline_hero_area,
        Rect::default(),
        "wide layout has no inline hero"
    );
    assert!(
        geometry.selected_item_rect.is_none(),
        "wide layout has no selected-item shell"
    );

    // Re-render the same mounted component narrow: the wide `right_area` must
    // not survive, and the admitted inline hero must agree across fields.
    let narrow = Rect::new(0, 0, 60, 40);
    terminal
        .draw(|frame| component.view(frame, narrow))
        .unwrap();
    let geometry = component.geometry();
    assert_eq!(
        geometry.right_area,
        Rect::default(),
        "narrow render must reset the wide right_area"
    );
    assert!(
        geometry.list_area.width > 0 && geometry.list_area.height > 0,
        "narrow list area must be nonzero"
    );
    assert!(
        geometry.list_area.y >= narrow.y && geometry.list_area.bottom() <= narrow.bottom(),
        "narrow list sits within the area"
    );
    assert!(
        geometry.hero_area.width > 0 && geometry.hero_area.height > 0,
        "narrow inline hero must be admitted for a short selected show"
    );
    assert_eq!(
        geometry.inline_hero_area, geometry.hero_area,
        "narrow inline hero must equal the painted hero"
    );
    assert_eq!(
        geometry.selected_item_rect,
        Some(geometry.hero_area),
        "narrow selected-item rect must equal the painted hero"
    );
    assert!(geometry.hero_area.right() <= narrow.right());
    assert!(geometry.hero_area.bottom() <= narrow.bottom());

    // No-show narrow render: every hero/right/selected field resets.
    let empty = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    });
    let mut empty_component = AudiobookshelfPodcastComponent::new();
    empty_component.set_content(&empty, false);
    empty_component.set_focused(true);
    let mut empty_terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

    empty_terminal
        .draw(|frame| empty_component.view(frame, wide))
        .unwrap();
    let empty_wide_geometry = empty_component.geometry();
    assert!(
        empty_wide_geometry.right_area.width > 0,
        "no-show wide layout still paints its right placeholder panel"
    );
    assert_eq!(
        empty_wide_geometry.list_area,
        empty_wide_geometry.right_area
    );
    assert_eq!(
        empty_wide_geometry.hero_area,
        Rect::default(),
        "no-show wide layout must not report an unpainted hero"
    );
    assert!(empty_wide_geometry.selected_item_rect.is_none());

    empty_terminal
        .draw(|frame| empty_component.view(frame, narrow))
        .unwrap();
    let empty_narrow_geometry = empty_component.geometry();
    assert_eq!(
        empty_narrow_geometry.list_area, narrow,
        "no-show narrow list_area is the whole area"
    );
    assert_eq!(empty_narrow_geometry.right_area, Rect::default());
    assert_eq!(empty_narrow_geometry.hero_area, Rect::default());
    assert_eq!(empty_narrow_geometry.inline_hero_area, Rect::default());
    assert!(empty_narrow_geometry.selected_item_rect.is_none());
}

/// Task 4.1/4.5: a double-click on a painted show row selects it and emits
/// the existing OpenOrPlay episode intent; a right-click is ignored
/// (task 4.6: no keyboard context-menu equivalent).
#[test]
fn abs_podcast_mouse_double_click_emits_open_or_play_and_right_click_ignored() {
    let state = narrow_grid_component_state();
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    view_narrow(&mut component, 100, 6);
    let rect = component.geometry().list_area;
    let clicked = 0usize;
    // Two quick Downs at the same point = DoubleClick on the second.
    component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    let msg = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        msg,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                None
            ))
        ))
    );
    assert_eq!(component.cursor(), clicked);
    assert_eq!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        })),
        None,
        "task 4.6: right-click must be ignored on this surface"
    );
}

use crate::app::components::{
    AudiobookshelfPodcastComponent, BrowserKey, BrowserKind, ComponentId, Msg,
    TerminalObserverEvent,
};
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use mbv_core::audiobookshelf::AudiobookshelfShow;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn podcast(harness: &mut TickHarness) -> &mut AudiobookshelfPodcastComponent {
    harness
        .model_mut()
        .abs_podcast_component_mut(0)
        .expect("podcast component mounted")
}

fn harness(width: u16) -> TickHarness {
    let mut app = audiobookshelf_app();
    app.terminal_width = width;
    app.terminal_height = 24;
    app.audiobookshelf_browse[0].shows.extend((1..4).map(|index| {
        AudiobookshelfShow {
            library_item_id: format!("show-{index}"),
            title: format!("Show {index}"),
            author: None,
            description: None,
            cover_path: None,
        }
    }));
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn draw(harness: &mut TickHarness, width: u16) {
    harness.model_mut().app.terminal_width = width;
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

fn selected_id(harness: &mut TickHarness) -> String {
    podcast(harness)
        .selected_id()
        .expect("selected show")
        .to_owned()
}

#[test]
fn podcast_tick_navigation_paints_selected_row_at_wide_and_narrow() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut harness = harness(width);
        draw(&mut harness, width);
        assert_eq!(selected_id(&mut harness), "show-a");
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(!outcome.raw_messages.is_empty());
        draw(&mut harness, width);
        assert_eq!(selected_id(&mut harness), "show-1");
    }
}

#[test]
fn podcast_tick_click_resolves_painted_show_and_blank_is_noop() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut harness = harness(width);
        draw(&mut harness, width);
        let row = podcast(&mut harness).geometry().list_area;
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row.x + 1,
            row: row.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(!outcome.raw_messages.is_empty(), "painted show click must be delivered");
        assert_eq!(selected_id(&mut harness), "show-a");

        let before = selected_id(&mut harness);
        let before_paint = podcast(&mut harness).geometry().selected_item_rect;
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert_eq!(
            outcome.raw_messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::MouseClick {
                column: 0,
                row: 0,
            })]
        );
        assert_eq!(selected_id(&mut harness), before);
        assert_eq!(podcast(&mut harness).geometry().selected_item_rect, before_paint);
    }
}

#[test]
fn podcast_tick_wheel_is_claimed_only_over_active_control() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut off = harness(width);
        draw(&mut off, width);
        let before = selected_id(&mut off);
        let before_paint = podcast(&mut off).geometry().selected_item_rect;
        off.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = off.step();
        assert_eq!(
            outcome.raw_messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::NoOp)]
        );
        assert_eq!(selected_id(&mut off), before);
        assert_eq!(podcast(&mut off).geometry().selected_item_rect, before_paint);

        let mut on = harness(width);
        draw(&mut on, width);
        let control = podcast(&mut on).geometry().list_area;
        on.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: control.x + 1,
            row: control.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = on.step();
        assert!(outcome.raw_messages.contains(&Msg::TerminalEvent(
            TerminalObserverEvent::MouseClaimed
        )));
        assert_eq!(selected_id(&mut on), "show-1");
    }
}

#[test]
fn podcast_tick_has_one_list_painter_and_no_base_frame_underpaint() {
    crate::app::render::reset_podcast_media_list_paints();
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(crate::app::render::podcast_wide_media_list_paints(), 1);
    assert_eq!(crate::app::render::podcast_inline_media_browser_paints(), 1);
    assert_eq!(crate::app::render::browser_legacy_plain_rows_paints(), 0);

    crate::app::render::reset_podcast_media_list_paints();
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD - 1);
    assert_eq!(crate::app::render::podcast_wide_media_list_paints(), 1);
    assert_eq!(crate::app::render::podcast_inline_media_browser_paints(), 1);
    assert_eq!(crate::app::render::browser_legacy_plain_rows_paints(), 0);
}

#[test]
fn podcast_tick_round_trip_preserves_selected_target_and_row_offset() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::End,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    let target = selected_id(&mut harness);
    let wide_offset = podcast(&mut harness)
        .selected_row_offset_for_test()
        .expect("wide row offset");

    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD - 1);
    assert_eq!(selected_id(&mut harness), target);
    let narrow_offset = podcast(&mut harness)
        .selected_row_offset_for_test()
        .expect("narrow row offset");
    assert_eq!(narrow_offset, wide_offset);

    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(selected_id(&mut harness), target);
    assert_eq!(podcast(&mut harness).selected_row_offset_for_test(), Some(wide_offset));
}

#[test]
fn podcast_tick_mounts_and_paints_through_shell_sync() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let id = ComponentId::Browser(BrowserKey {
        service: mbv_core::config::ServiceKind::Audiobookshelf,
        library_id: "abs-podcasts".into(),
        kind: BrowserKind::AudiobookshelfPodcast,
    });
    assert!(harness.model().application.get_component(&id).is_some());
}

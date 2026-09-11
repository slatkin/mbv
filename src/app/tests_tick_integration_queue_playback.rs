//! Task 3.7: the Queue playback panel's transport resolves pointer input from
//! its own retained geometry, in the layouts the panel paints it: `both` and
//! mini-view `queue-only`. A collapsed (idle) panel's rows resolve nothing.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind, KeyModifiers};

use crate::app::components::msg::PlaybackRequest;
use crate::app::components::{ComponentId, Msg, QueuePlaybackPanel};
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::types_playback::PlaybackState;
use crate::app::{PanelFocus, PanelMode};

fn click(column: u16, row: u16) -> tuirealm::event::Event<crate::app::components::UserEvent> {
    tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

/// An app with active playback and a non-empty queue, in the given panel
/// mode (mini view uses the ephemeral queue focus).
fn active_app(panel_mode: PanelMode) -> crate::app::App {
    let mut app = crate::app::render::make_queue_app(3);
    app.panel_mode = panel_mode;
    if panel_mode != PanelMode::Both {
        app.mini_view_focus = PanelFocus::Queue;
    }
    app.panel_focus = PanelFocus::Queue;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.queue_len = 3;
        status.current_idx = 0;
        status.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
    }
    app
}

fn playback_intents(outcome: &crate::app::tests_tick_harness::StepOutcome) -> Vec<&PlaybackRequest> {
    outcome
        .raw_messages
        .iter()
        .filter_map(|msg| match msg {
            Msg::Playback(request) => Some(request),
            _ => None,
        })
        .collect()
}

/// Draw one real shell frame (sync pass + `draw_frame`) and return the
/// harness plus the panel's retained transport hit geometry.
fn drawn_harness(mut app: crate::app::App, width: u16, height: u16) -> (TickHarness, Rect, Rect) {
    app.terminal_width = width;
    app.terminal_height = height;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("Queue playback panel mounted in a queue-visible layout");
    let (play_pause, seekbar) = panel.transport_hits();
    (harness, play_pause, seekbar)
}

#[test]
fn tick_clicks_play_pause_and_the_seekbar_in_both() {
    let (mut harness, play_pause, seekbar) =
        drawn_harness(active_app(PanelMode::Both), 100, 40);
    assert!(play_pause.width > 0 && seekbar.width > 0, "transport painted");

    harness.inject(click(play_pause.x + 1, play_pause.y));
    let outcome = harness.step();
    let intents = playback_intents(&outcome);
    assert!(
        matches!(intents.as_slice(), [PlaybackRequest::TogglePlayPause]),
        "play/pause click toggles: {intents:?}"
    );

    let column = seekbar.x + seekbar.width / 2;
    harness.inject(click(column, seekbar.y));
    let outcome = harness.step();
    let intents = playback_intents(&outcome);
    assert_eq!(intents.len(), 1, "exactly one surface claims the click");
    assert!(
        matches!(intents[0], PlaybackRequest::SeekTo(f) if (f - 0.5).abs() < 1e-6),
        "the seekbar click resolves a fraction: {intents:?}"
    );
}

#[test]
fn tick_clicks_play_pause_and_the_seekbar_in_mini_view_queue_only() {
    let (mut harness, play_pause, seekbar) =
        drawn_harness(active_app(PanelMode::QueueOnly), 60, 40);
    assert!(play_pause.width > 0 && seekbar.width > 0, "transport painted");

    harness.inject(click(play_pause.x + 1, play_pause.y));
    let outcome = harness.step();
    let intents = playback_intents(&outcome);
    assert!(
        matches!(intents.as_slice(), [PlaybackRequest::TogglePlayPause]),
        "play/pause click toggles: {intents:?}"
    );

    let column = seekbar.x + seekbar.width / 2;
    harness.inject(click(column, seekbar.y));
    let outcome = harness.step();
    let intents = playback_intents(&outcome);
    assert_eq!(intents.len(), 1, "exactly one surface claims the click");
    assert!(
        matches!(intents[0], PlaybackRequest::SeekTo(f) if (f - 0.5).abs() < 1e-6),
        "the seekbar click resolves a fraction: {intents:?}"
    );
}

#[test]
fn a_click_in_a_collapsed_panels_rows_emits_nothing() {
    let mut app = make_app_stub();
    app.terminal_width = 80;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("Queue playback panel mounted while idle");
    let (play_pause, seekbar) = panel.transport_hits();
    assert_eq!(
        (play_pause.width, seekbar.width),
        (0, 0),
        "the collapsed panel retains no hit geometry"
    );

    // The rows the slot/transport would have occupied — here the panel's
    // collapsed region above the queue panel — resolve no playback intent.
    let chrome = harness.model().app.compute_chrome_geometry(Rect::new(0, 0, 80, 40));
    let collapsed_row = chrome.left_content.y + 1;
    harness.inject(click(chrome.left_content.x + 5, collapsed_row));
    let outcome = harness.step();
    assert!(
        playback_intents(&outcome).is_empty(),
        "a collapsed panel resolves nothing: {:?}",
        outcome.raw_messages
    );
    let _ = PlaybackState::default();
}

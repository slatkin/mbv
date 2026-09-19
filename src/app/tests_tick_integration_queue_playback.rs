//! Task 3.7: the Queue playback panel's transport resolves pointer input from
//! its own retained geometry, in the layouts the panel paints it: `both` and
//! mini-view `queue-only`. A collapsed (idle) panel's rows resolve nothing.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind, KeyModifiers};

use crate::app::components::msg::PlaybackRequest;
use crate::app::components::{ComponentId, LibraryPlaybackPanel, Msg, QueuePlaybackPanel};
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::types_playback::PlaybackState;
use mbv_core::player::PlayerEvent;
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
        status.position_ticks = 45 * mbv_core::api::TICKS_PER_SECOND;
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

/// Task 3.7: the Queue playback panel's transport resolves pointer input from
/// its own retained geometry, in the layouts the panel paints it: `both` and
/// mini-view `queue-only`. A collapsed (idle) panel's rows resolve nothing.
/// Task 4.1: the right-column strip (`LibraryPlaybackPanel`) is mounted only
/// while the queue column is hidden, so it keeps no hit geometry across a
/// layout switch either.
#[cfg(test)]
mod strip_hits {
    use super::*;
    use crate::app::components::LibraryPlaybackPanel;

    /// Draw one real library-only frame (the strip paints and retains its
    /// transport hit geometry), then switch to `both` and draw again.
    fn strip_to_both_harness() -> (TickHarness, ratatui::layout::Rect) {
        let mut app = active_app(PanelMode::LibraryOnly);
        app.panel_focus = PanelFocus::Library;
        app.terminal_width = 100;
        app.terminal_height = 40;
        let mut harness = TickHarness::new(app);
        harness.model_mut().sync_mounted_surfaces();
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
            .unwrap();
        // The strip painted this frame and retained real hit geometry.
        let strip = harness
            .model()
            .app
            .layout
            .root_frame
            .library_playback
            .expect("the strip placed in a library-only layout");
        let playback = harness
            .model()
            .application
            .get_component(&ComponentId::LibraryPlaybackPanel)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPlaybackPanel>())
            .expect("strip mounted in a library-only layout");
        let (play_pause, seekbar) = playback.transport_hits();
        assert!(play_pause.width > 0 && seekbar.width > 0, "strip hits retained");

        // Switch to `both`: the queue column becomes visible and the strip
        // stops painting — the D1 mount rule unmounts it, so neither the
        // component nor its hits survive the switch.
        harness.model_mut().app.panel_mode = PanelMode::Both;
        harness.model_mut().sync_mounted_surfaces();
        terminal
            .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
            .unwrap();
        assert!(!harness
            .model()
            .application
            .mounted(&ComponentId::LibraryPlaybackPanel));
        (harness, strip)
    }

    /// A click in the library column where the strip used to sit reaches no
    /// `LibraryPlaybackPanel` once the layout stops painting it.
    #[test]
    fn switching_from_library_only_to_both_leaves_the_old_strip_rows_inert() {
        let (mut harness, strip) = strip_to_both_harness();

        // The old strip spanned the full right column; its seekbar row was
        // the placement's first row. In `both` the click lands in the library
        // column, past the queue column's 40 cells.
        harness.inject(click(strip.x + 60, strip.y));
        let outcome = harness.step();
        assert!(
            playback_intents(&outcome).is_empty(),
            "the unpainted strip resolves nothing: {:?}",
            outcome.raw_messages
        );
    }
}

#[test]
fn sync_projects_queue_transport_area_before_draw() {
    let mut app = active_app(PanelMode::Both);
    app.terminal_width = 100;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("Queue playback panel mounted in a queue-visible layout");
    let area = panel
        .transport_area_for_test()
        .expect("sync projects transport geometry before draw");
    assert!(area.width > 0 && area.height > 0, "transport area is non-degenerate");
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

/// Task 4.1 (D10): exactly one transport paints per frame, owned by the
/// expected panel — the Queue playback panel's in `both` and `queue-only`,
/// the `LibraryPlaybackPanel`'s strip in `library-only`. The seekbar track's
/// glyph is the transport's signature: every painted cell carrying it must
/// lie inside the owning panel's placement.
#[test]
fn exactly_one_transport_paints_per_frame_owned_by_the_expected_panel() {
    for (mode, width) in [
        (PanelMode::Both, 100),
        (PanelMode::QueueOnly, 60),
        (PanelMode::LibraryOnly, 100),
    ] {
        let queue_owned = mode != PanelMode::LibraryOnly;
        let mut app = active_app(mode);
        app.terminal_width = width;
        app.terminal_height = 40;
        let mut harness = TickHarness::new(app);
        harness.model_mut().sync_mounted_surfaces();
        let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
        // Two frames: the first draw publishes the visual slot's freshly
        // painted size, the second reserves it in the Queue playback panel's
        // placement (the shell's one-frame card publish).
        for _ in 0..2 {
            terminal
                .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
                .unwrap();
        }
        let model = harness.model();
        let root = &model.app.layout.root_frame;
        // The expected panel is mounted and the other is not (the D1 mount
        // rule), and its retained hits cover the transport it painted.
        assert_eq!(
            model.application.mounted(&ComponentId::QueuePlaybackPanel),
            queue_owned,
            "{mode:?}: Queue playback panel mounted exactly when the queue column is visible"
        );
        assert_eq!(
            model.application.mounted(&ComponentId::LibraryPlaybackPanel),
            !queue_owned,
            "{mode:?}: the strip mounted exactly when the queue column is hidden"
        );
        let owner = if queue_owned {
            let panel = model
                .application
                .get_component(&ComponentId::QueuePlaybackPanel)
                .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
                .expect("Queue playback panel mounted");
            let (play_pause, seekbar) = panel.transport_hits();
            assert!(
                play_pause.width > 0 && seekbar.width > 0,
                "{mode:?}: queue-column transport painted"
            );
            root.queue_playback.expect("queue playback placed")
        } else {
            let panel = model
                .application
                .get_component(&ComponentId::LibraryPlaybackPanel)
                .and_then(|component| component.as_any().downcast_ref::<LibraryPlaybackPanel>())
                .expect("strip mounted");
            let (play_pause, seekbar) = panel.transport_hits();
            assert!(
                play_pause.width > 0 && seekbar.width > 0,
                "{mode:?}: strip transport painted"
            );
            root.library_playback.expect("strip placed")
        };

        // Every transport-signature cell in the frame — the seekbar's filled
        // track, a `▔` in the ACCENT foreground — lies inside the owning
        // panel's placement, and the transport painted at all: exactly one
        // transport per frame.
        let buf = terminal.backend().buffer();
        let mut painted = 0;
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                let cell = &buf[(x, y)];
                if cell.symbol() == "\u{2594}" && cell.style().fg == Some(crate::app::palette::ACCENT)
                {
                    painted += 1;
                    assert!(
                        owner.contains((x, y).into()),
                        "{mode:?}: transport cell ({x}, {y}) outside the owning panel {owner:?}"
                    );
                }
            }
        }
        assert!(painted > 0, "{mode:?}: the seekbar track painted");
    }
}

#[test]
fn queue_rows_claim_now_playing_only_for_owner_confirmed_slot() {
    use crate::app::components::media_list::MediaSemanticState;
    use crate::app::components::queue::queue_media_rows;
    use crate::app::tests::make_audio_items;

    let mut app = make_app_stub();
    app.player_tab.set_items(make_audio_items(2), 0);
    let confirmed = app.player_tab.slot_id_at(0).unwrap();
    assert!(matches!(
        app.player_tab.queue.set_active_slot(confirmed),
        mbv_core::playback_queue::QueueMutationResult::Applied(())
    ));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 2;
    }
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let rows = queue_media_rows(
        harness.model().app.player_tab.slots(),
        harness.model().app.displayed_playback_state(),
        None,
    );
    assert!(matches!(
        &rows[0],
        crate::app::components::media_list::MediaListRow::Item {
            semantic_state: MediaSemanticState::NowPlaying { .. }, ..
        }
    ));
    assert!(matches!(
        &rows[1],
        crate::app::components::media_list::MediaListRow::Item {
            semantic_state: MediaSemanticState::Ordinary, ..
        }
    ));

    let confirmed_transition = {
        let (request_id, generation) = harness.model_mut().app.bare_owner.mint_local_transition();
        mbv_core::playback_transition::Transition::new(request_id, generation, confirmed)
    };
    harness
        .model_mut()
        .app
        .bare_owner
        .accept_local_transition(confirmed_transition);
    let target = harness.model().app.player_tab.slot_id_at(1).unwrap();
    let (request_id, generation) = harness.model_mut().app.bare_owner.mint_local_transition();
    let transition = mbv_core::playback_transition::Transition::new(
        request_id, generation, target,
    );
    harness.model_mut().app.bare_owner.accept_local_transition(transition);
    harness.model_mut().app.player.status.lock().unwrap().active = false;
    harness
        .model_mut()
        .app
        .handle_player_event(PlayerEvent::CommandRejected("rejected".into()));
    harness.model_mut().sync_mounted_surfaces();
    let rows = queue_media_rows(
        harness.model().app.player_tab.slots(),
        harness.model().app.displayed_playback_state(),
        None,
    );
    assert!(matches!(
        &rows[1],
        crate::app::components::media_list::MediaListRow::Item {
            semantic_state: MediaSemanticState::Ordinary, ..
        }
    ));
}

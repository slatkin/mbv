//! Task 3.7: the Queue playback panel's transport resolves pointer input from
//! its own retained geometry, in the layouts the panel paints it: `both` and
//! mini-view `queue-only`. A collapsed (idle) panel's rows resolve nothing.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};
use tuirealm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::tests::make_app_stub;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};
use mbv_components::{LibraryPlaybackPanel, QueuePlaybackPanel};
use mbv_ui_model::playback::PlaybackState;
use mbv_ui_msg::PlaybackRequest;
use mbv_ui_msg::{ComponentId, Msg};

fn click(column: u16, row: u16) -> tuirealm::event::Event<mbv_ui_msg::UserEvent> {
    tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

/// The seekbar fill signature: the strip's thin `▔`, or the queue band
/// Gauge's whole `█` cells and fractional eighths `▉`–`▏` (ratatui unicode
/// Gauge; the band's former whole-cell `▓` shades are gone since #862).
fn is_seek_fill(symbol: &str) -> bool {
    matches!(
        symbol,
        "\u{2594}"
            | "\u{2588}"
            | "\u{2589}"
            | "\u{258A}"
            | "\u{258B}"
            | "\u{258C}"
            | "\u{258D}"
            | "\u{258E}"
            | "\u{258F}"
    )
}

/// An app with active playback and a non-empty queue, in the given panel
/// mode (mini view uses the ephemeral queue focus).
fn active_app(panel_mode: PanelMode) -> crate::app::App {
    let mut app = crate::app::tests::render_fixtures::make_queue_app(3);
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
        status.position_ticks = 45 * mbv_emby_model::TICKS_PER_SECOND;
        status.runtime_ticks = 90 * mbv_emby_model::TICKS_PER_SECOND;
    };
    app
}

fn playback_intents(
    outcome: &crate::app::tests::tick_integration::harness::StepOutcome,
) -> Vec<&PlaybackRequest> {
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
        assert!(
            play_pause.width > 0 && seekbar.width > 0,
            "strip hits retained"
        );

        // Switch to `both`: the queue column becomes visible and the strip
        // stops painting — the D1 mount rule unmounts it, so neither the
        // component nor its hits survive the switch.
        harness.model_mut().app.panel_mode = PanelMode::Both;
        harness.model_mut().sync_mounted_surfaces();
        terminal
            .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
            .unwrap();
        assert!(
            !harness
                .model()
                .application
                .mounted(&ComponentId::LibraryPlaybackPanel)
        );
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
fn hidden_visual_slot_collapses_and_restores_queue_geometry_at_both_breakpoints() {
    use mbv_render::arrangements::chrome::{
        PLAYER_BOX_HEIGHT, QUEUE_PLAYBACK_HEADER_ROWS, QUEUE_TRANSPORT_GAP_ROWS,
    };

    for width in [80, 120] {
        let mut app = active_app(PanelMode::QueueOnly);
        app.terminal_width = width;
        app.terminal_height = 40;
        app.images.record_card_size(8, 16);
        let mut harness = TickHarness::new(app);

        harness.model_mut().sync_queue_card_geometry();
        let shown_card = (
            harness.model().app.layout.card.height,
            harness.model().app.layout.card.width,
        );
        assert!(shown_card.0 > 0, "slot has a reservation at width {width}");
        assert!(shown_card.1 > 0, "slot has a width at width {width}");
        let checkpoint = harness.model().app.images.last_card_size();

        harness.model_mut().app.visual_slot_hidden = true;
        // Geometry now reads the projected header visibility (design D2), so
        // refresh the classification before computing the frame; hiding the
        // slot is an unreachable fallback, so the header stays.
        harness.model_mut().sync_queue();
        harness.model_mut().sync_queue_card_geometry();
        assert_eq!(
            (
                harness.model().app.layout.card.height,
                harness.model().app.layout.card.width,
            ),
            (0, 0)
        );
        assert_eq!(
            harness.model().app.images.last_card_size(),
            checkpoint,
            "hiding preserves the paint checkpoint"
        );

        let root = harness
            .model()
            .app
            .compute_chrome_geometry(Rect::new(0, 0, width, 40))
            .root;
        let playback = root.queue_playback.expect("queue playback placed");
        let queue = root.queue.expect("queue placed");
        assert_eq!(
            playback.height,
            QUEUE_PLAYBACK_HEADER_ROWS + PLAYER_BOX_HEIGHT + QUEUE_TRANSPORT_GAP_ROWS,
            "header and transport remain at width {width}"
        );
        assert_eq!(
            queue.y,
            playback.y + QUEUE_PLAYBACK_HEADER_ROWS + PLAYER_BOX_HEIGHT + QUEUE_TRANSPORT_GAP_ROWS,
            "queue starts below the header and transport at width {width}"
        );

        harness.model_mut().app.visual_slot_hidden = false;
        harness.model_mut().sync_queue_card_geometry();
        assert_eq!(
            (
                harness.model().app.layout.card.height,
                harness.model().app.layout.card.width,
            ),
            shown_card
        );
    }
}

/// Design D1 / row 1.2: the header-visibility classification runs on every
/// sync pass, ungated by slot visibility. Hiding the visual slot while the
/// artwork carries the title flips the header visible on the next sync pass.
#[test]
fn hiding_the_visual_slot_flips_the_header_visible_on_the_next_sync_pass() {
    let mut app = active_app(PanelMode::Both);
    app.images.configure_protocol(None, true);
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.images
        .set_image_pickers_for_test(picker, Picker::halfblocks());
    let mut harness = TickHarness::new(app);

    harness.model_mut().sync_queue();
    assert!(
        !harness.model().app.queue_card_projection.header_visible,
        "a capable setup keeps the header hidden while the artwork carries the title"
    );

    harness.model_mut().app.visual_slot_hidden = true;
    harness.model_mut().sync_queue();
    assert!(
        harness.model().app.queue_card_projection.header_visible,
        "hiding the visual slot makes the header carry the title"
    );
}

/// Row 3.1 (design D2): a capable artwork-title setup with a measured card
/// and a queue, so the header classification resolves to the artwork site
/// (hidden) until an unreachable fallback input flips it.
fn capable_artwork_app() -> crate::app::App {
    let mut app = active_app(PanelMode::Both);
    app.images.configure_protocol(None, true);
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.images
        .set_image_pickers_for_test(picker, Picker::halfblocks());
    // (height, width): the card box the overlay is composed for.
    app.images.record_card_size(8, 16);
    app.terminal_width = 100;
    app.terminal_height = 40;
    app
}

/// The mounted `QueuePlaybackPanel`'s projected transport area, if sync
/// published one (idle panels project `None`).
fn projected_transport_area(harness: &TickHarness) -> Option<Rect> {
    harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("Queue playback panel mounted in a queue-visible layout")
        .transport_area_for_test()
}

/// The queue panel placement the shell computes from the same `App` state.
fn queue_placement(harness: &TickHarness, width: u16, height: u16) -> Rect {
    harness
        .model()
        .app
        .compute_chrome_geometry(Rect::new(0, 0, width, height))
        .root
        .queue
        .expect("queue placed")
}

/// Row 3.1 (design D2): the panel's slot-region offset and the placement's
/// header band read one shared `queue_header_rows()` policy, so flipping the
/// projected flag on the artwork site shifts the transport area and the
/// queue placement down by exactly the two header rows. Shell wiring only --
/// the geometry math is row 2.1's arrangement-test contract.
#[test]
fn queue_header_flag_shifts_transport_and_queue_placement_by_two_rows() {
    let mut harness = TickHarness::new(capable_artwork_app());
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness.model().app.queue_card_projection.header_visible,
        "a capable artwork site hides the header"
    );
    let (hidden_transport, hidden_queue) = (
        projected_transport_area(&harness).expect("sync projects transport geometry"),
        queue_placement(&harness, 100, 40),
    );

    // The visualizer is an unreachable fallback (design D1): the header
    // carries the title while the slot stays painted, so the card geometry
    // is unchanged and only the header band appears.
    harness.model_mut().app.visualizer_enabled = true;
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().app.queue_card_projection.header_visible,
        "the visualizer fallback shows the header"
    );
    let (shown_transport, shown_queue) = (
        projected_transport_area(&harness).expect("sync projects transport geometry"),
        queue_placement(&harness, 100, 40),
    );

    assert_eq!(
        (shown_transport.x, shown_transport.y, shown_transport.width),
        (
            hidden_transport.x,
            hidden_transport.y + 2,
            hidden_transport.width
        ),
        "the transport area moves down by the header band"
    );
    assert_eq!(
        (shown_queue.x, shown_queue.y, shown_queue.width),
        (hidden_queue.x, hidden_queue.y + 2, hidden_queue.width),
        "the queue placement moves down by the header band"
    );
}

/// Row 3.1 (design D2): idle is not classified by the title rule -- the shell
/// reserves the whole header band regardless of the projected flag, so
/// flipping it moves nothing.
#[test]
fn idle_keeps_the_header_band_regardless_of_the_projected_flag() {
    let app = capable_artwork_app();
    app.player.status.lock().unwrap().active = false;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let before = queue_placement(&harness, 100, 40);
    assert!(
        projected_transport_area(&harness).is_none(),
        "an idle panel projects no transport area"
    );

    harness.model_mut().app.queue_card_projection.header_visible = true;
    harness.model_mut().sync_queue_playback_panel();
    let after = queue_placement(&harness, 100, 40);

    assert_eq!(
        (after.x, after.y, after.width, after.height),
        (before.x, before.y, before.width, before.height),
        "the idle placement ignores the projected flag"
    );
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
    assert!(
        area.width > 0 && area.height > 0,
        "transport area is non-degenerate"
    );
}

#[test]
fn tick_clicks_play_pause_and_the_seekbar_in_both() {
    let (mut harness, play_pause, seekbar) = drawn_harness(active_app(PanelMode::Both), 100, 40);
    assert!(
        play_pause.width > 0 && seekbar.width > 0,
        "transport painted"
    );

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
    assert!(
        play_pause.width > 0 && seekbar.width > 0,
        "transport painted"
    );

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
    // Collapsed means idle: the stub player starts active (a remote-owner
    // stand-in), which would paint the transport this test expects hidden.
    app.player.status.lock().unwrap().active = false;
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
    let chrome = harness
        .model()
        .app
        .compute_chrome_geometry(Rect::new(0, 0, 80, 40));
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
            model
                .application
                .mounted(&ComponentId::LibraryPlaybackPanel),
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
        // track, the queue band Gauge's `█` whole cells and `▉`–`▏`
        // fractional eighths, or the strip's `▔`, in the ACCENT foreground —
        // lies inside the owning panel's placement, and the transport painted
        // at all: exactly one transport per frame.
        let buf = terminal.backend().buffer();
        let mut painted = 0;
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                let cell = &buf[(x, y)];
                if (is_seek_fill(cell.symbol())) && cell.style().fg == Some(mbv_theme::ACCENT) {
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

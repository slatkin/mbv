//! Task 12.2's sentinel proof: one tick integration test per Panel mode
//! (`both`, `queue-only`, `library-only`, mini view) pre-fills the test
//! buffer with a sentinel symbol, draws one real frame, and asserts no
//! sentinel cell survives inside any mounted panel's `RootFrame` placement.
//! A panel that paints only part of its placement leaves a sentinel cell the
//! assertion catches — proving each mounted panel fills its own surface
//! instead of relying on a full-column backdrop underneath it.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::app::render::{make_movie_app, make_queue_app};
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};

const SENTINEL: &str = "\u{2603}";

/// Draw one real shell frame (sync pass + `draw_frame`) over a buffer
/// pre-filled with `SENTINEL`, then assert the sentinel survives nowhere
/// inside any of the frame's placed `RootFrame` panels.
fn assert_placements_fully_repainted(mut app: crate::app::App, width: u16, height: u16) {
    app.terminal_width = width;
    app.terminal_height = height;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    for cell in &mut terminal.current_buffer_mut().content {
        cell.set_symbol(SENTINEL);
    }
    let completed = terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let buffer = completed.buffer.clone();

    let placements = harness.model().app.layout.root_frame.placements();
    let mut checked_any = false;
    for placement in placements.into_iter().flatten() {
        checked_any = true;
        let rect: Rect = placement.rect();
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                assert_ne!(
                    buffer[(x, y)].symbol(),
                    SENTINEL,
                    "sentinel survived at ({x}, {y}) inside placement {placement:?} \
                     -- the panel did not fully repaint its own placement"
                );
            }
        }
    }
    assert!(checked_any, "at least one panel must be placed");
}

fn active_queue_app(panel_mode: PanelMode) -> crate::app::App {
    let mut app = make_queue_app(3);
    app.panel_mode = panel_mode;
    app.panel_focus = PanelFocus::Queue;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.queue_len = 3;
        status.current_idx = 0;
        status.position_ticks = 45 * mbv_core::api::TICKS_PER_SECOND;
        status.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
    };
    app
}

/// `Both`: tab, library, queue playback, queue, status bar and the queue
/// boundary are all placed and must each fully repaint their own placement.
#[test]
fn both_mode_fills_every_placement() {
    assert_placements_fully_repainted(active_queue_app(PanelMode::Both), 120, 30);
}

/// `QueueOnly`: only the queue playback and queue placements are placed.
#[test]
fn queue_only_mode_fills_every_placement() {
    assert_placements_fully_repainted(active_queue_app(PanelMode::QueueOnly), 120, 30);
}

/// `LibraryOnly`: tab, the library playback strip, library and status bar
/// are placed; no queue-column placement exists.
#[test]
fn library_only_mode_fills_every_placement() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    assert_placements_fully_repainted(app, 120, 30);
}

/// Mini view (< `MINI_VIEW_THRESHOLD` columns): the stored three-state
/// `panel_mode` is ignored in favour of the ephemeral `mini_view_focus`
/// two-state derivation (`App::effective_panel_mode`); this exercises the
/// narrow library-focused mini view.
#[test]
fn mini_view_fills_every_placement() {
    let mut app = make_movie_app();
    app.mini_view_focus = PanelFocus::Library;
    assert_placements_fully_repainted(app, 60, 30);
}

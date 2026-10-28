//! Row 5.3 (design D7, `queue-playback-panel` delta): the queue footer's
//! owner indicator always names the Player owner whose queue the panel
//! shows, and the name flips in the same frame that first shows the peer
//! queue after a connect.

use std::sync::mpsc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::harness::TickHarness;
use crate::app::QueueLink;
use crate::app::tests::{make_items, make_local_daemon_app_stub};
use mbv_remote_player::{DaemonEndpoint, RemotePlayer};
use mbv_render::arrangements::queue::queue_footer_row;

const WIDTH: u16 = 100;
const HEIGHT: u16 = 40;

/// Draw one real shell frame and return the queue footer row's text.
fn footer_row_text(harness: &mut TickHarness) -> String {
    let footer = queue_footer_row(harness.model().app.queue_panel_placement().panel_area)
        .expect("queue footer row in a queue-visible layout");
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    (footer.x..footer.x + footer.width)
        .map(|x| {
            terminal.backend().buffer()[(x, footer.y)]
                .symbol()
                .to_string()
        })
        .collect()
}

// Owns queue-handoff-and-take 5.3: the footer's owner label tracks the shown
// queue's Player owner and flips in the frame that first shows the peer queue
// ("The name SHALL change in the same frame in which the shown queue changes").
#[test]
fn the_footer_owner_label_flips_in_the_frame_that_first_shows_the_peer_queue() {
    let mut app = make_local_daemon_app_stub(make_items(2));
    app.terminal_width = WIDTH;
    app.terminal_height = HEIGHT;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let local_row = footer_row_text(&mut harness);
    assert!(
        local_row.contains(&mbv_emby::device_name()),
        "the footer names the local Owner before a peer connects: {local_row:?}"
    );
    assert!(
        !local_row.contains("peer-owner"),
        "no peer is named before the connect: {local_row:?}"
    );

    let (remote, _peer_events) = RemotePlayer::stub(make_items(3), 0);
    harness.model_mut().app.switch_to_direct_remote(
        &mbv_emby::test_support::make_session("peer-owner", "mbv"),
        remote,
        mpsc::channel().1,
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    harness.step();

    assert_eq!(
        harness.model().app.playing_queue_link(),
        QueueLink::Peer,
        "the connect shows the peer queue"
    );
    let peer_row = footer_row_text(&mut harness);
    assert!(
        peer_row.contains("peer-owner"),
        "the footer names the peer in the frame that first shows its queue: {peer_row:?}"
    );
}

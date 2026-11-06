use std::sync::mpsc::Receiver;

use mbv_ctrl::CtrlCmd;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{Msg, ShellRequest};

use crate::app::tests::{
    QueueViewTestExt, make_app_stub, make_items, make_remote_app_stub_with_cmd_rx,
};

/// The slot id of every unified play-slot command the owner received, in
/// arrival order.
fn played_slot_ids(cmd_rx: &Receiver<CtrlCmd>) -> Vec<u64> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueuePlaySlot { slot_id, .. } => Some(slot_id),
            _ => None,
        })
        .collect()
}

// U17 review finding 2: a GUI single click on a queue row runs the TUI single
// click's App effects, so the Queue panel takes focus.
#[test]
fn queue_row_click_focuses_the_queue_panel_review_u17_2() {
    let mut app = make_app_stub();
    app.playback_queue_mut().adopt_items(make_items(2), 0);
    app.set_panel_focus(PanelFocus::Library);
    let row = app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::QueueRowClick {
        slot_id: Some(row),
    })));

    assert_eq!(app.effective_panel_focus(), PanelFocus::Queue);
}

// U17 review finding 2: a GUI double click on a queue row plays that row
// through the TUI double click's path, so the owner receives the play of the
// row's slot.
#[test]
fn queue_row_activate_plays_the_clicked_row_review_u17_2() {
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    // The direct-remote stub displays its Peer queue, so the rows go there.
    app.playback_queue_mut().adopt_items(make_items(3), 0);
    let row = app
        .displayed_queue()
        .slot_id_at(1)
        .expect("row 1 has a slot");

    app.handle_gui_surface_msg(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
        slot_id: Some(row),
    })));

    assert_eq!(played_slot_ids(&cmd_rx), vec![row.raw()]);
}

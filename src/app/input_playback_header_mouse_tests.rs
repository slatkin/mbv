//! Mouse hit-testing for the one-row playback header's transport
//! controls (issue #112): the play/pause glyph and the next glyph,
//! both of which must reuse the existing playback actions. The next
//! control must not fire when the queue is already at that boundary.
use super::*;
use crate::app::tests::make_app_stub;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

fn left_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn click_play_pause_area_sends_toggle_pause() {
    let mut app = make_app_stub();
    app.layout.playback.play_pause_area = Rect {
        x: 0,
        y: 0,
        width: 2,
        height: 1,
    };
    let rx = app.player.spy_on_commands();
    app.handle_mouse(left_down(0, 0));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
}

#[test]
fn click_next_area_jumps_forward_when_not_last_item() {
    let mut app = make_app_stub();
    app.layout.playback.next_area = Rect {
        x: 5,
        y: 0,
        width: 2,
        height: 1,
    };
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();
    app.handle_mouse(left_down(5, 0));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
}

#[test]
fn click_second_cell_of_wide_next_area_also_jumps_forward() {
    let mut app = make_app_stub();
    app.layout.playback.next_area = Rect {
        x: 5,
        y: 0,
        width: 2,
        height: 1,
    };
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();
    app.handle_mouse(left_down(6, 0));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
}

#[test]
fn click_next_area_is_a_no_op_on_last_item() {
    let mut app = make_app_stub();
    app.layout.playback.next_area = Rect {
        x: 5,
        y: 0,
        width: 2,
        height: 1,
    };
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 1;
    }
    let rx = app.player.spy_on_commands();
    app.handle_mouse(left_down(5, 0));
    assert!(rx.try_recv().is_err(), "last item: next must not fire");
}

#[test]
fn click_next_is_a_no_op_on_single_item_queue() {
    let mut app = make_app_stub();
    app.layout.playback.next_area = Rect {
        x: 5,
        y: 0,
        width: 2,
        height: 1,
    };
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();
    app.handle_mouse(left_down(5, 0));
    assert!(
        rx.try_recv().is_err(),
        "single-item queue: next must not fire"
    );
}

// ── issue #134: mouse regions onto the shared `Command` vocabulary ──────

#[test]
fn double_click_on_queue_row_dispatches_the_same_command_as_enter() {
    use crate::app::tests::make_item;

    let mut app = make_app_stub();
    app.player_tab.set_items(
        vec![
            make_item("Track One", "Audio"),
            make_item("Track Two", "Audio"),
        ],
        1, // cursor already on the second item, as if arrow-keyed there
    );
    app.panel_focus = PanelFocus::Queue;
    app.layout.main.queue_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 10,
    };
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    // Prime the double-click detector: a prior click already landed at
    // this exact cell within the timing window.
    app.last_click_time = Instant::now();
    app.last_click_pos = (2, 2);

    let rx = app.player.spy_on_commands();
    app.handle_mouse(left_down(2, 2));

    assert!(
        matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))),
        "double-click on a queue row must dispatch Command::QueuePlayCursor, \
             the same command the queue tab's Enter key uses"
    );
}

#[test]
fn scroll_wheel_on_volume_pill_dispatches_the_same_command_as_the_keys() {
    use crossterm::event::MouseEventKind;

    let mut app = make_app_stub();
    app.layout.tabbar_vol_area = Rect {
        x: 0,
        y: 0,
        width: 5,
        height: 1,
    };
    let before = app.ui_volume;

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    // ScrollUp maps to a +5 volume delta (mirrors the `+`/`=` keys, which
    // the scroll-wheel path now shares `Command::AdjustVolume` with).
    // Idle (`active == false`) clamps at 200, matching `adjust_volume`'s
    // existing idle-vs-active clamp split -- unrelated to this issue.
    assert_eq!(app.ui_volume, (before as i64 + 5).clamp(0, 200) as u8);
}

#[test]
fn panel_bounds_consume_clicks_over_the_physical_sidebar() {
    let mut app = make_app_stub();
    app.show_help = true;
    app.layout.main.panel_area = Rect::new(0, 0, 31, 24);
    app.layout.main.panel_content_area = Rect::new(2, 3, 27, 19);

    app.handle_mouse(left_down(30, 23));
    assert!(
        app.show_help,
        "clicks inside the resized sidebar must be consumed"
    );

    app.handle_mouse(left_down(31, 23));
    assert!(
        !app.show_help,
        "clicks beyond the sidebar must dismiss the panel"
    );
}

#[test]
fn settings_mouse_rows_follow_inset_content_area() {
    let mut app = make_app_stub();
    app.show_settings = true;
    app.terminal_width = 20;
    app.terminal_height = 10;
    app.layout.settings_content_area = Rect::new(2, 4, 15, 3);
    app.layout.settings_line_of_cursor = vec![0, 1];
    app.settings_cursor = 1;

    app.handle_mouse(left_down(2, 3));
    assert_eq!(app.settings_cursor, 1, "header inset row is not clickable");

    app.handle_mouse(left_down(2, 4));
    assert_eq!(
        app.settings_cursor, 0,
        "first inset content row is clickable"
    );
}

use super::*;

fn chrome_input(area: Rect, header_rows: u16) -> ChromeGeometryInput {
    ChromeGeometryInput {
        area,
        panel_mode: PanelMode::Both,
        panel_focus: PanelFocus::Queue,
        queue_column_width: 40,
        terminal_width: area.width,
        card_height: 0,
        playback_active: false,
        header_rows,
    }
}

/// Design D2 / row 2.1: the arrangement treats `header_rows` as a pure input.
/// At zero the playback region reserves no header rows (idle leaves it with no
/// placement at all) and the queue placement starts at the column's top
/// content row, reclaiming the two rows.
#[test]
fn zero_header_rows_starts_the_queue_at_the_top_content_row() {
    let area = Rect::new(0, 0, 120, 40);
    let chrome = chrome_geometry(chrome_input(area, 0));

    let queue = chrome.root.queue.expect("queue placement is present");
    assert_eq!(queue.y, area.y, "queue starts at the top content row");
    assert_eq!(queue.height, area.height, "queue reclaims the header rows");
    assert_eq!(
        chrome.root.queue_playback, None,
        "no header rows means no playback region while idle"
    );
}

/// Design D2 / row 2.1: at `QUEUE_PLAYBACK_HEADER_ROWS` the placements keep
/// today's shape — the playback region reserves the two header rows and the
/// queue starts below them.
#[test]
fn two_header_rows_keep_the_playback_header_placement() {
    let area = Rect::new(0, 0, 120, 40);
    let chrome = chrome_geometry(chrome_input(area, QUEUE_PLAYBACK_HEADER_ROWS));

    let playback = chrome.root.queue_playback.expect("playback placement");
    let queue = chrome.root.queue.expect("queue placement");
    assert_eq!(playback.height, QUEUE_PLAYBACK_HEADER_ROWS);
    assert_eq!(queue.y, playback.bottom(), "queue starts below the header");
    assert_eq!(queue.height, area.height - QUEUE_PLAYBACK_HEADER_ROWS);
}

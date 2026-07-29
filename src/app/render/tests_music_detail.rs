use super::test_helpers::*;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab, QueueScope, RemoteSlotState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn selectable_artist_header_renders_focused() {
    let mut app = make_power_music_group_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Alpha".into(),
    });

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();
    let header_row = lines
        .iter()
        .position(|line| line.contains("Alpha"))
        .expect("expected Alpha header");

    let hint_row = header_row + 1;
    assert!(
        lines[hint_row].contains("^P: Play | ^A: Enqueue | ^S: Shuffle"),
        "expected the artist action-hint row directly below the header:\n{out}"
    );
    assert!(
        !lines[hint_row].contains("ENTER"),
        "artist action hint should not include the album's ENTER clause:\n{out}"
    );

    assert_eq!(
        layout.cursor_screen_y,
        Some(header_row as u16),
        "selected header should own the screen cursor row"
    );
}

#[test]
fn music_group_pills_render_on_row_below_title_marker() {
    let mut app = make_power_music_group_app();
    app.queue_column_width = 20;
    let width = 100u16;
    let height = 20u16;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &mut layout,
            &mut LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);
    let row0 = out.lines().next().unwrap();

    let row3 = out.lines().nth(3).unwrap();

    assert!(
        !row0.contains("Alpha") && !row0.contains("Beta"),
        "expected pills not on the first row:\n{out}"
    );
    assert!(
        row3.contains("Alpha") && row3.contains("Beta"),
        "expected group pills below the tab bar (no header row):\n{out}"
    );

    let char_x = |line: &str, needle: &str| -> u16 {
        let byte_idx = line.find(needle).expect("needle not found");
        line[..byte_idx].chars().count() as u16
    };

    let right_col_x = app.queue_column_width + COLUMN_GAP;
    let buf = term.backend().buffer();
    assert!(
        row3.chars().take(right_col_x as usize).all(|c| c == ' '),
        "expected the pill row to be confined to the right library column:\n{out}"
    );

    let alpha_x = char_x(row3, "Alpha");
    assert!(
        alpha_x >= right_col_x,
        "expected pills confined to the right column"
    );

    assert!(!layout.selector_tabs.is_empty());
    for (rect, _) in &layout.selector_tabs {
        assert_eq!(rect.y, 3, "expected selector hitboxes on the pills row");
        assert!(
            rect.x >= right_col_x,
            "expected selector hitboxes confined to the right column"
        );
    }

    let spacer_row = out.lines().nth(4).unwrap();
    assert!(
        spacer_row.trim().is_empty(),
        "expected a blank spacer row between the pills and the album list:\n{out}"
    );
    let album_row = out.lines().nth(7).unwrap();
    assert!(
        album_row.contains("Alpha") || album_row.contains("First Album"),
        "expected album list content to start below the pill/spacer rows:\n{out}"
    );
}

#[test]
fn music_group_pills_scroll_within_reserved_space_when_overflowing() {
    let mut app = make_power_music_group_app();
    app.queue_column_width = 20;
    let width = 40u16;
    let height = 20u16;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &mut layout,
            &mut LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);

    let row3 = out.lines().nth(3).unwrap();

    let rchar_x = |line: &str, needle: &str| -> u16 {
        let byte_idx = line.rfind(needle).expect("needle not found");
        line[..byte_idx].chars().count() as u16
    };

    let right_col_x = (app.queue_column_width + COLUMN_GAP) as usize;
    assert!(
        row3.chars().take(right_col_x).all(|c| c == ' '),
        "expected the pill row to be confined to the right library column:\n{out}"
    );

    assert!(!layout.selector_tabs.is_empty());
    for (rect, _) in &layout.selector_tabs {
        assert_eq!(rect.y, 3, "expected pill hitboxes on the pills row");
        assert!(
            rect.x as usize >= right_col_x,
            "expected pill hitboxes confined to the right column"
        );
        assert!(
            rect.x + rect.width <= width,
            "expected pill hitboxes confined to the visible pill row"
        );
    }
}

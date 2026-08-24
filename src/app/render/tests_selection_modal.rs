use super::components::selection_modal::SelectionModalRenderGeometry;
use super::test_helpers::buffer_to_string;
use super::{render_selection_modal_content, SelectionModalRenderModel};
use crate::app::types_selection_modal::{
    SelectionModal, SelectionModalFilter, SelectionModalItem, SelectionModalListState,
    SelectionModalRow, SelectionModalSource,
};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

fn item(name: &str, meta: &str, id: &str) -> SelectionModalRow {
    SelectionModalRow::Item(SelectionModalItem {
        name: name.into(),
        meta: meta.into(),
        id: id.into(),
    })
}

/// Column of the first cell of `text`'s first occurrence, scanning row by
/// row. Only correct for single-width (ASCII) needles, which is all this
/// suite asserts on.
fn find_text_cell(buf: &Buffer, text: &str) -> (u16, u16) {
    let area = *buf.area();
    for y in 0..area.height {
        let mut line = String::new();
        for x in 0..area.width {
            line.push_str(buf[(x, y)].symbol());
        }
        if let Some(idx) = line.find(text) {
            return (idx as u16, y);
        }
    }
    panic!("text not found in buffer: {text:?}");
}

fn render(width: u16, height: u16, modal: SelectionModal) -> Terminal<TestBackend> {
    render_with_layout(width, height, modal).0
}

fn render_with_layout(
    width: u16,
    height: u16,
    modal: SelectionModal,
) -> (Terminal<TestBackend>, SelectionModalRenderGeometry) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut dim_backdrop_active = false;
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = Some(render_selection_modal_content(
                f,
                &mut dim_backdrop_active,
                SelectionModalRenderModel {
                    title: &modal.title,
                    state: &modal.state,
                    cursor: modal.cursor,
                    filter: modal.filter.as_ref(),
                },
            ));
        })
        .unwrap();
    (terminal, geometry.unwrap())
}

#[test]
fn selection_modal_renders_items_with_cursor_row_highlighted() {
    let modal = SelectionModal {
        source: SelectionModalSource::Album {
            album_id: "album-1".into(),
        },
        title: "Tracks".into(),
        state: SelectionModalListState::Ready(vec![
            item("Track One", "3:21", "0"),
            item("Track Two", "4:02", "1"),
        ]),
        cursor: 1,
        filter: None,
    };
    let terminal = render(60, 16, modal);
    let output = buffer_to_string(&terminal);
    assert!(output.contains("Tracks"), "modal title missing: {output}");
    assert!(output.contains("Track One"), "{output}");
    assert!(output.contains("Track Two"), "{output}");

    let buf = terminal.backend().buffer();
    let (x1, y1) = find_text_cell(buf, "Track One");
    let (x2, y2) = find_text_cell(buf, "Track Two");
    assert_ne!(
        buf[(x1, y1)].style().fg,
        buf[(x2, y2)].style().fg,
        "cursor row (Track Two) must be styled differently from a non-cursor row"
    );
}

#[test]
fn selection_modal_header_row_has_no_marker_and_cursor_skips_headers() {
    let modal = SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-1".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Ready(vec![
            SelectionModalRow::Header("Season 1".into()),
            item("Episode 1", "24m", "e1"),
            SelectionModalRow::Header("Season 2".into()),
            item("Episode 2", "24m", "e2"),
        ]),
        cursor: 1,
        filter: None,
    };

    let backend = TestBackend::new(60, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut dim_backdrop_active = false;
    terminal
        .draw(|f| {
            render_selection_modal_content(
                f,
                &mut dim_backdrop_active,
                SelectionModalRenderModel {
                    title: &modal.title,
                    state: &modal.state,
                    cursor: modal.cursor,
                    filter: modal.filter.as_ref(),
                },
            );
        })
        .unwrap();
    let output = buffer_to_string(&terminal);
    assert!(output.contains("Season 1"), "{output}");
    assert!(output.contains("Episode 1"), "{output}");

    let buf = terminal.backend().buffer();
    let (hx, hy) = find_text_cell(buf, "Season 1");
    assert_ne!(
        buf[(hx.saturating_sub(2), hy)].symbol(),
        "▸",
        "header row must not show the item cursor marker"
    );
}

#[test]
fn selection_modal_renders_filter_pills_above_the_list() {
    let modal = SelectionModal {
        source: SelectionModalSource::Podcast {
            library_item_id: "show-1".into(),
        },
        title: "Episodes".into(),
        state: SelectionModalListState::Ready(vec![
            item("Episode 1", "24m", "e1"),
            item("Episode 2", "24m", "e2"),
        ]),
        cursor: 0,
        filter: Some(SelectionModalFilter {
            labels: vec!["All".into(), "Unplayed".into()],
            selected: 0,
        }),
    };
    let (terminal, geometry) = render_with_layout(60, 16, modal);
    let output = buffer_to_string(&terminal);
    assert!(output.contains("All"), "{output}");
    assert!(output.contains("Unplayed"), "{output}");
    assert!(output.contains("Episode 1"), "{output}");
    assert!(output.contains("Episode 2"), "{output}");

    let buf = terminal.backend().buffer();
    let (_, pill_y) = find_text_cell(buf, "All");
    let (_, item_y) = find_text_cell(buf, "Episode 1");
    assert!(
        item_y == pill_y + 2,
        "filter pills must have exactly one parent-background spacer row"
    );
    assert_eq!(geometry.selector_tabs.len(), 2);
    assert!(geometry
        .selector_tabs
        .iter()
        .all(|(rect, _)| rect.height == 1));
    assert_eq!(geometry.rows.len(), 2);
    assert!(geometry.rows.iter().all(|(rect, index)| {
        rect.height == 1 && *index < 2 && geometry.area.contains((rect.x, rect.y).into())
    }));
}

#[test]
fn selection_modal_uses_bounded_frame_and_viewport_for_long_lists() {
    let rows = (0..100)
        .map(|i| item(&format!("Track {i}"), "3:21", &i.to_string()))
        .collect();
    let terminal = render(
        80,
        40,
        SelectionModal {
            source: SelectionModalSource::Album {
                album_id: "album-1".into(),
            },
            title: "Tracks".into(),
            state: SelectionModalListState::Ready(rows),
            cursor: 0,
            filter: None,
        },
    );
    let buf = terminal.backend().buffer();
    let (_, title_y) = find_text_cell(buf, "Tracks");
    assert!(
        title_y > 0,
        "long modal should remain centered instead of filling the frame"
    );
    assert!(!buffer_to_string(&terminal).contains("Track 99"));
}

#[test]
fn filtered_selection_modal_keeps_rows_inside_bounded_frame() {
    let rows = (0..100)
        .map(|i| item(&format!("Track {i}"), "3:21", &i.to_string()))
        .collect();
    let modal = SelectionModal {
        source: SelectionModalSource::Podcast {
            library_item_id: "show-1".into(),
        },
        title: "Episodes".into(),
        state: SelectionModalListState::Ready(rows),
        cursor: 0,
        filter: Some(SelectionModalFilter {
            labels: vec!["All".into(), "Unplayed".into()],
            selected: 0,
        }),
    };
    let terminal = render(80, 40, modal);
    let buf = terminal.backend().buffer();
    let (_, title_y) = find_text_cell(buf, "Episodes");
    let outside_frame_y = title_y + 20;
    let outside_frame = (0..buf.area().width)
        .map(|x| buf[(x, outside_frame_y)].symbol())
        .collect::<String>();
    assert!(
        !outside_frame.contains("Track"),
        "filtered rows must stay inside the max-height frame: {outside_frame:?}"
    );
}

#[test]
fn selection_modal_renders_explicit_status_states() {
    for (state, label) in [
        (SelectionModalListState::Loading, "Loading"),
        (SelectionModalListState::Empty, "No items available"),
    ] {
        let terminal = render(
            60,
            8,
            SelectionModal {
                source: SelectionModalSource::Series {
                    series_id: "series-1".into(),
                },
                title: "Series".into(),
                state,
                cursor: 0,
                filter: None,
            },
        );
        assert!(
            buffer_to_string(&terminal).contains(label),
            "status {label:?} missing from modal"
        );
    }
}

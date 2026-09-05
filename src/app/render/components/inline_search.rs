//! Shared Inline Search painter (design.md D3): one bordered input box and
//! the flat column-aware result list, placed by
//! [`inline_search_areas`](crate::app::render::arrangements::inline_search::inline_search_areas)
//! from the exact library-list area the destination owns. There is no Wide
//! flag; the three-row input is admitted purely from available height.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::inline_search::inline_search_areas;
use crate::app::render::palette;
use crate::app::render::{render_generic_movies_home_video_rows_with_ctx, LibraryListRenderCtx};

fn render_inline_search_input(f: &mut Frame, area: Rect, query: &str, loading: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::ACCENT_ACTIVE))
        .title(Span::styled(
            " Search ",
            Style::default()
                .fg(palette::TEXT_ACCENT_MUTED)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let input_text = if loading {
        format!("{query}\u{2588} [loading\u{2026}]")
    } else {
        format!("{query}\u{2588}")
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            input_text,
            Style::default().fg(palette::TEXT_EMPHASIS),
        )),
        inner,
    );
}

/// Paints one embedded Inline Search frame into `area` and returns the
/// result list's scroll offset to persist (mirrors
/// `render_generic_movies_home_video_rows_with_ctx`). `items` is the
/// caller's already-scored, already-ordered result set (design.md D2); this
/// function only places and paints.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render_inline_search(
    f: &mut Frame,
    area: Rect,
    query: &str,
    loading: bool,
    items: Vec<mbv_core::api::EmbyItem>,
    cursor: usize,
    scroll: usize,
    focused: bool,
    columns: usize,
    layout: &mut LayoutMain,
) -> usize {
    let areas = inline_search_areas(area);
    if let Some(input_area) = areas.input_area {
        render_inline_search_input(f, input_area, query, loading);
    }
    let ctx = LibraryListRenderCtx::from_items(items, cursor, scroll)
        .with_search(query.to_string(), loading);
    render_generic_movies_home_video_rows_with_ctx(
        f,
        areas.result_area,
        &ctx,
        focused,
        columns,
        layout,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn render_inline_search_places_bordered_input_above_the_result_list() {
        let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();
        let mut layout = LayoutMain::default();
        let items = vec![make_item("One", "Movie")];
        terminal
            .draw(|f| {
                render_inline_search(f, f.area(), "on", false, items, 0, 0, true, 1, &mut layout);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "┌", "top border row");
        let row1: String = (0..30)
            .map(|x| buffer.cell((x, 1)).unwrap().symbol())
            .collect();
        assert!(
            row1.contains("on\u{2588}"),
            "query with block cursor inside the border: {row1}"
        );
        assert_eq!(
            buffer.cell((0, 2)).unwrap().symbol(),
            "└",
            "bottom border row"
        );

        let row3: String = (0..30)
            .map(|x| buffer.cell((x, 3)).unwrap().symbol())
            .collect();
        assert!(row3.contains("One"), "result row starts at row 3: {row3}");
        assert_eq!(
            layout.left_area,
            Rect {
                x: 0,
                y: 3,
                width: 30,
                height: 3
            },
            "result area is exactly the input's three rows shorter"
        );
    }

    #[test]
    fn render_inline_search_omits_input_when_area_is_too_short() {
        let mut terminal = Terminal::new(TestBackend::new(30, 2)).unwrap();
        let mut layout = LayoutMain::default();
        let items = vec![make_item("One", "Movie")];
        terminal
            .draw(|f| {
                render_inline_search(f, f.area(), "on", false, items, 0, 0, true, 1, &mut layout);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_ne!(
            buffer.cell((0, 0)).unwrap().symbol(),
            "┌",
            "no border painted"
        );
        let row0: String = (0..30)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect();
        assert!(row0.contains("One"), "list uses the full area: {row0}");
        assert_eq!(
            layout.left_area,
            Rect {
                x: 0,
                y: 0,
                width: 30,
                height: 2
            }
        );
    }
}

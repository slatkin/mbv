use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::{palette, ui_util};
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

const DURATION_COL_W: usize = 8;

pub(in crate::app) fn render_wide_left_tracks(
    f: &mut Frame,
    track_area: &Rect,
    album: &EmbyItem,
    tracks: Option<&[EmbyItem]>,
    tracks_loading: bool,
    track_cursor: Option<usize>,
    left_focused: bool,
    library_focused: bool,
    layout: &mut LayoutMain,
) {
    if track_area.height == 0 {
        return;
    }
    let (track_panel, track_content) =
        hero_left::hero_on_left_recessed_box(f, *track_area, PANE_PAD_X, PANE_PAD_Y);
    if track_content.height == 0 || track_content.width == 0 {
        return;
    }

    let Some(tracks) = tracks else {
        let _ = tracks_loading;
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Loading\u{2026}",
                Style::default().fg(palette::TEXT_MUTED),
            ))),
            track_content,
        );
        return;
    };
    if tracks.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "(no tracks)",
                Style::default().fg(palette::TEXT_MUTED),
            ))),
            track_content,
        );
        return;
    }

    let n = tracks.len();
    let visible = track_content.height as usize;
    let scroll = track_focus_scroll(track_cursor, n, visible);
    let title_col_w = (track_content.width as usize).saturating_sub(DURATION_COL_W);
    layout.wide_music_track_hitmap.clear();

    for vi in 0..visible {
        let ti = scroll + vi;
        if ti >= n {
            break;
        }
        let track = &tracks[ti];
        let row_y = track_content.y + vi as u16;
        let row_rect = Rect {
            x: track_panel.x,
            y: row_y,
            width: track_panel.width,
            height: 1,
        };
        let is_cursor = Some(ti) == track_cursor;
        let selected = is_cursor && left_focused;
        let text_fg = if selected {
            palette::TEXT_FOCUS_ACCENT
        } else if left_focused {
            palette::TEXT_STRONG
        } else {
            palette::TEXT_EMPHASIS
        };
        if is_cursor && left_focused {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
                row_rect,
            );
        }

        let track_num = if track.index_number > 0 {
            format!("{:>2}. ", track.index_number)
        } else {
            format!("{:>2}. ", ti + 1)
        };
        let name_w = title_col_w.saturating_sub(track_num.chars().count());
        let name = ui_util::trunc_str(&track.name, name_w);
        let duration = if track.runtime_ticks > 0 {
            ui_util::fmt_duration_mmss(track.runtime_ticks / mbv_core::api::TICKS_PER_SECOND)
        } else {
            "\u{2014}".to_string()
        };
        let used = track_num.chars().count() + name.chars().count();
        let mut spans = vec![
            crate::app::render::selection_marker(selected, crate::app::render::MarkerEdge::Left),
            Span::raw(" "),
        ];
        spans.push(Span::styled(track_num, Style::default().fg(text_fg)));
        spans.push(Span::styled(name, Style::default().fg(text_fg)));
        let pad = track_content
            .width
            .saturating_sub((used + duration.len() + 1) as u16) as usize;
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        spans.push(Span::styled(
            format!(" {duration}"),
            Style::default().fg(palette::STATUS_AVAILABLE),
        ));
        f.render_widget(Paragraph::new(Line::from(spans)), row_rect);
        layout.wide_music_track_hitmap.push((row_rect, ti));
    }

    if n > visible && library_focused {
        crate::app::render::render_right_scrollbar(
            f,
            track_content,
            n.saturating_sub(visible),
            scroll,
            palette::SCROLLBAR,
        );
    }
    if let Some(cursor) = track_cursor {
        if cursor >= scroll && cursor < scroll + visible {
            layout.selected_item_rect = Some(Rect {
                x: track_content.x,
                y: track_content.y + (cursor - scroll) as u16,
                width: track_content.width,
                height: 1,
            });
        }
    }
    let _ = album;
}

fn track_focus_scroll(track_cursor: Option<usize>, count: usize, visible: usize) -> usize {
    let Some(cursor) = track_cursor else {
        return 0;
    };
    cursor
        .saturating_sub(visible.saturating_sub(1))
        .min(count.saturating_sub(visible))
}

use crate::app::components::playback::PlaybackProjection;
use crate::app::layout::LayoutPlayback;
use crate::app::palette;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub(in crate::app) fn render_playback_chrome_content(
    frame: &mut Frame,
    model: &PlaybackProjection,
) -> LayoutPlayback {
    let mut geometry = LayoutPlayback::default();
    if model.player_area.width > 0 && model.player_area.height > 0 {
        let area = model.player_area;
        let panel_bg = if model.focused {
            palette::SURFACE_FOCUSED
        } else {
            palette::SURFACE_PLAYBACK
        };
        frame.render_widget(Block::default().style(Style::default().bg(panel_bg)), area);
        let seek = Rect { height: 1, ..area };
        geometry.seekbar_area = seek;
        let ratio = if model.state.runtime_ticks > 0 {
            (model.state.position_ticks as f64 / model.state.runtime_ticks as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let filled = (ratio * seek.width as f64).round() as usize;
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("▔".repeat(filled), Style::default().fg(palette::ACCENT)),
                Span::styled(
                    "▔".repeat(seek.width as usize - filled.min(seek.width as usize)),
                    Style::default().fg(palette::PROGRESS_TRACK),
                ),
            ]))
            .style(Style::default().bg(panel_bg)),
            seek,
        );
        let title_area = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };
        let title = model.title.as_deref().unwrap_or("");
        let pos = format_duration(model.state.position_ticks);
        let runtime = format_duration(model.state.runtime_ticks);
        let glyph = if model.state.paused { ">" } else { "||" };
        let left = format!("{glyph}  {title}");
        let right = format!("{pos} / {runtime}");
        let left_w = left.width() as u16;
        let right_w = right.width() as u16;
        let gap = area.width.saturating_sub(left_w + right_w) as usize;
        let mut spans = vec![Span::styled(
            left,
            Style::default().fg(palette::TEXT_STRONG),
        )];
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(
            right,
            Style::default().fg(palette::PLAYBACK_META_FG),
        ));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
            title_area,
        );
        let controls = Rect {
            y: area.y + 2,
            height: area.height.saturating_sub(2),
            ..area
        };
        let controls_text = if model.show_controls {
            format!(
                "  X {}  >>{}",
                if model.stop_available { "stop" } else { "" },
                if model.next_available { " next" } else { "" }
            )
        } else {
            String::new()
        };
        frame.render_widget(
            Paragraph::new(controls_text)
                .style(Style::default().fg(palette::TEXT_MUTED).bg(panel_bg)),
            controls,
        );
        geometry.play_pause_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: 3,
            height: 1,
        };
        geometry.stop_area = Rect {
            x: area.x + 3,
            y: area.y + 2,
            width: 1,
            height: 1,
        };
        geometry.next_area = Rect {
            x: area.x + 7,
            y: area.y + 2,
            width: 2,
            height: 1,
        };
    }
    if model.status_area.width > 0 && model.status_area.height > 0 {
        frame.render_widget(
            Paragraph::new(format!(
                " volume {}{}",
                model.volume,
                if model.muted { " muted" } else { "" }
            ))
            .style(
                Style::default()
                    .fg(palette::TEXT_METADATA)
                    .bg(palette::SURFACE_CHROME),
            ),
            model.status_area,
        );
    }
    geometry
}

fn format_duration(ticks: i64) -> String {
    let seconds = (ticks / TICKS_PER_SECOND).max(0);
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

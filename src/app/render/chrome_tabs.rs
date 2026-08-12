#![allow(unused_imports)]

use super::super::ui_util::*;
use super::indicators;
use crate::app::layout::LayoutPlayback;
use crate::app::{
    palette, App, PanelFocus, RemoteSlotState, TABBAR_LEFT_RESERVE, TABBAR_RIGHT_RESERVE,
};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

impl App {
    /// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
    /// Returns None if the local player is not active.
    /// Callers wrap these in [ ... ] with whatever surrounding style they need.
    pub(super) fn build_status_indicator_spans(&self) -> Option<Vec<Span<'static>>> {
        let data = self.playback_indicator_target().indicator_data(self)?;
        Some(indicators::indicator_spans(
            self.indicator_style,
            &data,
            self.use_nerd_fonts,
        ))
    }

    /// Renders the tab bar within the given 1-row `area` and populates
    /// `layout.tabs_area` / `layout.tabbar_vol_area` for mouse hit testing.
    pub(super) fn render_tabs(
        &mut self,
        f: &mut Frame,
        area: Rect,
        tabs_area_out: &mut Rect,
        tabbar_vol_area_out: &mut Rect,
    ) {
        // Fill the tab bar area with the tab box's own background.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::DARK_BG)),
            area,
        );

        // Tabs render on the second row; first row is padding inside the box.
        let tab_row = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };

        let pb_h: u16 = 2; // 2-col padding inside the coloured box
        let tabs_x = area.x + 1;
        let tabs_w = area
            .width
            .saturating_sub(2 * pb_h + TABBAR_LEFT_RESERVE + TABBAR_RIGHT_RESERVE);
        let tabs_area = Rect {
            x: tabs_x,
            width: tabs_w,
            ..tab_row
        };
        *tabs_area_out = tabs_area;

        let volume = self.playback_display_target().displayed_volume(self);
        let vol_color = if volume > 100 {
            palette::RED
        } else if volume > 60 {
            palette::YELLOW
        } else {
            palette::AQUA
        };
        let vol_spans = vec![
            Span::styled("VOL ", Style::default().fg(palette::PLAYBACK_META_FG)),
            Span::styled(
                volume.to_string(),
                Style::default().fg(vol_color).add_modifier(Modifier::BOLD),
            ),
        ];
        let vol_w: u16 = vol_spans.iter().map(|s| s.content.width() as u16).sum();
        let vol_rect = Rect {
            x: area.x + area.width.saturating_sub(vol_w + pb_h),
            y: tab_row.y,
            width: vol_w,
            height: 1,
        };
        *tabbar_vol_area_out = vol_rect;
        f.render_widget(Paragraph::new(Line::from(vol_spans)), vol_rect);

        let (vis_start, vis_end) = self.visible_tab_range(tabs_w);
        let has_left = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let ind_style = Style::default().fg(palette::WHITE);
        let left_w: u16 = if has_left { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left {
            f.render_widget(
                Paragraph::new("« ").style(ind_style),
                Rect {
                    x: tabs_x,
                    y: tab_row.y,
                    width: 2,
                    height: 1,
                },
            );
        }
        if has_right {
            f.render_widget(
                Paragraph::new(" »").style(ind_style),
                Rect {
                    x: tabs_x + tabs_w.saturating_sub(2),
                    y: tab_row.y,
                    width: 2,
                    height: 1,
                },
            );
        }
        let inner_tabs = Rect {
            x: tabs_x + left_w,
            y: tab_row.y,
            width: tabs_w.saturating_sub(left_w + right_w),
            height: area.height,
        };
        let all_names: Vec<String> = std::iter::once("Home".to_string())
            .chain(self.libs.iter().map(|l| l.library.name.clone()))
            .chain(self.audiobookshelf_libraries.iter().map(|l| l.name.clone()))
            .chain(if self.has_feeds_subscriptions() {
                Some("Feeds".to_string())
            } else {
                None
            })
            .collect();
        let tab_pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        let selected_tab = if tab_pos < vis_start || tab_pos >= vis_end {
            usize::MAX
        } else {
            tab_pos - vis_start
        };
        let tab_titles: Vec<Line> = all_names[vis_start..vis_end]
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let n = n.to_uppercase();
                if i == selected_tab {
                    Line::from(vec![
                        Span::styled("▐", Style::default().fg(palette::AQUA)),
                        Span::styled(
                            format!(" {n}  "),
                            Style::default()
                                .fg(palette::WHITE)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ])
                } else {
                    Line::from(Span::styled(
                        format!("  {n}  "),
                        Style::default().fg(Color::Rgb(73, 81, 86)),
                    ))
                }
            })
            .collect();
        f.render_widget(
            Tabs::new(tab_titles)
                .select(usize::MAX)
                .style(Style::default().fg(palette::SUBTLE))
                .highlight_style(Style::default())
                .divider(Span::raw(""))
                .padding("", ""),
            inner_tabs,
        );
    }
}

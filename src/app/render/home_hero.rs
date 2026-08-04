use super::super::ui_util::*;
use super::home_video::format_release_date;
use super::POWER_RENDER_FILTER;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

/// Pre-wrapped content for the Keep Watching hero panel's metadata column,
/// plus the total row count it needs. Computed once (mirroring
/// `compact_banner_layout`'s measure-before-render pattern) so the caller
/// can size the panel to fit before rendering, and so the title and
/// overview are wrapped exactly once per frame rather than once to measure
/// and again to render.
pub(super) struct KeepWatchingHeroLayout {
    title_lines: Vec<String>,
    show_name: String,
    overview_lines: Vec<String>,
    pub(super) height: u16,
}

impl App {
    /// Image types to request for the Keep Watching hero panel, mirroring
    /// the per-type conventions used for the queue card (`render_power_card`).
    pub(super) fn keep_watching_hero_image_types(
        item: &mbv_core::api::MediaItem,
    ) -> &'static [&'static str] {
        match item.item_type.as_str() {
            "Movie" => &["Backdrop", "Primary", "Logo"],
            _ => &["Primary", "Backdrop"],
        }
    }

    /// Builds the Keep Watching hero panel's metadata layout for `item` at
    /// the meta column's width: title wrap lines, then one row each for the
    /// show-name line, the duration/progress line, and the blank separator,
    /// then the wrapped overview.
    pub(super) fn keep_watching_hero_layout(
        item: &mbv_core::api::MediaItem,
        text_w: usize,
    ) -> KeepWatchingHeroLayout {
        if text_w == 0 {
            return KeepWatchingHeroLayout {
                title_lines: Vec::new(),
                show_name: String::new(),
                overview_lines: Vec::new(),
                height: 0,
            };
        }
        let title_lines: Vec<String> = wrap(&item.name, text_w)
            .into_iter()
            .map(|s| s.into_owned())
            .collect();
        let show_name = if item.item_type == "Episode" {
            item.series_name.clone()
        } else {
            String::new()
        };
        let overview_lines: Vec<String> = if item.overview.is_empty() {
            Vec::new()
        } else {
            let ov_w = text_w.saturating_sub(4); // 2-col padding each side
            wrap(&clean_overview(&item.overview), ov_w)
                .into_iter()
                .map(|s| s.into_owned())
                .collect()
        };
        let height = title_lines.len() as u16 // title
            + if show_name.is_empty() { 0 } else { 1 } // show name row (only for episodes)
            + 1 // duration / progress row
            + 1 // blank separator row
            + if overview_lines.is_empty() {
                0
            } else {
                1 + overview_lines.len() as u16 + 1 // overview block: top pad + lines + bottom pad
            };
        KeepWatchingHeroLayout {
            title_lines,
            show_name,
            overview_lines,
            height,
        }
    }

    /// Renders the Keep Watching hero panel's image column into `area`,
    /// top-aligned and, in wide two-column layouts, horizontally centered. The column is a fixed reserved
    /// box (unlike the queue card's growing/shrinking slot), so a dim
    /// placeholder simply fills it while no artwork is ready yet.
    pub(super) fn render_keep_watching_hero_image(
        &mut self,
        f: &mut Frame,
        area: Rect,
        cache_key: &str,
        centered: bool,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let img_area = area;
        if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = Size {
                width: img_area.width,
                height: img_area.height,
            };
            if let Some(actual) = state.size_for(
                ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                avail,
            ) {
                let img_rect = Rect {
                    x: if centered {
                        img_area.x + img_area.width.saturating_sub(actual.width) / 2
                    } else {
                        img_area.x + img_area.width.saturating_sub(actual.width)
                    },
                    y: img_area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
                    img_rect,
                    state,
                );
                return;
            }
        }
        f.render_widget(
            Block::default().style(Style::default().bg(palette::OVERLAY)),
            img_area,
        );
    }

    /// Renders the Keep Watching hero panel's metadata column for the
    /// focused item: episode title (yellow, wraps), show name (green), a
    /// duration/percent-watched line, a blank separator row, then the full
    /// overview (the caller sizes the panel via
    /// `keep_watching_hero_meta_height` so nothing here gets clipped).
    pub(super) fn render_keep_watching_hero_meta(
        &self,
        f: &mut Frame,
        area: Rect,
        item: &mbv_core::api::MediaItem,
        layout: &KeepWatchingHeroLayout,
        focused: bool,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let text_w = area.width as usize;
        let mut row = area.y;
        let max_y = area.y + area.height;

        for line in &layout.title_lines {
            if row >= max_y {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(palette::YELLOW)
                        .add_modifier(Modifier::BOLD),
                )),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        if row < max_y && !layout.show_name.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    trunc_str(&layout.show_name, text_w),
                    Style::default().fg(palette::FOAM),
                )),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        if row < max_y {
            let release_date = if item.premiere_date.is_empty() {
                String::new()
            } else {
                format_release_date(&item.premiere_date)
            };
            let dur_str = if item.runtime_ticks > 0 {
                fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
            } else {
                String::new()
            };
            let progress_span =
                if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                    let pct =
                        (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    Some(Span::styled(
                        format!("{}% watched", pct),
                        Style::default().fg(palette::BG_GREEN),
                    ))
                } else if !item.played {
                    Some(Span::styled(
                        "Unwatched",
                        Style::default().fg(palette::MUTED),
                    ))
                } else {
                    None
                };

            let mut spans: Vec<Span> = Vec::new();
            if !release_date.is_empty() {
                spans.push(Span::styled(
                    release_date,
                    Style::default().fg(palette::SUBTLE),
                ));
            }
            if !dur_str.is_empty() {
                if !spans.is_empty() {
                    spans.push(Span::raw("  "));
                }
                spans.push(Span::styled(
                    trunc_str(&dur_str, text_w),
                    Style::default().fg(palette::SUBTLE),
                ));
            }
            if let Some(progress_span) = progress_span {
                if !spans.is_empty() {
                    spans.push(Span::raw("  "));
                }
                spans.push(progress_span);
            }
            if !spans.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
            }
            row += 1;
        }

        row += 1; // blank separator row

        if !layout.overview_lines.is_empty() && row < max_y {
            let ov_color = if focused {
                palette::WHITE
            } else {
                palette::MUTED
            };
            let block = Block::default().style(Style::default().bg(palette::PLAYBACK_PANEL_BG));
            // 2-col horizontal padding, 1-row top padding
            let block_h = 1 + layout.overview_lines.len() as u16 + 1; // top pad + lines + bottom pad
            let block_area = Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: block_h,
            };
            f.render_widget(block, block_area);
            let inner = Rect {
                x: block_area.x + 2,
                y: block_area.y + 1,
                width: block_area.width.saturating_sub(4),
                height: block_area.height.saturating_sub(2),
            };
            let overview_text: Vec<Line> = layout
                .overview_lines
                .iter()
                .map(|line| Line::from(Span::styled(line.clone(), Style::default().fg(ov_color))))
                .collect();
            f.render_widget(Paragraph::new(overview_text), inner);
        }
    }
}

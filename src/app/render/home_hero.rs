use super::super::ui_util::*;
use super::home_video::format_release_date;
use super::RENDER_FILTER;
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
    /// Overview text lines with a per-line flag: `true` once the line has
    /// wrapped past the image's row extent and reclaims the full hero
    /// width (the image no longer occupies that row), `false` while beside
    /// the image at the narrower meta-column width.
    overview_lines: Vec<(String, bool)>,
    pub(super) height: u16,
}

impl App {
    /// Image types to request for the Keep Watching hero panel, mirroring
    /// the per-type conventions used for the queue card (`render_card`).
    pub(super) fn keep_watching_hero_image_types(
        item: &mbv_core::api::EmbyItem,
    ) -> &'static [&'static str] {
        match item.item_type.as_str() {
            "Movie" => &["Backdrop", "Primary", "Logo"],
            _ => &["Primary", "Backdrop"],
        }
    }

    /// Builds the Keep Watching hero panel's metadata layout for `item`:
    /// title wrap lines, then one row each for the show-name line, the
    /// duration/progress line, and the blank separator, then the wrapped
    /// overview. The overview wraps around the image: it wraps at `text_w`
    /// (the meta column, beside the image) for however many of its rows
    /// still fall within `image_rows`, then reclaims the full `wide_w` for
    /// any remaining rows once past the image's bottom edge. `overview_pad`
    /// is the two-column (wide) hero's original 2-col horizontal padding
    /// around the overview block; the single-column hero passes 0 so its
    /// overview stays flush with the title above it.
    pub(super) fn keep_watching_hero_layout(
        item: &mbv_core::api::EmbyItem,
        text_w: usize,
        wide_w: usize,
        image_rows: u16,
        overview_pad: usize,
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
        let header_rows = title_lines.len() as u16
            + if show_name.is_empty() { 0 } else { 1 } // show name row (only for episodes)
            + 1 // duration / progress row
            + 1; // blank separator row
        let ov_text_w = text_w.saturating_sub(overview_pad * 2);
        let ov_wide_w = wide_w.saturating_sub(overview_pad * 2);
        let overview_lines: Vec<(String, bool)> = if item.overview.is_empty() {
            Vec::new()
        } else {
            let cleaned = clean_overview(&item.overview);
            let narrow_capacity = image_rows.saturating_sub(header_rows) as usize;
            if narrow_capacity == 0 {
                wrap(&cleaned, ov_wide_w.max(1))
                    .into_iter()
                    .map(|s| (s.into_owned(), true))
                    .collect()
            } else {
                let narrow_all: Vec<String> = wrap(&cleaned, ov_text_w)
                    .into_iter()
                    .map(|s| s.into_owned())
                    .collect();
                if narrow_all.len() <= narrow_capacity {
                    narrow_all.into_iter().map(|l| (l, false)).collect()
                } else {
                    let consumed_words: usize = narrow_all[..narrow_capacity]
                        .iter()
                        .map(|l| l.split_whitespace().count())
                        .sum();
                    let remainder: String = cleaned
                        .split_whitespace()
                        .skip(consumed_words)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let mut lines: Vec<(String, bool)> = narrow_all[..narrow_capacity]
                        .iter()
                        .cloned()
                        .map(|l| (l, false))
                        .collect();
                    lines.extend(
                        wrap(&remainder, ov_wide_w.max(1))
                            .into_iter()
                            .map(|s| (s.into_owned(), true)),
                    );
                    lines
                }
            }
        };
        let height = header_rows
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
        if let Some(state) = self.cached_image_protocol_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = Size {
                width: img_area.width,
                height: img_area.height,
            };
            if let Some(actual) =
                state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
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
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    img_rect,
                    state,
                );
                return;
            }
        }
        f.render_widget(
            Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
            img_area,
        );
    }

    /// Renders the Keep Watching hero panel's metadata column for the
    /// focused item: episode title (yellow, wraps), show name (green), a
    /// duration/percent-watched line, a blank separator row, then the full
    /// overview (the caller sizes the panel via
    /// `keep_watching_hero_meta_height` so nothing here gets clipped).
    /// Overview rows past the image's bottom edge (`layout`'s wide lines)
    /// render across `wide_area` instead of the narrower `area`, so the
    /// text wraps around the image rather than staying squeezed beside it
    /// for its full height. `overview_pad` insets the overview text within
    /// its background row (the two-column hero's original 2-col padding);
    /// the single-column hero passes 0 to stay flush with the title.
    pub(super) fn render_keep_watching_hero_meta(
        &self,
        f: &mut Frame,
        area: Rect,
        wide_area: Rect,
        item: &mbv_core::api::EmbyItem,
        layout: &KeepWatchingHeroLayout,
        focused: bool,
        overview_pad: u16,
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
            // The overview "card" blends with the panel surface: green in the
            // single-column layout while the whole Home panel is green
            // (`overview_pad == 0` identifies that layout), library surface
            // otherwise.
            let block_bg = if overview_pad == 0 && focused {
                palette::SURFACE_FOCUSED
            } else {
                palette::SURFACE_BACKDROP
            };
            let bg_row = |wide: bool, y: u16| -> Rect {
                if wide {
                    Rect {
                        x: wide_area.x,
                        y,
                        width: wide_area.width,
                        height: 1,
                    }
                } else {
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    }
                }
            };
            let first_wide = layout.overview_lines[0].1;
            let last_wide = layout.overview_lines[layout.overview_lines.len() - 1].1;
            f.render_widget(
                Block::default().style(Style::default().bg(block_bg)),
                bg_row(first_wide, row),
            );
            row += 1;
            for (line, wide) in &layout.overview_lines {
                if row >= max_y {
                    break;
                }
                let r = bg_row(*wide, row);
                f.render_widget(Block::default().style(Style::default().bg(block_bg)), r);
                let text_r = Rect {
                    x: r.x + overview_pad,
                    width: r.width.saturating_sub(overview_pad * 2),
                    ..r
                };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(ov_color),
                    ))),
                    text_r,
                );
                row += 1;
            }
            if row < max_y {
                f.render_widget(
                    Block::default().style(Style::default().bg(block_bg)),
                    bg_row(last_wide, row),
                );
            }
        }
    }
}

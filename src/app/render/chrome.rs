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

pub(super) fn thin_vertical_thumb(mut glyphs: GlyphSet) -> GlyphSet {
    glyphs.thumb_vertical_lower = ['▕'; 8];
    glyphs.thumb_vertical_upper = ['▕'; 8];
    glyphs
}

pub(super) const PLAY_ICON: &str = "\u{f04b}";
pub(super) const LIST_PLAY_ICON: &str = "▶";
const PLAY_ICON_FALLBACK: &str = ">";

pub(super) fn play_icon(use_nerd_fonts: bool) -> &'static str {
    if use_nerd_fonts {
        PLAY_ICON
    } else {
        PLAY_ICON_FALLBACK
    }
}

pub(super) fn daemon_endpoint_label(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.eq_ignore_ascii_case("local") {
        return None;
    }
    if let Some(tcp) = endpoint.strip_prefix("tcp://") {
        return tcp
            .rsplit_once(':')
            .map(|(host, _port)| host)
            .filter(|host| !host.is_empty())
            .map(str::to_string);
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
    }
    std::path::Path::new(endpoint)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

pub(super) fn server_url_label(server_url: &str) -> Option<String> {
    let value = server_url.trim();
    if value.is_empty() {
        return None;
    }
    let without_scheme = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value);
    without_scheme
        .split('/')
        .next()
        .and_then(|host_port| host_port.split('@').next_back())
        .and_then(|host_port| host_port.split(':').next())
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

impl App {
    pub(super) fn toast_line(s: &str, fg: Color) -> Line<'static> {
        let text_style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        let open = s.find(['[', '(']);
        if let Some(i) = open {
            let close = s[i..].find([']', ')']).map(|j| i + j);
            if let Some(j) = close {
                let mut spans = vec![
                    Span::styled(s[..i].to_string(), text_style),
                    Span::styled(s[i..i + 1].to_string(), text_style),
                ];
                for c in s[i + 1..j].chars() {
                    spans.push(Span::styled(c.to_string(), text_style));
                }
                spans.push(Span::styled(s[j..j + 1].to_string(), text_style));
                if j + 1 < s.len() {
                    spans.push(Span::styled(s[j + 1..].to_string(), text_style));
                }
                return Line::from(spans);
            }
        }
        Line::from(Span::styled(s.to_string(), text_style))
    }

    pub(super) fn render_panel_shell(
        f: &mut Frame,
        full: Rect,
        width: u16,
        title: &str,
        hints: &str,
    ) -> Rect {
        let sidebar = Rect {
            x: full.x,
            y: full.y + 2,
            width: width.min(full.width),
            height: full.height.saturating_sub(2),
        };
        Self::render_panel_shell_at(f, sidebar, title, hints, false)
    }

    pub(super) fn panel_content_area(sidebar: Rect) -> Rect {
        Rect {
            x: sidebar.x,
            y: sidebar.y + 1,
            width: sidebar.width.saturating_sub(1),
            height: sidebar.height.saturating_sub(3),
        }
    }

    pub(super) fn left_panel_content_area(sidebar: Rect) -> Rect {
        Rect {
            x: sidebar.x + 2,
            y: sidebar.y + 3,
            width: sidebar.width.saturating_sub(4),
            height: sidebar.height.saturating_sub(5),
        }
    }

    pub(super) fn settings_content_area(content: Rect) -> Rect {
        Rect {
            x: content.x.saturating_add(2),
            y: content.y.saturating_add(1),
            width: content.width.saturating_sub(4),
            height: content.height.saturating_sub(2),
        }
    }

    pub(super) fn render_panel_shell_at(
        f: &mut Frame,
        sidebar: Rect,
        title: &str,
        hints: &str,
        style: bool,
    ) -> Rect {
        f.render_widget(Clear, sidebar);
        // Too short to fit a title row, a content row, and the 2-row footer;
        // bail out rather than let `footer_y = sidebar.y + sidebar.height - 2`
        // underflow below.
        if sidebar.height < 4 || sidebar.width == 0 {
            return if style {
                Self::left_panel_content_area(sidebar)
            } else {
                sidebar
            };
        }
        f.render_widget(
            Block::default().style(Style::default().bg(if style {
                palette::PLAYBACK_PANEL_BG
            } else {
                palette::PANEL_BG
            })),
            sidebar,
        );
        if !style {
            for row in sidebar.y..sidebar.y + sidebar.height {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "\u{2502}",
                        Style::default().fg(palette::OVERLAY),
                    )),
                    Rect {
                        x: sidebar.x + sidebar.width - 1,
                        y: row,
                        width: 1,
                        height: 1,
                    },
                );
            }
        }
        let (inner_w, ix) = if style {
            (sidebar.width.saturating_sub(4), sidebar.x + 2)
        } else {
            (sidebar.width.saturating_sub(1), sidebar.x)
        };
        let header_style = Style::default()
            .fg(palette::TEXT)
            .bg(if style {
                palette::QUEUE_BUTTON_FOCUSED_BG
            } else {
                palette::FOCUSED
            })
            .add_modifier(Modifier::BOLD);
        let header_area = if style {
            Rect {
                x: sidebar.x + 2,
                y: sidebar.y + 1,
                width: sidebar.width.saturating_sub(4),
                height: 1,
            }
        } else {
            Rect {
                x: sidebar.x,
                y: sidebar.y,
                width: sidebar.width.saturating_sub(1),
                height: 1,
            }
        };
        let title_text = if style {
            format!(" {}", title)
        } else {
            title.to_owned()
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(title_text, header_style)])).style(
                if style {
                    Style::default().bg(palette::QUEUE_BUTTON_FOCUSED_BG)
                } else {
                    Style::default().bg(palette::FOCUSED)
                },
            ),
            header_area,
        );
        if !style {
            f.render_widget(
                Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
                Rect {
                    x: sidebar.x + sidebar.width - 1,
                    y: sidebar.y,
                    width: 1,
                    height: 1,
                },
            );
        }
        let footer_y = sidebar.y + sidebar.height - 2;
        if !style {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "\u{2500}".repeat(inner_w as usize),
                    Style::default().fg(palette::OVERLAY),
                )),
                Rect {
                    x: ix,
                    y: footer_y,
                    width: inner_w,
                    height: 1,
                },
            );
        }
        let footer_bg = if style {
            palette::DARK_BG
        } else {
            palette::FOCUSED
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                trunc_str(hints, inner_w as usize),
                Style::default().fg(palette::TEXT),
            )]))
            .style(Style::default().bg(footer_bg)),
            Rect {
                x: ix,
                y: footer_y,
                width: inner_w,
                height: 1,
            },
        );
        if style {
            f.render_widget(
                Paragraph::new(Span::raw(""))
                    .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
                Rect {
                    x: sidebar.x,
                    y: sidebar.y + sidebar.height - 1,
                    width: sidebar.width,
                    height: 1,
                },
            );
        }
        if !style {
            f.render_widget(
                Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
                Rect {
                    x: sidebar.x + sidebar.width - 1,
                    y: footer_y,
                    width: 1,
                    height: 1,
                },
            );
        }
        if style {
            Self::left_panel_content_area(sidebar)
        } else {
            Self::panel_content_area(sidebar)
        }
    }

    /// Overlay a thin scroll indicator on a sidebar's right border column when
    /// its content doesn't fit `content.height`. Reuses the existing border
    /// column instead of reserving a dedicated width for a scrollbar.
    pub(super) fn render_sidebar_scrollbar(
        f: &mut Frame,
        content: Rect,
        total: usize,
        scroll: usize,
    ) {
        super::widgets::render_scrollbar_with_viewport_at(
            f,
            content,
            total,
            content.height as usize,
            scroll,
            content.x.saturating_add(content.width),
            thin_vertical_thumb(GlyphSet::box_drawing()),
            palette::SCROLLBAR,
        );
    }

    /// Render one row in a sidebar panel list.
    /// `content_spans` should not include the indicator — it is prepended automatically.
    /// Returns the usable text width (content area minus indicator and space).
    pub(super) fn panel_row_text_width(content_width: u16) -> usize {
        content_width.saturating_sub(1) as usize // indicator char
    }

    pub(super) fn render_panel_row(
        f: &mut Frame,
        x: u16,
        y: u16,
        width: u16,
        selected: bool,
        spans: Vec<Span>,
    ) {
        let indicator = Span::styled(
            if selected { "\u{258c}" } else { " " },
            Style::default().fg(palette::AQUA),
        );
        let mut all = vec![indicator];
        all.extend(spans);
        f.render_widget(
            Paragraph::new(Line::from(all)),
            Rect {
                x,
                y,
                width,
                height: 1,
            },
        );
    }
}

use super::super::super::palette;
use super::super::super::panel_targets::PanelTarget;
use super::super::super::ui_util::{fmt_duration_short, trunc_str};
use super::super::super::SESSIONS_PANEL_W;
use super::chrome;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Paint the Sessions sidebar from an owned runtime snapshot and return the
/// panel rect plus the fixed-stride row hit targets produced by this frame.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render_sessions_overlay_content(
    f: &mut Frame,
    area: Option<Rect>,
    panel_targets: &[PanelTarget],
    sessions_loading: bool,
    cursor: &mut usize,
    scroll: &mut usize,
    connected_session_id: Option<&str>,
    tracking: bool,
    cast_attachment_id: Option<&str>,
    can_disconnect: bool,
) -> (Rect, Vec<(Rect, usize)>) {
    let footer = if can_disconnect {
        "[↵]conn [d]disc [r]refresh [Esc]close"
    } else {
        "[↵]conn [r]refresh [Esc]close"
    };
    let content = match area {
        Some(area) => chrome::render_panel_shell_at(f, area, "REMOTE SESSIONS", footer, true),
        None => {
            chrome::render_panel_shell(f, f.area(), SESSIONS_PANEL_W, "REMOTE SESSIONS", footer)
        }
    };
    let panel_area = area.unwrap_or_else(|| Rect {
        x: f.area().x,
        y: f.area().y + 2,
        width: SESSIONS_PANEL_W.min(f.area().width),
        height: f.area().height.saturating_sub(2),
    });
    let ix = content.x;
    let inner_w = content.width;
    let list_y = content.y;
    let list_h = content.height;
    let list_area = content;

    if sessions_loading && panel_targets.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            list_area,
        );
        return (panel_area, Vec::new());
    }
    if panel_targets.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                " No sessions or cast receivers found",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            list_area,
        );
        return (panel_area, Vec::new());
    }

    const CARD_H: u16 = 3;
    const DIV_H: u16 = 1;
    let entry_h = CARD_H + DIV_H;
    let visible_entries = ((list_h + DIV_H) / entry_h).max(1) as usize;
    *cursor = (*cursor).min(panel_targets.len() - 1);
    if *cursor < *scroll {
        *scroll = *cursor;
    } else if *cursor >= *scroll + visible_entries {
        *scroll = *cursor + 1 - visible_entries;
    }
    *scroll = (*scroll).min(panel_targets.len().saturating_sub(visible_entries));

    let mut row_targets = Vec::new();
    for (i, target) in panel_targets.iter().enumerate().skip(*scroll) {
        let entry_y = list_y + (i - *scroll) as u16 * entry_h;
        if entry_y + CARD_H > list_y + list_h {
            break;
        }

        let selected = i == *cursor;
        let name_color = if selected {
            palette::ACCENT_ACTIVE
        } else {
            palette::TEXT_PRIMARY
        };
        let dim = Style::default().fg(palette::TEXT_MUTED);

        if selected {
            let bar: Vec<Line> = (0..CARD_H)
                .map(|_| Line::from(Span::styled("▌", Style::default().fg(palette::ACCENT))))
                .collect();
            f.render_widget(
                Paragraph::new(bar),
                Rect {
                    x: ix,
                    y: entry_y,
                    width: 1,
                    height: CARD_H,
                },
            );
        }
        let text_x = ix + 2;
        let text_w = inner_w.saturating_sub(2) as usize;

        match target {
            PanelTarget::Emby(s) => {
                let is_connected = connected_session_id == Some(s.id.as_str());
                let badge = if is_connected {
                    if tracking {
                        " ✚ TRACKING"
                    } else {
                        " ✚"
                    }
                } else {
                    ""
                };
                render_kind_labelled_line(
                    f,
                    "EMBY",
                    &s.device_name,
                    badge,
                    name_color,
                    text_x,
                    entry_y,
                    inner_w,
                    text_w,
                );

                let meta = format!("{} · {}@{}", s.client, s.user_name, s.host);
                f.render_widget(
                    Paragraph::new(Span::styled(
                        trunc_str(&meta, text_w),
                        dim.fg(palette::TEXT_SECONDARY),
                    )),
                    Rect {
                        x: text_x,
                        y: entry_y + 1,
                        width: inner_w.saturating_sub(2),
                        height: 1,
                    },
                );

                let state_icon = if s.now_playing.is_some() {
                    if s.is_paused {
                        "⏸"
                    } else {
                        "▶"
                    }
                } else {
                    "■"
                };
                let time = if s.now_playing.is_some() {
                    format!(
                        " {}/{}",
                        fmt_duration_short(s.position_s),
                        fmt_duration_short(s.runtime_s)
                    )
                } else {
                    String::new()
                };
                let title = s.now_playing.as_deref().unwrap_or("idle");
                let playing = format!(
                    "{} {}{}",
                    state_icon,
                    trunc_str(title, text_w.saturating_sub(11)),
                    time
                );
                f.render_widget(
                    Paragraph::new(Span::styled(trunc_str(&playing, text_w), dim)),
                    Rect {
                        x: text_x,
                        y: entry_y + 2,
                        width: inner_w.saturating_sub(2),
                        height: 1,
                    },
                );
            }
            PanelTarget::Cast(r) => {
                let attached = cast_attachment_id == Some(r.id.as_str());
                let badge = if attached { " ✚" } else { "" };
                render_kind_labelled_line(
                    f,
                    "CAST",
                    &r.friendly_name,
                    badge,
                    name_color,
                    text_x,
                    entry_y,
                    inner_w,
                    text_w,
                );

                let meta = format!("{}:{}", r.host, r.port);
                f.render_widget(
                    Paragraph::new(Span::styled(
                        trunc_str(&meta, text_w),
                        dim.fg(palette::TEXT_SECONDARY),
                    )),
                    Rect {
                        x: text_x,
                        y: entry_y + 1,
                        width: inner_w.saturating_sub(2),
                        height: 1,
                    },
                );
            }
        }
    }
    // render_sidebar_scrollbar expects total/scroll in the same row units as
    // content.height, so convert from "entries" to rows (entry_h rows each).
    chrome::render_sidebar_scrollbar(
        f,
        content,
        panel_targets.len() * entry_h as usize,
        *scroll * entry_h as usize,
    );
    row_targets.extend(
        panel_targets
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(visible_entries)
            .filter_map(|(index, _)| {
                let y = list_y + (index - *scroll) as u16 * entry_h;
                (y + CARD_H <= list_y + list_h).then_some((
                    Rect {
                        x: ix,
                        y,
                        width: inner_w,
                        height: CARD_H,
                    },
                    index,
                ))
            }),
    );
    (panel_area, row_targets)
}

/// A target row's first line, shared by both `PanelTarget` kinds: a kind
/// tag ("EMBY"/"CAST") so a device reachable on both channels is visibly
/// two distinct rows (8.2), the target's name, and its trailing
/// connected/attached badge.
#[allow(clippy::too_many_arguments)]
fn render_kind_labelled_line(
    f: &mut Frame,
    kind: &str,
    name: &str,
    badge: &str,
    name_color: ratatui::style::Color,
    text_x: u16,
    entry_y: u16,
    inner_w: u16,
    text_w: usize,
) {
    let kind_tag = format!("[{kind}] ");
    let name_max = text_w
        .saturating_sub(kind_tag.len())
        .saturating_sub(badge.len());
    let name_line = Line::from(vec![
        Span::styled(kind_tag, Style::default().fg(palette::TEXT_MUTED)),
        Span::styled(
            trunc_str(name, name_max),
            Style::default().fg(name_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(badge, Style::default().fg(palette::ACCENT_ACTIVE)),
    ]);
    f.render_widget(
        Paragraph::new(name_line),
        Rect {
            x: text_x,
            y: entry_y,
            width: inner_w.saturating_sub(2),
            height: 1,
        },
    );
}

use super::super::super::palette;
use super::super::super::panel_targets::PanelTarget;
use super::super::super::ui_util::{fmt_duration_short, trunc_str};
use super::super::super::App;
use super::super::super::SESSIONS_PANEL_W;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    /// Renders the F3 panel's merged Emby+Cast target list (8.2): each row
    /// is labelled by the channel that produced it, so a device reachable
    /// on both channels renders as two distinct, separately labelled rows
    /// rather than one merged entry. Content decisions (what counts as a
    /// target, the merge order) live in `panel_targets`/`App`, not here --
    /// this only paints `self.panel_targets` as already resolved.
    pub(in crate::app::render) fn render_sessions_overlay(
        &mut self,
        f: &mut Frame,
        area: Option<Rect>,
    ) {
        let footer = self.sessions_overlay_footer();
        let content = match area {
            Some(area) => Self::render_panel_shell_at(f, area, "REMOTE SESSIONS", footer, true),
            None => {
                Self::render_panel_shell(f, f.area(), SESSIONS_PANEL_W, "REMOTE SESSIONS", footer)
            }
        };
        let ix = content.x;
        let inner_w = content.width;
        let list_y = content.y;
        let list_h = content.height;
        let list_area = content;

        if self.sessions_loading && self.panel_targets.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Loading…",
                    Style::default().fg(palette::TEXT_SECONDARY),
                )),
                list_area,
            );
            return;
        }
        if self.panel_targets.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " No sessions or cast receivers found",
                    Style::default().fg(palette::TEXT_SECONDARY),
                )),
                list_area,
            );
            return;
        }

        const CARD_H: u16 = 3;
        const DIV_H: u16 = 1;
        let entry_h = CARD_H + DIV_H;
        let visible_entries = ((list_h + DIV_H) / entry_h).max(1) as usize;
        self.sessions_cursor = self.sessions_cursor.min(self.panel_targets.len() - 1);
        if self.sessions_cursor < self.sessions_scroll {
            self.sessions_scroll = self.sessions_cursor;
        } else if self.sessions_cursor >= self.sessions_scroll + visible_entries {
            self.sessions_scroll = self.sessions_cursor + 1 - visible_entries;
        }
        self.sessions_scroll = self
            .sessions_scroll
            .min(self.panel_targets.len().saturating_sub(visible_entries));

        for (i, target) in self
            .panel_targets
            .iter()
            .enumerate()
            .skip(self.sessions_scroll)
        {
            let entry_y = list_y + (i - self.sessions_scroll) as u16 * entry_h;
            if entry_y + CARD_H > list_y + list_h {
                break;
            }

            let selected = i == self.sessions_cursor;
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
                    let is_connected = self.connected_session_id.as_deref() == Some(s.id.as_str());
                    let badge = if is_connected {
                        if self.remote_tracker.is_some() {
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
                    let attached = self
                        .cast_attachment
                        .as_ref()
                        .is_some_and(|a| a.receiver_id == r.id);
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
        Self::render_sidebar_scrollbar(
            f,
            content,
            self.panel_targets.len() * entry_h as usize,
            self.sessions_scroll * entry_h as usize,
        );
    }
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

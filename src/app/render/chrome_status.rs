#![allow(unused_imports)]

use super::super::ui_util::*;
use super::chrome::{daemon_endpoint_label, server_url_label};
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
    pub(super) fn remote_status_spans(
        &self,
        remote_state: RemoteSlotState,
        daemon_endpoint: &str,
    ) -> Vec<Span<'static>> {
        let remote_on = matches!(
            remote_state,
            RemoteSlotState::AttachedSession | RemoteSlotState::DirectRemote
        );
        let glyph_style = Style::default()
            .bg(palette::SURFACE_STATUS_PILL)
            .fg(ratatui::style::Color::White);

        let target = match remote_state {
            RemoteSlotState::Off => None,
            RemoteSlotState::AttachedSession => {
                self.connected_session_state.as_ref().and_then(|session| {
                    let device_name = session.device_name.trim();
                    if !device_name.is_empty() {
                        Some(device_name.to_string())
                    } else {
                        let host = session.host.trim();
                        (!host.is_empty()).then(|| host.to_string())
                    }
                })
            }
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
            RemoteSlotState::LocalDaemon => None,
        };
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let label = match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_core::api::device_name()),
        };
        let label_style = Style::default()
            .fg(if remote_on {
                palette::AQUA
            } else {
                ratatui::style::Color::Black
            })
            .bg(palette::SURFACE_STATUS_PILL);

        vec![
            Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f1616}"
                } else {
                    "\u{1F5A7}"
                },
                glyph_style,
            ),
            Span::styled(label, label_style),
            Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
        ]
    }

    /// Returns `(icon, label)` for a remote pill without styling.
    /// Used by queue-title rendering that applies its own colors.
    pub(super) fn remote_icon_and_label(
        &self,
        remote_state: RemoteSlotState,
        daemon_endpoint: &str,
    ) -> (&'static str, String) {
        let icon = if self.use_nerd_fonts {
            "\u{f1616}"
        } else {
            "\u{1F5A7}"
        };
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let target = match remote_state {
            RemoteSlotState::Off => None,
            RemoteSlotState::AttachedSession => {
                self.connected_session_state.as_ref().and_then(|session| {
                    let device_name = session.device_name.trim();
                    if !device_name.is_empty() {
                        Some(device_name.to_string())
                    } else {
                        let host = session.host.trim();
                        (!host.is_empty()).then(|| host.to_string())
                    }
                })
            }
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
            RemoteSlotState::LocalDaemon => None,
        };
        let label = match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_core::api::device_name()),
        };
        (icon, label)
    }

    pub(super) fn playlist_status_spans(&self) -> Vec<Span<'static>> {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let (label, on) = match &self.queue_source {
            crate::config::QueueSource::Playlist { name, .. } => (format!("{gap}{name}"), true),
            _ => (format!("{gap}none"), false),
        };
        let glyph_style = Style::default()
            .bg(palette::SURFACE_STATUS_PILL)
            .fg(ratatui::style::Color::White);
        let label_style = Style::default()
            .fg(if on { palette::YELLOW } else { palette::SUBTLE })
            .bg(palette::SURFACE_STATUS_PILL);

        vec![
            Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f03a}"
                } else {
                    "\u{1F5AD}"
                },
                glyph_style,
            ),
            Span::styled(label, label_style),
            Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
        ]
    }

    pub(super) fn autosave_status_spans(&self) -> Option<Vec<Span<'static>>> {
        let autosave_on = self.queue_is_saved_playlist() && {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
        };
        if self.queue_dirty {
            Some(vec![
                Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
                Span::styled(
                    " UNSAVED ",
                    Style::default()
                        .fg(palette::YELLOW)
                        .bg(palette::SURFACE_STATUS_PILL)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
            ])
        } else if autosave_on {
            Some(vec![
                Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
                Span::styled(
                    " AUTOSAVE ",
                    Style::default()
                        .fg(palette::AQUA)
                        .bg(palette::SURFACE_STATUS_PILL),
                ),
                Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
            ])
        } else {
            None
        }
    }

    pub(super) fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
        self.playback_display_target()
            .displayed_mute(self)
            .then(|| {
                vec![
                    Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
                    Span::styled(
                        "muted",
                        Style::default()
                            .fg(palette::RED)
                            .bg(palette::SURFACE_STATUS_PILL)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
                ]
            })
    }

    pub(super) fn status_width(spans: &[Span]) -> u16 {
        spans.iter().map(|s| s.content.width() as u16).sum()
    }

    pub(super) fn append_status(spans: &mut Vec<Span<'static>>, status: Vec<Span<'static>>) {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.extend(status);
    }

    pub(super) fn set_status_label_color(spans: &mut [Span<'static>], color: Color) {
        if let Some(label) = spans.get_mut(2) {
            label.style = label.style.fg(color);
        }
    }

    pub(super) fn set_status_pill_style(spans: &mut [Span<'static>], fg: Color, bg: Color) {
        for span in spans.iter_mut() {
            span.style = span.style.bg(bg);
        }
        Self::set_status_label_color(spans, fg);
    }

    /// Uppercase the status label span (index 2, same convention as
    /// [`Self::set_status_label_color`]) in place.
    pub(super) fn uppercase_status_label(spans: &mut [Span<'static>]) {
        let Some(label) = spans.get_mut(2) else {
            return;
        };
        label.content = label.content.to_uppercase().into();
    }

    pub(super) fn render_remote_status_hitbox(
        &self,
        layout: &mut LayoutPlayback,
        area: Rect,
        remote_x: Option<u16>,
        remote_w: u16,
    ) {
        if area.width == 0 {
            layout.ind_rc = Rect::default();
        } else if let Some(x) = remote_x {
            layout.ind_rc = Rect {
                x,
                y: area.y,
                width: remote_w,
                height: 1,
            };
        } else {
            layout.ind_rc = Rect::default();
        }
    }

    /// Persistent bottom status bar. Left side: connection, stay-alive,
    /// shared-state, and mute status groups. Right side: queue
    /// source/save-state/scope detail.
    /// The playlist status pill renders in the left queue panel instead.
    pub(super) fn render_status_bar(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPlayback,
        show_session_pill: bool,
    ) {
        // Keep the row itself darker so the pills read as segments sitting on top of it.
        let bar_style = Style::default().bg(palette::SURFACE_CHROME);
        f.render_widget(Block::default().style(bar_style), area);
        layout.ind_mu = Rect::default();

        let remote_state = self.remote_slot_state();
        let (daemon_endpoint, server_url, username) = {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            (
                cfg.daemon_client_endpoint.clone(),
                cfg.server_url.clone(),
                cfg.username.clone(),
            )
        };
        let remote_status = if show_session_pill {
            self.remote_status_spans(remote_state, &daemon_endpoint)
        } else {
            Vec::new()
        };
        let alive_status: Option<Vec<Span>> = self.is_local_daemon().then(|| {
            vec![
                Span::raw(" "),
                Span::styled(
                    if self.use_nerd_fonts {
                        "\u{f004}"
                    } else {
                        "\u{2665}"
                    },
                    Style::default().fg(palette::RED),
                ),
            ]
        });
        let shared_status = self.shared_client.as_ref().is_some_and(|client| {
            matches!(
                client.state(),
                mbv_core::shared_client::SharedClientState::Shared
            )
        });
        let client_status = match (alive_status, shared_status) {
            (None, false) => None,
            (alive, shared) => {
                let mut spans = alive.unwrap_or_default();
                if shared {
                    spans.extend([
                        Span::raw(" "),
                        Span::styled("", Style::default().fg(palette::FOAM)),
                    ]);
                }
                Some(spans)
            }
        };
        let mute_status = self.mute_status_spans();

        // Preserve the existing left-segment overflow order: client indicators
        // drop first, then mute, then remote.
        let remote_w = Self::status_width(&remote_status);
        let client_w: u16 = client_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let mute_w: u16 = mute_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let available = area.width;
        let joined_width = |widths: &[u16]| -> u16 {
            let mut total = 0u16;
            for (count, width) in widths.iter().copied().filter(|w| *w > 0).enumerate() {
                total = total.saturating_add(width);
                if count > 0 {
                    total = total.saturating_add(1);
                }
            }
            total
        };
        let fits_all = joined_width(&[remote_w, client_w, mute_w]) <= available;
        let fits_without_client = !fits_all && joined_width(&[remote_w, mute_w]) <= available;
        let fits_without_mute =
            !fits_all && !fits_without_client && joined_width(&[remote_w, client_w]) <= available;
        let fits_without_remote =
            !fits_all && !fits_without_client && !fits_without_mute && client_w <= available;

        let show_remote = remote_w > 0 && (fits_all || fits_without_client || fits_without_mute);
        let show_client =
            client_status.is_some() && (fits_all || fits_without_mute || fits_without_remote);

        let mut spans: Vec<Span> = Vec::new();
        if show_client {
            if let Some(client) = client_status.as_ref() {
                spans.extend(client.iter().cloned());
            }
        }
        let remote_x =
            show_remote.then(|| area.x + Self::status_width(&spans) + u16::from(!spans.is_empty()));
        if show_remote {
            Self::append_status(&mut spans, remote_status);
        }
        self.render_remote_status_hitbox(layout, area, remote_x, remote_w);
        if fits_all || fits_without_client {
            if let Some(mute) = mute_status {
                let mute_x = area.x + Self::status_width(&spans);
                let mute_w = Self::status_width(&mute);
                Self::append_status(&mut spans, mute);
                layout.ind_mu = Rect {
                    x: mute_x,
                    y: area.y,
                    width: mute_w,
                    height: 1,
                };
            }
        }

        // `left_content_w` tracks how far the left segment actually extends after
        // the above priority drop, so the right-segment overlap check can compare
        // against the real left edge instead of a hardcoded constant.
        let label_w: u16 = spans.iter().map(|s| s.content.width() as u16).sum();
        let left_content_w: u16 = label_w;
        if !spans.is_empty() {
            let label_rect = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(spans)).style(bar_style),
                label_rect,
            );
        }

        {
            let mut right_spans: Vec<Span> = Vec::new();
            let source_label: Option<(String, Color)> = match &self.queue_source {
                crate::config::QueueSource::Playlist { .. } => None,
                crate::config::QueueSource::Album
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("ALBUM".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Series
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("SERIES".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Shuffle
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("SHUFFLE".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Remote
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("REMOTE Q".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Collection { collection_type }
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some((collection_type.to_uppercase(), palette::MUTED))
                }
                crate::config::QueueSource::Unknown => None,
                _ => None,
            };
            let append_right = |right_spans: &mut Vec<Span<'static>>, span: Span<'static>| {
                if !right_spans.is_empty() {
                    right_spans.push(Span::raw(" "));
                }
                right_spans.push(span);
            };
            if let Some((label, color)) = source_label {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        format!(" {label} "),
                        Style::default().fg(color).bg(palette::SURFACE_STATUS_PILL),
                    ),
                );
            }
            if !username.is_empty() {
                if !right_spans.is_empty() {
                    right_spans.push(Span::raw(" "));
                }
                right_spans.push(Span::styled(
                    " 🯅",
                    Style::default()
                        .fg(palette::FOAM)
                        .bg(palette::SURFACE_STATUS_PILL),
                ));
                right_spans.push(Span::styled(
                    format!(" {username} "),
                    Style::default()
                        .fg(palette::PLAYBACK_META_FG)
                        .bg(palette::SURFACE_STATUS_PILL),
                ));
            }
            if let Some(server) = server_url_label(&server_url) {
                if self.use_nerd_fonts {
                    if !right_spans.is_empty() {
                        right_spans.push(Span::raw(" "));
                    }
                    right_spans.push(Span::styled(
                        " \u{F06B4}",
                        Style::default()
                            .fg(palette::AQUA)
                            .bg(palette::SURFACE_STATUS_PILL),
                    ));
                    right_spans.push(Span::styled(
                        format!(" {server} "),
                        Style::default()
                            .fg(palette::SUBTLE)
                            .bg(palette::SURFACE_STATUS_PILL),
                    ));
                } else {
                    append_right(
                        &mut right_spans,
                        Span::styled(
                            format!(" {server} "),
                            Style::default()
                                .fg(palette::SUBTLE)
                                .bg(palette::SURFACE_STATUS_PILL),
                        ),
                    );
                }
            }
            // Remote queue scope is omitted here: the active queue is already
            // apparent from the queue UI.
            if !right_spans.is_empty() {
                let right_w: u16 = right_spans.iter().map(|s| s.content.width() as u16).sum();
                // Compare against `left_content_w` (pill + session label, from Task 2),
                // not a hardcoded pill-only width -- otherwise this check passes while
                // the right segment still overlaps a rendered session label (e.g.
                // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
                let left_end = area.x + left_content_w;
                let right_x = area.x + area.width.saturating_sub(right_w);
                if right_x > left_end {
                    let right_rect = Rect {
                        x: right_x,
                        y: area.y,
                        width: right_w,
                        height: 1,
                    };
                    f.render_widget(
                        Paragraph::new(Line::from(right_spans)).style(bar_style),
                        right_rect,
                    );
                }
                // else: terminal too narrow for both segments -- right segment drops
                // silently rather than overlapping the pill or the session label.
                // (Design doc's open question on narrow-terminal truncation: right
                // segment yields first.)
            }
        }
    }
}

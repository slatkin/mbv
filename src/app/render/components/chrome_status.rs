//! Status-bar span builders (task 2.2).
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`src/app/components/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. The `impl App`
//! methods here are content production only: they read app state and build
//! the pill/right-segment spans the shell projects into the component.
//!
//! The Local/Remote queue-scope pills are queue concern and paint in the
//! `QueueColumn` footer (`render_queue_status`), never here.

use super::chrome::daemon_endpoint_label;
use super::indicators;
use crate::app::ui_util::service_state_color;
use crate::app::{palette, App, PanelFocus, RemoteSlotState};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

impl App {
    /// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
    /// Returns None if the local player is not active.
    /// Callers wrap these in [ ... ] with whatever surrounding style they need.
    pub(in crate::app) fn build_status_indicator_spans(&self) -> Option<Vec<Span<'static>>> {
        let data = self.playback_indicator_target().indicator_data(self)?;
        Some(indicators::indicator_spans(
            self.indicator_style,
            &data,
            self.use_nerd_fonts,
        ))
    }

    pub(in crate::app) fn remote_status_spans(
        &self,
        remote_state: RemoteSlotState,
        daemon_endpoint: &str,
    ) -> Vec<Span<'static>> {
        let remote_on = matches!(
            remote_state,
            RemoteSlotState::AttachedSession | RemoteSlotState::DirectRemote
        );
        let glyph_style = Style::default()
            .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill)
            .fg(ratatui::style::Color::White);

        let target = match remote_state {
            RemoteSlotState::Off | RemoteSlotState::LocalDaemon => None,
            RemoteSlotState::AttachedSession => {
                self.connected_session_state.as_ref().and_then(|session| {
                    let device_name = session.device_name.trim();
                    if device_name.is_empty() {
                        let host = session.host.trim();
                        (!host.is_empty()).then(|| host.to_string())
                    } else {
                        Some(device_name.to_string())
                    }
                })
            }
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.remote.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
        };
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let label = match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_core::api::device_name()),
        };
        let label_style = Style::default()
            .fg(if remote_on {
                palette::ACCENT
            } else {
                ratatui::style::Color::Black
            })
            .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill);

        vec![
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f1616}"
                } else {
                    "\u{1F5A7}"
                },
                glyph_style,
            ),
            Span::styled(label, label_style),
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
        ]
    }

    /// Returns `(icon, label)` for a remote pill without styling.
    /// Used by queue-title rendering that applies its own colors.
    pub(in crate::app) fn remote_icon_and_label(
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
            RemoteSlotState::Off | RemoteSlotState::LocalDaemon => None,
            RemoteSlotState::AttachedSession => {
                self.connected_session_state.as_ref().and_then(|session| {
                    let device_name = session.device_name.trim();
                    if device_name.is_empty() {
                        let host = session.host.trim();
                        (!host.is_empty()).then(|| host.to_string())
                    } else {
                        Some(device_name.to_string())
                    }
                })
            }
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.remote.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
        };
        let label = match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_core::api::device_name()),
        };
        (icon, label)
    }

    /// The playback target's host label plus whether it names a remote target
    /// (a cast attachment, an attached session, or a direct-remote
    /// route/label): the connected session's device name (falling back to
    /// its host), the direct-remote route/label, or this machine's device
    /// name when playback is local — computed together from one
    /// `remote_slot_state()` resolution for callers (the queue header sync)
    /// that need both every tick. Callers style the returned label and the
    /// header row shows it verbatim.
    pub(in crate::app) fn playback_host_label_and_remote(&self) -> (String, bool) {
        let remote_state = self.remote_slot_state();
        let is_remote = self.cast_attachment.is_some()
            || matches!(
                remote_state,
                RemoteSlotState::AttachedSession | RemoteSlotState::DirectRemote
            );
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let (_, label) = self.remote_icon_and_label(remote_state, &daemon_endpoint);
        (label.trim_start().to_string(), is_remote)
    }

    pub(in crate::app) fn playlist_status_spans(&self) -> Vec<Span<'static>> {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let (label, on) = match &self.queue_source {
            crate::config::QueueSource::Playlist { name, .. } => (format!("{gap}{name}"), true),
            _ => (format!("{gap}none"), false),
        };
        let glyph_style = Style::default()
            .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill)
            .fg(ratatui::style::Color::White);
        let label_style = Style::default()
            .fg(if on {
                palette::TEXT_FOCUS_ACCENT
            } else {
                palette::TEXT_SECONDARY
            })
            .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill);

        vec![
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f03a}"
                } else {
                    "\u{1F5AD}"
                },
                glyph_style,
            ),
            Span::styled(label, label_style),
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
        ]
    }

    pub(in crate::app) fn autosave_status_spans(&self) -> Option<Vec<Span<'static>>> {
        let autosave_on = self.queue_is_saved_playlist() && {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
        };
        if self.queue_dirty {
            Some(vec![
                Span::styled(
                    " ",
                    Style::default().bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
                Span::styled(
                    " UNSAVED ",
                    Style::default()
                        .fg(palette::TEXT_FOCUS_ACCENT)
                        .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " ",
                    Style::default().bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
            ])
        } else if autosave_on {
            Some(vec![
                Span::styled(
                    " ",
                    Style::default().bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
                Span::styled(
                    " AUTOSAVE ",
                    Style::default()
                        .fg(palette::ACCENT)
                        .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
                ),
                Span::styled(
                    " ",
                    Style::default().bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
            ])
        } else {
            None
        }
    }

    pub(in crate::app) fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
        self.playback_display_target()
            .displayed_mute(self)
            .then(|| {
                vec![
                    Span::styled(
                        " ",
                        Style::default().bg(palette::surface_colors(
                            palette::Surface::StatusBarPill,
                            false,
                        )
                        .fill),
                    ),
                    Span::styled(
                        "muted",
                        Style::default()
                            .fg(palette::STATUS_ERROR)
                            .bg(
                                palette::surface_colors(palette::Surface::StatusBarPill, false)
                                    .fill,
                            )
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " ",
                        Style::default().bg(palette::surface_colors(
                            palette::Surface::StatusBarPill,
                            false,
                        )
                        .fill),
                    ),
                ]
            })
    }

    /// The prefix-armed pill (design D7, task 7.3): present only while
    /// prefix mode is armed, styled like the other left-segment pills.
    /// Non-interactive: any mouse event already disarms prefix mode
    /// silently, so the pill carries no hit region.
    pub(in crate::app) fn prefix_armed_status_spans(&self) -> Option<Vec<Span<'static>>> {
        self.prefix_armed.then(|| {
            vec![
                Span::styled(
                    " ",
                    Style::default().bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
                Span::styled(
                    " PREFIX ",
                    Style::default()
                        .fg(palette::ACCENT)
                        .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " ",
                    Style::default().bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
            ]
        })
    }

    pub(in crate::app) fn volume_status_spans(&self) -> Vec<Span<'static>> {
        let volume = self.playback_display_target().displayed_volume(self);
        // Speaker glyph reflects the volume state (0 / low / mid / high).
        let icon = if volume == 0 {
            "\u{1F507}"
        } else if volume <= 25 {
            "\u{1F508}"
        } else if volume <= 75 {
            "\u{1F509}"
        } else {
            "\u{1F50A}"
        };
        vec![
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
            Span::styled(
                icon,
                Style::default()
                    .fg(palette::PLAYBACK_META_FG)
                    .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
            ),
            Span::styled(
                format!(" {volume}"),
                Style::default()
                    .fg(palette::ACCENT)
                    .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    }

    /// The status row's right segment: queue-source scope label, username,
    /// and the service-state glyphs (Emby, Audiobookshelf, stay-alive) —
    /// always visible, coloured by state. Built shell-side
    /// (task 2.2); the mounted `StatusBarPanel` positions and paints it.
    pub(in crate::app) fn status_bar_right_spans(&self) -> Vec<Span<'static>> {
        let (username, stay_alive) = {
            let config = self.config.lock().unwrap();
            (config.username.clone(), config.stay_alive)
        };
        // Stay-alive indicator: red whenever this process runs in stay-alive
        // mode (the daemon persists after quit), yellow when that daemon is
        // lost, grey when stay-alive is off. Deliberately independent of the
        // current player target: a stay-alive client that routes playback to
        // another daemon is still in stay-alive mode.
        let alive_color = if !stay_alive {
            palette::TEXT_MUTED
        } else if self.player.is_remote_disconnected() {
            palette::TEXT_FOCUS_ACCENT
        } else {
            palette::STATUS_ERROR
        };
        let mut right_spans: Vec<Span> = Vec::new();
        let source_label = queue_source_status_label(
            &self.queue_source,
            matches!(self.effective_panel_focus(), PanelFocus::Queue),
        );
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
                    Style::default().fg(color).bg(palette::surface_colors(
                        palette::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
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
                    .fg(palette::TEXT_METADATA)
                    .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
            ));
            right_spans.push(Span::styled(
                format!(" {username} "),
                Style::default()
                    .fg(palette::PLAYBACK_META_FG)
                    .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
            ));
        }
        // Service-state glyphs — Emby coloured by state (brand colour
        // when active, grey when inactive), Audiobookshelf always
        // yellow, stay-alive daemon lost = yellow. One leading space
        // per glyph, no trailing space.
        right_spans.extend([
            Span::raw(" "),
            Span::styled(
                "\u{F06B4}",
                Style::default().fg(service_state_color(
                    self.emby_runtime.state,
                    palette::ACCENT,
                )),
            ),
            Span::raw(" "),
            Span::styled(
                "\u{EDE2}",
                Style::default().fg(palette::ACCENT_AUDIOBOOKSHELF),
            ),
            Span::raw(" "),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f004}"
                } else {
                    "\u{2665}"
                },
                Style::default().fg(alive_color),
            ),
            Span::raw(" "),
        ]);
        // Remote queue scope is omitted here: the active queue is already
        // apparent from the queue UI.
        right_spans
    }

    pub(in crate::app) fn set_status_label_color(spans: &mut [Span<'static>], color: Color) {
        if let Some(label) = spans.get_mut(2) {
            label.style = label.style.fg(color);
        }
    }

    pub(in crate::app) fn set_status_pill_style(spans: &mut [Span<'static>], fg: Color, bg: Color) {
        for span in spans.iter_mut() {
            span.style = span.style.bg(bg);
        }
        Self::set_status_label_color(spans, fg);
    }

    /// Uppercase the status label span (index 2, same convention as
    /// [`Self::set_status_label_color`]) in place.
    pub(in crate::app) fn uppercase_status_label(spans: &mut [Span<'static>]) {
        let Some(label) = spans.get_mut(2) else {
            return;
        };
        label.content = label.content.to_uppercase().into();
    }
}

fn queue_source_status_label(
    source: &crate::config::QueueSource,
    queue_focused: bool,
) -> Option<(String, Color)> {
    if !queue_focused {
        return None;
    }
    let label = match source {
        crate::config::QueueSource::Album => "ALBUM".to_string(),
        crate::config::QueueSource::Series => "SERIES".to_string(),
        crate::config::QueueSource::Shuffle => "SHUFFLE".to_string(),
        crate::config::QueueSource::Remote => "REMOTE Q".to_string(),
        crate::config::QueueSource::Collection { collection_type } => {
            collection_type.to_uppercase()
        }
        crate::config::QueueSource::Playlist { .. } | crate::config::QueueSource::Unknown => {
            return None;
        }
    };
    Some((label, palette::TEXT_MUTED))
}

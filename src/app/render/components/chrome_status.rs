//! Status-bar span builders and painter (task 2.2).
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`src/app/components/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. The `impl App`
//! methods here are content production only: they read app state and build
//! the pill/right-segment spans the shell projects into the component. The
//! free [`render_status_bar`] is the component's painter.

use super::chrome::{daemon_endpoint_label, service_state_color};
use super::indicators;
use super::queue::QueueTitleModel;
use crate::app::{palette, App, PanelFocus, RemoteSlotState};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

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
        let username = {
            let config = self.config.lock().unwrap();
            config.username.clone()
        };
        let alive_color = if self.dim_backdrop_active {
            palette::TEXT_FOCUS_ACCENT
        } else if self.is_local_daemon() {
            palette::STATUS_ERROR
        } else {
            palette::TEXT_MUTED
        };
        let mut right_spans: Vec<Span> = Vec::new();
        let source_label: Option<(String, Color)> = match &self.queue_source {
            crate::config::QueueSource::Playlist { .. } => None,
            crate::config::QueueSource::Album
                if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
            {
                Some(("ALBUM".to_string(), palette::TEXT_MUTED))
            }
            crate::config::QueueSource::Series
                if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
            {
                Some(("SERIES".to_string(), palette::TEXT_MUTED))
            }
            crate::config::QueueSource::Shuffle
                if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
            {
                Some(("SHUFFLE".to_string(), palette::TEXT_MUTED))
            }
            crate::config::QueueSource::Remote
                if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
            {
                Some(("REMOTE Q".to_string(), palette::TEXT_MUTED))
            }
            crate::config::QueueSource::Collection { collection_type }
                if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
            {
                Some((collection_type.to_uppercase(), palette::TEXT_MUTED))
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
        // Service-state glyphs — Emby, Audiobookshelf, stay-alive — always
        // visible, coloured by state (brand colour when active,
        // grey when inactive; stay-alive daemon lost = yellow). One
        // leading space per glyph, no trailing space.
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
                Style::default().fg(service_state_color(
                    self.audiobookshelf_runtime.state,
                    palette::ACCENT_AUDIOBOOKSHELF,
                )),
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

    pub(in crate::app) fn status_width(spans: &[Span]) -> u16 {
        spans.iter().map(|s| s.content.width() as u16).sum()
    }

    pub(in crate::app) fn append_status(
        spans: &mut Vec<Span<'static>>,
        status: Vec<Span<'static>>,
    ) {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.extend(status);
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

/// Plain-data paint model for one status row. The shell projects spans;
/// `StatusBarPanel` owns overflow, hit regions and pointer resolution.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct VisualModeIndicator {
    pub count: usize,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct StatusBarModel {
    /// Whether the remote/session pill participates (the base frame has
    /// always passed `false` here — the queue-scope pills below show the
    /// same info; the parameter is retained verbatim).
    pub show_session_pill: bool,
    /// Remote/session pill spans (empty unless `show_session_pill`).
    pub remote: Vec<Span<'static>>,
    /// Mute pill spans (absent when not muted).
    pub mute: Option<Vec<Span<'static>>>,
    /// Volume pill spans.
    pub volume: Vec<Span<'static>>,
    /// Fully built right segment (scope label, username, service glyphs).
    pub right: Vec<Span<'static>>,
    /// Queue-scope pills (moved from the queue column's removed title band):
    /// `Some` only while connected to an mbv-based session (`show_split`
    /// and `is_mbv_session`). The painter keeps only those two flags plus
    /// `local_selected` and `remote_icon`.
    pub queue_scope: Option<QueueTitleModel>,
    pub visual_mode: Option<VisualModeIndicator>,
}

/// The status row's pointer regions, retained by the mounted
/// `StatusBarPanel` after painting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct StatusBarRegions {
    /// Volume pill: scroll-wheel adjusts the volume.
    pub volume: Option<Rect>,
    /// Mute pill: click toggles mute.
    pub mute: Option<Rect>,
    /// Remote/session pill region, when the session pill is enabled.
    pub remote: Option<Rect>,
    /// Queue-scope Local pill, when the scope pills are shown.
    pub scope_local: Option<Rect>,
    /// Queue-scope Remote pill, when the scope pills are shown.
    pub scope_remote: Option<Rect>,
    pub visual_clear: Option<Rect>,
}

/// Paint the one-row status bar within `area` (the `RootFrame.status_bar`
/// placement) and return the painted pill regions.
///
/// Persistent bottom status bar. Left side: volume, connection,
/// and mute status groups. Right side: queue source/save-state/scope
/// detail and the service-state glyphs (Emby, Audiobookshelf,
/// stay-alive), with the queue-scope pills (Local/Remote,
/// while connected to an mbv-based session) at the far right.
/// The playlist status pill renders in the left queue panel instead.
pub(in crate::app) fn render_status_bar(
    f: &mut Frame,
    area: Rect,
    model: &StatusBarModel,
) -> StatusBarRegions {
    let mut regions = StatusBarRegions::default();
    // Keep the row itself darker so the pills read as segments sitting on top of it.
    let bar_style =
        Style::default().bg(palette::surface_colors(palette::Surface::StatusBar, false).fill);
    // `Clear` blanks every cell's symbol first (task 12.2): a bare
    // `Block::style` only recolors a cell, it never overwrites a stale
    // glyph left by whatever painted this placement before the status bar
    // owned it.
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(bar_style), area);

    let mute_status = model.mute.clone();
    let vol_status = model.volume.clone();
    let remote_status = if model.show_session_pill {
        model.remote.clone()
    } else {
        Vec::new()
    };

    // Preserve the existing left-segment overflow order: mute drops
    // first, then the volume pill, then remote. (The service-state
    // glyphs now live in the right segment.)
    let remote_w = App::status_width(&remote_status);
    let visual_status = model.visual_mode.as_ref().map(|indicator| {
        vec![Span::styled(
            format!("-- VISUAL ({}) --", indicator.count),
            Style::default()
                .fg(palette::TEXT_FOCUS_ACCENT)
                .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
        )]
    });
    let visual_w = visual_status
        .as_ref()
        .map(|spans| App::status_width(spans))
        .unwrap_or(0);
    let mute_w: u16 = mute_status
        .as_ref()
        .map(|spans| App::status_width(spans))
        .unwrap_or(0);
    let vol_w = App::status_width(&vol_status);
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
    let fits_all = joined_width(&[visual_w, remote_w, mute_w, vol_w]) <= available;
    let fits_without_mute = !fits_all && joined_width(&[visual_w, remote_w, vol_w]) <= available;
    let fits_without_volume =
        !fits_all && !fits_without_mute && joined_width(&[visual_w, remote_w, mute_w]) <= available;
    let fits_without_remote = !fits_all
        && !fits_without_mute
        && !fits_without_volume
        && joined_width(&[visual_w, mute_w, vol_w]) <= available;

    let show_visual = visual_w > 0
        && (fits_all || fits_without_mute || fits_without_volume || fits_without_remote);
    let show_remote = remote_w > 0 && (fits_all || fits_without_mute || fits_without_volume);
    let show_volume = fits_all || fits_without_mute || fits_without_remote;

    let mut spans: Vec<Span> = Vec::new();
    if show_visual {
        let visual_x = area.x + App::status_width(&spans);
        App::append_status(&mut spans, visual_status.unwrap_or_default());
        regions.visual_clear = Some(Rect {
            x: visual_x,
            y: area.y,
            width: visual_w,
            height: 1,
        });
    }
    if show_volume {
        let vol_x = area.x + App::status_width(&spans);
        App::append_status(&mut spans, vol_status);
        regions.volume = Some(Rect {
            x: vol_x,
            y: area.y,
            width: vol_w,
            height: 1,
        });
    }
    let remote_x =
        show_remote.then(|| area.x + App::status_width(&spans) + u16::from(!spans.is_empty()));
    if show_remote {
        App::append_status(&mut spans, remote_status);
        regions.remote = remote_x.map(|x| Rect {
            x,
            y: area.y,
            width: remote_w,
            height: 1,
        });
    }
    if fits_all || fits_without_mute {
        if let Some(mute) = mute_status {
            let mute_x = area.x + App::status_width(&spans);
            let mute_w = App::status_width(&mute);
            App::append_status(&mut spans, mute);
            regions.mute = Some(Rect {
                x: mute_x,
                y: area.y,
                width: mute_w,
                height: 1,
            });
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

    let right_spans = &model.right;
    // Queue-scope pills (moved from the queue column's removed title band):
    // at the far right while connected to an mbv-based session. They win
    // over the right segment on narrow terminals — scope switching stays
    // reachable while the passive service glyphs yield.
    let scope = model
        .queue_scope
        .as_ref()
        .filter(|m| m.show_split && m.is_mbv_session);
    let (scope_spans, scope_local_w, scope_remote_w) = match scope {
        Some(m) => {
            let selected_bg =
                palette::surface_colors(palette::Surface::QueueScopePillSelected, false).fill;
            let chip_bg = palette::surface_colors(palette::Surface::PillChip, false).fill;
            let (local_bg, local_fg, remote_bg, remote_fg) = if m.local_selected {
                (
                    selected_bg,
                    palette::TEXT_FOCUS_ACCENT,
                    chip_bg,
                    palette::PILL_FG,
                )
            } else {
                (
                    chip_bg,
                    palette::PILL_FG,
                    selected_bg,
                    palette::TEXT_FOCUS_ACCENT,
                )
            };
            let local_span = Span::styled(" \u{2302} ", Style::default().fg(local_fg).bg(local_bg));
            let remote_span = Span::styled(
                format!(" {} ", m.remote_icon),
                Style::default().fg(remote_fg).bg(remote_bg),
            );
            let local_w = local_span.content.width() as u16;
            let remote_w = remote_span.content.width() as u16;
            (vec![local_span, remote_span], local_w, remote_w)
        }
        None => (Vec::new(), 0, 0),
    };
    let scope_w = scope_local_w + scope_remote_w;
    if !right_spans.is_empty() || scope_w > 0 {
        let right_w: u16 = right_spans.iter().map(|s| s.content.width() as u16).sum();
        // Compare against `left_content_w` (pill + session label, from Task 2),
        // not a hardcoded pill-only width -- otherwise this check passes while
        // the right segment still overlaps a rendered session label (e.g.
        // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
        let left_end = area.x + left_content_w;
        // The scope pills sit at the far right; the right segment yields
        // first when the two no longer fit alongside the left segment.
        let scope_x = area.x + area.width.saturating_sub(scope_w);
        let show_scope = scope_w > 0 && scope_x > left_end;
        let right_end = if show_scope {
            scope_x
        } else {
            scope_x + scope_w
        };
        let right_x = right_end.saturating_sub(right_w);
        let show_right = right_w > 0 && right_x > left_end;
        if show_scope {
            regions.scope_local = Some(Rect {
                x: scope_x,
                y: area.y,
                width: scope_local_w,
                height: 1,
            });
            regions.scope_remote = Some(Rect {
                x: scope_x + scope_local_w,
                y: area.y,
                width: scope_remote_w,
                height: 1,
            });
            f.render_widget(
                Paragraph::new(Line::from(scope_spans)).style(bar_style),
                Rect {
                    x: scope_x,
                    y: area.y,
                    width: scope_w,
                    height: 1,
                },
            );
        }
        if show_right {
            let right_rect = Rect {
                x: right_x,
                y: area.y,
                width: right_w,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(right_spans.clone())).style(bar_style),
                right_rect,
            );
        }
        // else: terminal too narrow for both segments -- right segment drops
        // silently rather than overlapping the pill or the session label.
        // (Design doc's open question on narrow-terminal truncation: right
        // segment yields first.)
    }
    regions
}

#[cfg(test)]
#[path = "chrome_status_tests.rs"]
mod chrome_status_tests;

//! Status-bar span builders and painter (task 2.2).
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`src/app/components/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. The `impl App`
//! methods here are content production only: they read app state and build
//! the pill/right-segment spans the shell projects into the component. The
//! free [`render_status_bar`] is the component's painter.
//!
//! The Local/Remote queue-scope pills are queue concern and paint in the
//! `QueueColumn` footer (`render_queue_status`), never here.

use super::chrome::daemon_endpoint_label;
use super::indicators;
use crate::app::ui_util::service_state_color;
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
                .or_else(|| self.direct_remote_label.clone())
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
                .or_else(|| self.direct_remote_label.clone())
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

    pub(in crate::app) fn status_width(spans: &[Span]) -> u16 {
        spans_width(spans)
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

/// Plain-data indicator for active Visual mode.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct VisualModeIndicator {
    /// Number of selected items.
    pub count: usize,
}
/// Plain-data paint model for one status row. The shell projects spans; the
/// mounted `StatusBarPanel` owns overflow, hit regions and Visual-mode clearing.
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
    /// Visual-mode count indicator, when selected items exist.
    pub visual_mode: Option<VisualModeIndicator>,
    /// Prefix-armed pill spans (absent when not armed).
    pub prefix_armed: Option<Vec<Span<'static>>>,
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
    /// Visual-mode region; clicking it clears selection.
    pub visual_clear: Option<Rect>,
}

/// Paint the one-row status bar within `area` (the `RootFrame.status_bar`
/// placement) and return the painted pill regions.
///
/// Persistent bottom status bar. Left side: volume, connection,
/// and mute status groups. Right side: queue source/save-state/scope
/// detail and the service-state glyphs (Emby, Audiobookshelf,
/// stay-alive). The playlist status pill renders in the left queue panel
/// instead; the Local/Remote queue-scope pills paint in the `QueueColumn`
/// footer (`render_queue_status`), never here.
struct StatusBarLeftSegments {
    mute: Option<Vec<Span<'static>>>,
    volume: Vec<Span<'static>>,
    remote: Vec<Span<'static>>,
    armed: Option<Vec<Span<'static>>>,
    visual: Option<Vec<Span<'static>>>,
    volume_width: u16,
    remote_width: u16,
    visual_width: u16,
    fits: StatusBarFit,
    visibility: StatusBarVisibility,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent per-segment visibility results over distinct status-bar fit drop tiers; all four are simultaneously true at StatusBarFit::All (design analysis, issue #804)"
)]
#[derive(Clone, Copy)]
struct StatusBarVisibility {
    visual: bool,
    armed: bool,
    remote: bool,
    volume: bool,
}

struct StatusBarWidths {
    visual: u16,
    armed: u16,
    remote: u16,
    mute: u16,
    volume: u16,
}

#[derive(Clone, Copy)]
enum StatusBarFit {
    All,
    WithoutMute,
    WithoutVolume,
    WithoutRemote,
    None,
}

fn status_bar_fit(widths: &StatusBarWidths, available: u16) -> StatusBarFit {
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
    let all = joined_width(&[
        widths.visual,
        widths.armed,
        widths.remote,
        widths.mute,
        widths.volume,
    ]) <= available;
    let without_mute = !all
        && joined_width(&[widths.visual, widths.armed, widths.remote, widths.volume]) <= available;
    let without_volume = !all
        && !without_mute
        && joined_width(&[widths.visual, widths.armed, widths.remote, widths.mute]) <= available;
    if all {
        StatusBarFit::All
    } else if without_mute {
        StatusBarFit::WithoutMute
    } else if without_volume {
        StatusBarFit::WithoutVolume
    } else if joined_width(&[widths.visual, widths.armed, widths.mute, widths.volume]) <= available
    {
        StatusBarFit::WithoutRemote
    } else {
        StatusBarFit::None
    }
}

fn status_bar_left_segments(model: &StatusBarModel, available: u16) -> StatusBarLeftSegments {
    let mute = model.mute.clone();
    let volume = model.volume.clone();
    let remote = if model.show_session_pill {
        model.remote.clone()
    } else {
        Vec::new()
    };
    let armed = model.prefix_armed.clone();

    // Preserve the existing left-segment overflow order: mute drops
    // first, then the volume pill, then remote. The armed pill sits with
    // the visual-mode indicator at the top persistence tier: it drops only
    // when nothing else is left (it names the active routing mode).
    let remote_width = App::status_width(&remote);
    let armed_width = armed.as_ref().map_or(0, |spans| App::status_width(spans));
    let visual = model.visual_mode.as_ref().map(|indicator| {
        vec![Span::styled(
            format!("-- VISUAL ({}) --", indicator.count),
            Style::default()
                .fg(palette::TEXT_FOCUS_ACCENT)
                .bg(palette::surface_colors(palette::Surface::StatusBarPill, false).fill),
        )]
    });
    let visual_width = visual.as_ref().map_or(0, |spans| App::status_width(spans));
    let mute_width: u16 = mute.as_ref().map_or(0, |spans| App::status_width(spans));
    let volume_width = App::status_width(&volume);
    let fits = status_bar_fit(
        &StatusBarWidths {
            visual: visual_width,
            armed: armed_width,
            remote: remote_width,
            mute: mute_width,
            volume: volume_width,
        },
        available,
    );
    let any_fit = !matches!(fits, StatusBarFit::None);
    let visibility = StatusBarVisibility {
        visual: visual_width > 0 && any_fit,
        armed: armed_width > 0 && any_fit,
        remote: remote_width > 0
            && matches!(
                fits,
                StatusBarFit::All | StatusBarFit::WithoutMute | StatusBarFit::WithoutVolume
            ),
        volume: matches!(
            fits,
            StatusBarFit::All | StatusBarFit::WithoutMute | StatusBarFit::WithoutRemote
        ),
    };
    StatusBarLeftSegments {
        mute,
        volume,
        remote,
        armed,
        visual,
        volume_width,
        remote_width,
        visual_width,
        fits,
        visibility,
    }
}

fn render_status_bar_left(
    f: &mut Frame,
    area: Rect,
    bar_style: Style,
    segments: StatusBarLeftSegments,
) -> (StatusBarRegions, u16) {
    let mut regions = StatusBarRegions::default();
    let mut spans: Vec<Span> = Vec::new();
    if segments.visibility.visual {
        let visual_x = area.x + App::status_width(&spans);
        App::append_status(&mut spans, segments.visual.unwrap_or_default());
        regions.visual_clear = Some(Rect {
            x: visual_x,
            y: area.y,
            width: segments.visual_width,
            height: 1,
        });
    }
    if segments.visibility.armed {
        App::append_status(&mut spans, segments.armed.unwrap_or_default());
    }
    if segments.visibility.volume {
        let vol_x = area.x + App::status_width(&spans);
        App::append_status(&mut spans, segments.volume);
        regions.volume = Some(Rect {
            x: vol_x,
            y: area.y,
            width: segments.volume_width,
            height: 1,
        });
    }
    let remote_x = segments
        .visibility
        .remote
        .then(|| area.x + App::status_width(&spans) + u16::from(!spans.is_empty()));
    if segments.visibility.remote {
        App::append_status(&mut spans, segments.remote);
        regions.remote = remote_x.map(|x| Rect {
            x,
            y: area.y,
            width: segments.remote_width,
            height: 1,
        });
    }
    if matches!(segments.fits, StatusBarFit::All | StatusBarFit::WithoutMute) {
        if let Some(mute) = segments.mute {
            let mute_x = area.x + App::status_width(&spans);
            let mute_width = App::status_width(&mute);
            App::append_status(&mut spans, mute);
            regions.mute = Some(Rect {
                x: mute_x,
                y: area.y,
                width: mute_width,
                height: 1,
            });
        }
    }

    // `left_content_w` tracks how far the left segment actually extends after
    // the above priority drop, so the right-segment overlap check can compare
    // against the real left edge instead of a hardcoded constant.
    let label_w = spans_width(&spans);
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
    (regions, label_w)
}

/// Total rendered width of a span run, saturating instead of truncating
/// per-span or overflowing the `u16` sum.
fn spans_width(spans: &[Span<'_>]) -> u16 {
    spans
        .iter()
        .map(|s| u16::try_from(s.content.width()).unwrap_or(u16::MAX))
        .fold(0, u16::saturating_add)
}

fn render_status_bar_right(
    f: &mut Frame,
    area: Rect,
    bar_style: Style,
    left_content_width: u16,
    right_spans: &[Span<'static>],
) {
    if !right_spans.is_empty() {
        let right_w = spans_width(right_spans);
        // Compare against `left_content_width` (pill + session label, from Task 2),
        // not a hardcoded pill-only width -- otherwise this check passes while
        // the right segment still overlaps a rendered session label (e.g.
        // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
        let left_end = area.x + left_content_width;
        let right_x = area.x + area.width.saturating_sub(right_w);
        if right_w > 0 && right_x > left_end {
            let right_rect = Rect {
                x: right_x,
                y: area.y,
                width: right_w,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(right_spans.to_vec())).style(bar_style),
                right_rect,
            );
        }
        // else: terminal too narrow -- the right segment drops silently
        // rather than overlapping the pill or the session label.
        // (Design doc's open question on narrow-terminal truncation: right
        // segment yields first.)
    }
}

pub(in crate::app) fn render_status_bar(
    f: &mut Frame,
    area: Rect,
    model: &StatusBarModel,
) -> StatusBarRegions {
    // Keep the row itself darker so the pills read as segments sitting on top of it.
    let bar_style =
        Style::default().bg(palette::surface_colors(palette::Surface::StatusBar, false).fill);
    // `Clear` blanks every cell's symbol first (task 12.2): a bare
    // `Block::style` only recolors a cell, it never overwrites a stale
    // glyph left by whatever painted this placement before the status bar
    // owned it.
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(bar_style), area);

    let segments = status_bar_left_segments(model, area.width);
    let (regions, left_content_width) = render_status_bar_left(f, area, bar_style, segments);
    render_status_bar_right(f, area, bar_style, left_content_width, &model.right);
    regions
}

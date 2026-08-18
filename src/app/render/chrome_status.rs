#![allow(unused_imports)]

use super::super::ui_util::*;
use super::chrome::{daemon_endpoint_label, service_state_color};
use super::indicators;
use super::RENDER_FILTER;
use crate::app::layout::LayoutPlayback;
use crate::app::{palette, App, PanelFocus, RemoteSlotState, TABBAR_LEFT_RESERVE};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

/// The bundled app-icon bolt (assets/icon.svg rasterized for the tray into
/// assets/tray_icon.bin): a 24×24 raw RGBA pixmap of the bolt in magenta
/// over transparent, with every opaque pixel at partial alpha (the source
/// renders at ~29% opacity). Used as the status-bar Emby indicator, tinted
/// and normalized to full opacity per connection state.
static EMBY_BOLT_BYTES: &[u8] = include_bytes!("../../../assets/tray_icon.bin");
const EMBY_BOLT_SIZE: u32 = 24;

/// Cache-key prefix for status-bar Emby bolt protocols (one entry per tint).
const EMBY_STATUS_BOLT_KEY_PREFIX: &str = "__emby_status_bolt__:";

/// Tint the bundled bolt to `target`, normalized to full opacity.
///
/// The source's opaque pixels are partial-alpha (max 75/255), so alpha is
/// rescaled to each pixel's share of that max: the bolt interior becomes
/// solid `target` while antialiased edges stay proportionally softer.
/// Transparent pixels are left untouched.
pub(super) fn emby_bolt_tinted(target: Color) -> image::DynamicImage {
    let src = image::RgbaImage::from_raw(EMBY_BOLT_SIZE, EMBY_BOLT_SIZE, EMBY_BOLT_BYTES.to_vec())
        .expect("assets/tray_icon.bin is a 24x24 raw RGBA pixmap");
    let max_a = src.pixels().map(|p| p[3]).max().unwrap_or(0).max(1);
    let (tr, tg, tb) = match target {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    };
    let tinted = image::RgbaImage::from_fn(EMBY_BOLT_SIZE, EMBY_BOLT_SIZE, |x, y| {
        let p = src.get_pixel(x, y);
        if p[3] == 0 {
            *p
        } else {
            let cover = p[3] as u32 * 255 / max_a as u32; // 0..=255 coverage
            image::Rgba([
                (tr as u32 * cover / 255) as u8,
                (tg as u32 * cover / 255) as u8,
                (tb as u32 * cover / 255) as u8,
                cover as u8,
            ])
        }
    });
    image::DynamicImage::ImageRgba8(tinted)
}

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

    pub(super) fn volume_status_spans(&self) -> Vec<Span<'static>> {
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
            Span::styled(" ", Style::default().bg(palette::SURFACE_STATUS_PILL)),
            Span::styled(
                icon,
                Style::default()
                    .fg(palette::PLAYBACK_META_FG)
                    .bg(palette::SURFACE_STATUS_PILL),
            ),
            Span::styled(
                format!(" {volume}"),
                Style::default()
                    .fg(palette::AQUA)
                    .bg(palette::SURFACE_STATUS_PILL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
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

    /// The status-bar Emby indicator: the bundled app-icon bolt tinted to
    /// `color`, cached in `card_image_states` under a per-colour key.
    /// Returns `None` when the active image protocol can't render a one-row
    /// image (halfblocks) or no protocol is ready yet.
    pub(super) fn emby_status_bolt_protocol_mut(
        &mut self,
        color: Color,
    ) -> Option<&mut ratatui_image::thread::ThreadProtocol> {
        if self.current_protocol_suffix() == "halfblock" {
            return None;
        }
        let key = format!("{EMBY_STATUS_BOLT_KEY_PREFIX}{color:?}");
        if !self.card_image_states.contains_key(&key) {
            let entry = self.build_cached_image(&key, Some(emby_bolt_tinted(color)));
            self.card_image_states.insert(key.clone(), entry);
        }
        self.cached_image_protocol_mut(&key)
    }

    /// Persistent bottom status bar. Left side: volume, connection,
    /// and mute status groups. Right side: queue source/save-state/scope
    /// detail and the service-state glyphs (Emby, Audiobookshelf,
    /// stay-alive, shared-data).
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
        let (daemon_endpoint, username) = {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            (cfg.daemon_client_endpoint.clone(), cfg.username.clone())
        };
        let remote_status = if show_session_pill {
            self.remote_status_spans(remote_state, &daemon_endpoint)
        } else {
            Vec::new()
        };
        // Stay-alive (local daemon) indicator: red when the daemon is the active
        // target (stay-alive's brand colour), yellow when the daemon is lost —
        // the error state since red already means active, grey when not in use.
        let alive_color = if self.daemon_lost_modal.is_some() {
            palette::YELLOW
        } else if self.is_local_daemon() {
            palette::RED
        } else {
            palette::MUTED
        };
        let shared_color = if self.shared_client.as_ref().is_some_and(|client| {
            matches!(
                client.state(),
                mbv_core::shared_client::SharedClientState::Shared
            )
        }) {
            palette::FOAM
        } else {
            palette::MUTED
        };
        let mute_status = self.mute_status_spans();
        let vol_status = self.volume_status_spans();

        // Preserve the existing left-segment overflow order: mute drops
        // first, then the volume pill, then remote. (The service-state
        // glyphs now live in the right segment.)
        let remote_w = Self::status_width(&remote_status);
        let mute_w: u16 = mute_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let vol_w = Self::status_width(&vol_status);
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
        let fits_all = joined_width(&[remote_w, mute_w, vol_w]) <= available;
        let fits_without_mute = !fits_all && joined_width(&[remote_w, vol_w]) <= available;
        let fits_without_volume =
            !fits_all && !fits_without_mute && joined_width(&[remote_w, mute_w]) <= available;
        let fits_without_remote = !fits_all
            && !fits_without_mute
            && !fits_without_volume
            && joined_width(&[mute_w, vol_w]) <= available;

        let show_remote = remote_w > 0 && (fits_all || fits_without_mute || fits_without_volume);
        let show_volume = fits_all || fits_without_mute || fits_without_remote;

        let mut spans: Vec<Span> = Vec::new();
        if show_volume {
            let vol_x = area.x + Self::status_width(&spans);
            Self::append_status(&mut spans, vol_status);
            layout.ind_vol = Rect {
                x: vol_x,
                y: area.y,
                width: vol_w,
                height: 1,
            };
        } else {
            layout.ind_vol = Rect::default();
        }
        let remote_x =
            show_remote.then(|| area.x + Self::status_width(&spans) + u16::from(!spans.is_empty()));
        if show_remote {
            Self::append_status(&mut spans, remote_status);
        }
        self.render_remote_status_hitbox(layout, area, remote_x, remote_w);
        if fits_all || fits_without_mute {
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
                    if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
                {
                    Some(("ALBUM".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Series
                    if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
                {
                    Some(("SERIES".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Shuffle
                    if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
                {
                    Some(("SHUFFLE".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Remote
                    if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
                {
                    Some(("REMOTE Q".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Collection { collection_type }
                    if matches!(self.effective_panel_focus(), PanelFocus::Queue) =>
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
            // Service-state glyphs — Audiobookshelf, stay-alive, shared-data —
            // always visible, coloured by state (brand colour when active,
            // grey when inactive; stay-alive daemon lost = yellow). One
            // leading space per glyph, no trailing space. The Emby bolt
            // renders as a terminal image just before this group.
            let service_spans: Vec<Span> = vec![
                Span::raw(" "),
                Span::styled(
                    "\u{EDE2}",
                    Style::default().fg(service_state_color(
                        self.audiobookshelf_runtime.state,
                        palette::AMBER,
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
                Span::styled("\u{F1C0}", Style::default().fg(shared_color)),
                // Right edge of the segment: the shared-data glyph gets its own
                // trailing margin like a pill.
                Span::raw(" "),
            ];
            // The Emby bolt (assets/icon.svg) tinted by connection state
            // (green=ready, red=configured-but-down, grey=not configured).
            // Only an image can draw it, so it appears only on terminals with
            // a pixel-precise protocol (kitty/sixel/iterm2); halfblocks
            // cannot shape a one-row image, so those terminals just omit it.
            let emby_color = service_state_color(self.emby_runtime.state, palette::AQUA);
            let emby_protocol = self.emby_status_bolt_protocol_mut(emby_color);
            let emby_size = emby_protocol.as_ref().and_then(|state| {
                state.size_for(
                    ratatui_image::Resize::Scale(Some(RENDER_FILTER)),
                    ratatui::layout::Size {
                        width: area.width,
                        height: 1,
                    },
                )
            });
            let emby_w = emby_size.map(|s| s.width).unwrap_or(0);
            let left_text_w: u16 = right_spans.iter().map(|s| s.content.width() as u16).sum();
            let service_w: u16 = service_spans.iter().map(|s| s.content.width() as u16).sum();
            // Gap between the username/source pill and the bolt only when both
            // are present; the service group's own leading space doubles as
            // the gap after the bolt.
            let bolt_gap = u16::from(!right_spans.is_empty() && emby_w > 0);
            let right_w = left_text_w
                .saturating_add(bolt_gap)
                .saturating_add(emby_w)
                .saturating_add(service_w);
            // Remote queue scope is omitted here: the active queue is already
            // apparent from the queue UI.
            if right_w > 0 {
                // Compare against `left_content_w` (pill + session label, from Task 2),
                // not a hardcoded pill-only width -- otherwise this check passes while
                // the right segment still overlaps a rendered session label (e.g.
                // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
                let left_end = area.x + left_content_w;
                let right_x = area.x + area.width.saturating_sub(right_w);
                if right_x > left_end {
                    if !right_spans.is_empty() {
                        f.render_widget(
                            Paragraph::new(Line::from(right_spans)).style(bar_style),
                            Rect {
                                x: right_x,
                                y: area.y,
                                width: left_text_w,
                                height: 1,
                            },
                        );
                    }
                    if let (Some(state), Some(size)) = (emby_protocol, emby_size) {
                        type SImg =
                            ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                        f.render_stateful_widget(
                            SImg::default()
                                .resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                            Rect {
                                x: right_x + left_text_w + bolt_gap,
                                y: area.y,
                                width: size.width,
                                height: size.height,
                            },
                            state,
                        );
                    }
                    f.render_widget(
                        Paragraph::new(Line::from(service_spans)).style(bar_style),
                        Rect {
                            x: right_x + left_text_w + bolt_gap + emby_w,
                            y: area.y,
                            width: service_w,
                            height: 1,
                        },
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

#[cfg(test)]
mod tests {
    use super::emby_bolt_tinted;
    use ratatui::style::Color;

    #[test]
    fn emby_bolt_tinted_normalizes_and_tints() {
        let img = emby_bolt_tinted(Color::Rgb(1, 2, 3));
        let rgba = img.to_rgba8();
        assert_eq!(rgba.dimensions(), (24, 24));
        let (mut opaque, mut transparent) = (0, 0);
        for p in rgba.pixels() {
            if p[3] == 255 {
                opaque += 1;
                assert_eq!((p[0], p[1], p[2]), (1, 2, 3));
            } else if p[3] == 0 {
                transparent += 1;
                assert_eq!((p[0], p[1], p[2]), (0, 0, 0));
            }
            // Nontransparent pixels are premultiplied toward the target.
            if p[3] > 0 {
                assert_eq!(p[0], (u16::from(p[3]) * 1 / 255) as u8);
                assert_eq!(p[1], (u16::from(p[3]) * 2 / 255) as u8);
                assert_eq!(p[2], (u16::from(p[3]) * 3 / 255) as u8);
            }
        }
        assert!(
            opaque > 0,
            "the bolt interior is fully opaque after tinting"
        );
        assert!(transparent > 0, "the bolt sits on a transparent background");
    }
}

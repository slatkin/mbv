//! Status-bar span builders (task 2.2).
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`crates/mbv-components/src/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. The `impl App`
//! methods here are content production only: they read app state and build
//! the pill/right-segment spans the shell projects into the component.
//!
//! The Local/Remote queue-scope pills are queue concern and paint in the
//! `QueueColumn` footer (`render_queue_status`), never here.

use crate::app::{App, PanelFocus, RemoteSlotState};
use mbv_render::components::chrome::daemon_endpoint_label;
use mbv_render::components::widgets::service_state_color;
use mbv_render::indicators;
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

    /// The label naming the cast receiver that is the owner's playback
    /// output, per the last `CtrlEvent::PlaybackOutput` (design D13). Shared
    /// by the status-bar remote pill and the queue header so both name the
    /// receiver consistently; the owner's report carries the discovered
    /// friendly name.
    fn cast_target_label(&self) -> Option<String> {
        match &self.playback_output {
            // A target marked disconnected must not read as connected: the
            // pill falls back to the local label until the reconnect lands,
            // so it never contradicts the header's "Cast: disconnected".
            Some(mbv_ctrl::OutputState::Cast {
                name: _,
                connection: mbv_ctrl::CastConnection::Disconnected,
                ..
            }) => None,
            Some(mbv_ctrl::OutputState::Cast { name, .. }) => Some(name.clone()),
            _ => None,
        }
    }

    /// Whether the remote pill shows its connected state: a cast receiver is
    /// attached and live, or the remote slot names an attached session or direct route.
    fn remote_connected(&self, remote_state: RemoteSlotState) -> bool {
        self.cast_target_label().is_some()
            || matches!(
                remote_state,
                RemoteSlotState::AttachedSession | RemoteSlotState::DirectRemote
            )
    }

    /// The remote pill label, gap included. An attached cast receiver names it
    /// first; otherwise the remote slot names it, and this machine's device
    /// name is the fallback. `or_else` leaves the slot unread while a receiver
    /// is attached.
    fn remote_label(&self, remote_state: RemoteSlotState, daemon_endpoint: &str) -> String {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let target = self.cast_target_label().or_else(|| match remote_state {
            RemoteSlotState::LocalDaemon => None,
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
        });
        match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_emby::device_name()),
        }
    }

    pub(in crate::app) fn remote_status_spans(
        &self,
        remote_state: RemoteSlotState,
        daemon_endpoint: &str,
    ) -> Vec<Span<'static>> {
        let remote_on = self.remote_connected(remote_state);
        let glyph_style = Style::default()
            .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill)
            .fg(ratatui::style::Color::White);

        let label = self.remote_label(remote_state, daemon_endpoint);
        let label_style = Style::default()
            .fg(if remote_on {
                mbv_theme::Role::Accent.color()
            } else {
                ratatui::style::Color::Black
            })
            .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill);

        vec![
            Span::styled(
                " ",
                Style::default().bg(mbv_theme::surface_colors(
                    mbv_theme::Surface::StatusBarPill,
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
                Style::default().bg(mbv_theme::surface_colors(
                    mbv_theme::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
        ]
    }

    /// The playback target's host label plus whether it names a remote target
    /// (a cast attachment, an attached session, or a direct-remote
    /// route/label): the attached receiver's name, the connected session's
    /// device name (falling back to its host), the direct-remote route/label,
    /// or this machine's device name when playback is local — computed
    /// together from one
    /// `remote_slot_state()` resolution for callers (the queue header sync)
    /// that need both every tick. Callers style the returned label and the
    /// header row shows it verbatim.
    pub(in crate::app) fn playback_host_label_and_remote(&self) -> (String, bool) {
        let remote_state = self.remote_slot_state();
        let is_remote = self.remote_connected(remote_state);
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let label = self.remote_label(remote_state, &daemon_endpoint);
        (label.trim_start().to_string(), is_remote)
    }

    /// The playlist source pill content (glyph plus name), without outer pads.
    /// The painter adds the leading/trailing pads and the autosave delimiter.
    /// Empty when no playlist is loaded: the glyph alone, grey. Loaded: blue.
    pub(in crate::app) fn playlist_status_spans(&self) -> Vec<Span<'static>> {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let (label, loaded) = match self.playback_queue().source() {
            mbv_queue::QueueSource::Playlist { name, .. } => (format!("{gap}{name}"), true),
            _ => (String::new(), false),
        };
        let fg = if loaded {
            mbv_theme::Role::TextMetadata.color()
        } else {
            mbv_theme::Role::TextMuted.color()
        };
        let style = Style::default()
            .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill)
            .fg(fg);
        let glyph = if self.use_nerd_fonts {
            "\u{f0411}"
        } else {
            "\u{1F5AD}"
        };
        if label.is_empty() {
            vec![Span::styled(glyph, style)]
        } else {
            vec![Span::styled(glyph, style), Span::styled(label, style)]
        }
    }

    /// The autosave indicator content, without outer pads. Present only while
    /// save-on-consume is on for a saved playlist. Independent of the dirty
    /// flag: both can show at one time. The painter puts it left of the
    /// playlist pill with one space delimiter.
    pub(in crate::app) fn autosave_status_spans(&self) -> Option<Vec<Span<'static>>> {
        let autosave_on = self.queue_is_saved_playlist() && {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
        };
        if !autosave_on {
            return None;
        }
        let pill_bg = mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill;
        let style = Style::default()
            .fg(mbv_theme::Role::Accent.color())
            .bg(pill_bg);
        Some(if self.use_nerd_fonts {
            vec![Span::styled("\u{f18ea}", style)]
        } else {
            vec![Span::styled("AUTOSAVE", style)]
        })
    }

    /// The dirty indicator content, without outer pads. Present only while
    /// the queue differs from its saved playlist. Independent of autosave.
    /// The painter puts it right-anchored.
    pub(in crate::app) fn dirty_status_spans(&self) -> Option<Vec<Span<'static>>> {
        if !self.queue_dirty {
            return None;
        }
        let pill_bg = mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill;
        let style = Style::default()
            .fg(mbv_theme::Role::TextFocusAccent.color())
            .add_modifier(Modifier::BOLD)
            .bg(pill_bg);
        Some(if self.use_nerd_fonts {
            vec![Span::styled("\u{f0f42}", style)]
        } else {
            vec![Span::styled("UNSAVED", style)]
        })
    }

    pub(in crate::app) fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
        self.playback_display_target()
            .displayed_mute(self)
            .then(|| {
                vec![
                    Span::styled(
                        " ",
                        Style::default().bg(mbv_theme::surface_colors(
                            mbv_theme::Surface::StatusBarPill,
                            false,
                        )
                        .fill),
                    ),
                    Span::styled(
                        "muted",
                        Style::default()
                            .fg(mbv_theme::Role::StatusError.color())
                            .bg(
                                mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                                    .fill,
                            )
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " ",
                        Style::default().bg(mbv_theme::surface_colors(
                            mbv_theme::Surface::StatusBarPill,
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
                    Style::default().bg(mbv_theme::surface_colors(
                        mbv_theme::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
                Span::styled(
                    " PREFIX ",
                    Style::default()
                        .fg(mbv_theme::Role::Accent.color())
                        .bg(
                            mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                                .fill,
                        )
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " ",
                    Style::default().bg(mbv_theme::surface_colors(
                        mbv_theme::Surface::StatusBarPill,
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
                Style::default().bg(mbv_theme::surface_colors(
                    mbv_theme::Surface::StatusBarPill,
                    false,
                )
                .fill),
            ),
            Span::styled(
                icon,
                Style::default()
                    .fg(mbv_theme::Role::PlaybackMetaFg.color())
                    .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill),
            ),
            Span::styled(
                format!(" {volume}"),
                Style::default()
                    .fg(mbv_theme::Role::Accent.color())
                    .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    }

    /// The shuffle-order indicator content, without outer pads (queue-play-order
    /// design D7). Present only while the owner's play order is Shuffle, and
    /// independent of panel focus — unlike the queue-source label, which shows
    /// only while the Queue panel is focused. The painter adds the pill
    /// background; the span carries the glyph or fallback text and the color.
    pub(in crate::app) fn shuffle_status_span(&self) -> Option<Span<'static>> {
        let shuffle_on = self.playback_queue().play_order() == mbv_queue::PlayOrderMode::Shuffle;
        shuffle_on.then(|| {
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f074}"
                } else {
                    "SHUFFLE"
                },
                Style::default()
                    .fg(mbv_theme::Role::Accent.color())
                    .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill),
            )
        })
    }

    /// The status row's right segment: queue-source scope label, shuffle-order
    /// indicator, username, and the service-state glyphs (Emby,
    /// Audiobookshelf, stay-alive) — always visible, coloured by state. Built
    /// shell-side (task 2.2); the mounted `StatusBarPanel` positions and
    /// paints it.
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
            mbv_theme::Role::TextMuted.color()
        } else if self.player.is_remote_disconnected() {
            mbv_theme::Role::TextFocusAccent.color()
        } else {
            mbv_theme::Role::StatusError.color()
        };
        let mut right_spans: Vec<Span> = Vec::new();
        let source_label = queue_source_status_label(
            self.playback_queue().source(),
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
                    Style::default().fg(color).bg(mbv_theme::surface_colors(
                        mbv_theme::Surface::StatusBarPill,
                        false,
                    )
                    .fill),
                ),
            );
        }
        // The shuffle indicator does not depend on Queue focus (design D7):
        // it follows the owner's play order, not the panel under the cursor.
        if let Some(shuffle) = self.shuffle_status_span() {
            append_right(&mut right_spans, shuffle);
        }
        if !username.is_empty() {
            if !right_spans.is_empty() {
                right_spans.push(Span::raw(" "));
            }
            right_spans.push(Span::styled(
                " 🯅",
                Style::default()
                    .fg(mbv_theme::Role::TextMetadata.color())
                    .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill),
            ));
            right_spans.push(Span::styled(
                format!(" {username} "),
                Style::default()
                    .fg(mbv_theme::Role::PlaybackMetaFg.color())
                    .bg(mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false).fill),
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
                    mbv_theme::Role::Accent.color(),
                )),
            ),
            Span::raw(" "),
            Span::styled(
                "\u{EDE2}",
                Style::default().fg(mbv_theme::Role::AccentAudiobookshelf.color()),
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
}

fn queue_source_status_label(
    source: &mbv_queue::QueueSource,
    queue_focused: bool,
) -> Option<(String, Color)> {
    if !queue_focused {
        return None;
    }
    let label = match source {
        mbv_queue::QueueSource::Album => "ALBUM".to_string(),
        mbv_queue::QueueSource::Series => "SERIES".to_string(),
        // Shuffle is no pill (2026-10-09 user rule): the shuffle-sourced
        // queue's pill was never requested, so it reads as no label at all.
        mbv_queue::QueueSource::Shuffle => return None,
        mbv_queue::QueueSource::Remote => "REMOTE Q".to_string(),
        mbv_queue::QueueSource::Collection { collection_type } => collection_type.to_uppercase(),
        mbv_queue::QueueSource::Playlist { .. } | mbv_queue::QueueSource::Unknown => {
            return None;
        }
    };
    Some((label, mbv_theme::Role::TextMuted.color()))
}

#[cfg(test)]
mod tests {
    use crate::app::PanelFocus;
    use crate::app::RemoteSlotState;
    use crate::app::state::queue_view::QueueView;
    use crate::app::tests::{emby_unified_state, make_app_stub, make_items};
    use mbv_queue::PlayOrderMode;

    /// Same consistency contract as the queue header: the status-bar remote
    /// pill names the owner's cast output instead of this machine's device
    /// name, with the remote-on styling.
    #[test]
    fn attached_cast_receiver_names_the_status_bar_pill() {
        let mut app = make_app_stub();
        app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
            mbv_ctrl::OutputState::Cast {
                receiver_id: "device-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::CastConnection::Connected,
            },
        ));

        let spans = app.remote_status_spans(RemoteSlotState::LocalDaemon, "endpoint");

        assert!(
            spans
                .iter()
                .any(|span| span.content.contains("Living Room")),
            "the pill names the attached receiver: {:?}",
            spans.iter().map(|s| s.content.as_ref()).collect::<Vec<_>>()
        );
    }

    /// The owner's cast output names the queue header's host, matching
    /// the connected icon cast-session-control "Playing while attached"
    /// pairs with it: the header previously showed the connected icon but
    /// labelled the host with this machine's device name.
    #[test]
    fn attached_cast_receiver_names_the_queue_header_host() {
        let mut app = make_app_stub();
        app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
            mbv_ctrl::OutputState::Cast {
                receiver_id: "device-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::CastConnection::Connected,
            },
        ));

        let (host, is_remote) = app.playback_host_label_and_remote();

        assert_eq!(host, "Living Room");
        assert!(is_remote);
    }

    /// A cast output reported disconnected (7.6, D13) must not read as
    /// connected: the pill falls back to the local label until the
    /// reconnect lands, so it never contradicts the header's "Cast:
    /// disconnected".
    #[test]
    fn a_disconnected_cast_target_stops_naming_the_status_bar_pill() {
        let mut app = make_app_stub();
        app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
            mbv_ctrl::OutputState::Cast {
                receiver_id: "device-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::CastConnection::Disconnected,
            },
        ));

        let spans = app.remote_status_spans(RemoteSlotState::LocalDaemon, "endpoint");

        assert!(
            !spans
                .iter()
                .any(|span| span.content.contains("Living Room")),
            "the pill stops naming a disconnected receiver: {:?}",
            spans.iter().map(|s| s.content.as_ref()).collect::<Vec<_>>()
        );
    }

    /// queue-play-order spec, "The footer shows Shuffle": the shuffle
    /// indicator follows the owner's play order, not the panel under the
    /// cursor — it shows with the Library panel focused (design D7), where
    /// the queue-source label stays hidden.
    #[test]
    fn shuffle_indicator_shows_with_the_library_panel_focused() {
        let mut app = make_app_stub();
        app.panel_focus = PanelFocus::Library;
        let mut state = emby_unified_state(&make_items(2), 0);
        state.status.play_order = PlayOrderMode::Shuffle;
        app.local_view = QueueView::from_snapshot(&state);

        let spans = app.status_bar_right_spans();

        assert!(
            spans.iter().any(|span| span.content.contains("SHUFFLE")),
            "shuffle indicator missing from the status row with the Library panel focused: {:?}",
            spans.iter().map(|s| s.content.as_ref()).collect::<Vec<_>>()
        );
    }
}

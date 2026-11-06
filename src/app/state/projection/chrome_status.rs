//! Status-bar span builders (task 2.2).
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`crates/mbv-components/src/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. The `impl App`
//! methods here are content production only: they read app state and build
//! the pill/right-segment spans the shell projects into the component.
//!
//! The Local/Peer queue-link pills are queue concern and paint in the
//! `QueueColumn` footer (`render_queue_status`), never here.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, PanelFocus, QueueLink, RemoteSlotState};
use mbv_render::components::widgets::service_state_color;
use mbv_render::indicators;
use mbv_ui_model::toast::StatusToast;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

impl App {
    /// The live toast's presentation for the status row, or `None` when no
    /// toast should render in-app (spec `toast-notification-semantics`).
    /// Restores the in-app toast surface dropped during the `TuiRealm`
    /// migration (commit `e9f3ab957`): Neutral toasts always render in-app;
    /// Success/Warning/Error toasts render in-app only when system
    /// notifications are disabled or the desktop notification failed
    /// (`notif_failed`), because a delivered desktop notification hides the
    /// in-app row. The presentation is neutral (extract-client-core row 4.6):
    /// the Severity's colour is the painters' choice, and the presented
    /// facts — the text and the severity — ride the Status bar's Surface
    /// state.
    pub(in crate::app) fn toast_presentation(&self) -> Option<StatusToast> {
        if self.status.is_empty() || !self.status_toast_live() {
            return None;
        }
        let delivered_offscreen = self.system_notifications
            && self.status_severity != ToastSeverity::Neutral
            && !self.notif_failed;
        if delivered_offscreen {
            return None;
        }
        Some(StatusToast {
            text: self.status.clone(),
            severity: self.status_severity,
        })
    }

    /// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
    /// Returns None if the local player is not active.
    /// Callers wrap these in [ ... ] with whatever surrounding style they need.
    pub(in crate::app) fn build_status_indicator_spans(&self) -> Option<Vec<Span<'static>>> {
        let data = crate::app::LocalPlaybackTarget::indicator_data(self)?;
        Some(indicators::indicator_spans(
            self.indicator_style,
            &data,
            self.use_nerd_fonts,
        ))
    }

    /// The label naming the remote receiver that is the owner's playback
    /// output, per the last `CtrlEvent::PlaybackOutput` (design D12/D13).
    /// Feeds the queue header so it names the output consistently; the
    /// owner's report carries the discovered or device name. An Emby Session
    /// output names the Session device. A disconnected output keeps the
    /// device name with a " (disconnected)" suffix so the header presents
    /// it as disconnected (spec `cast-session-control` "Connection drops
    /// while attached", `owner-playback-output` "Device disappears"; bug:
    /// a disconnected output looked like local playback).
    fn remote_output_label(&self) -> Option<String> {
        match &self.playback_output {
            Some(
                mbv_ctrl::OutputState::Cast {
                    connection, name, ..
                }
                | mbv_ctrl::OutputState::EmbySession {
                    connection, name, ..
                },
            ) => match connection {
                mbv_ctrl::OutputConnection::Disconnected => Some(format!("{name} (disconnected)")),
                mbv_ctrl::OutputConnection::Connecting | mbv_ctrl::OutputConnection::Connected => {
                    Some(name.clone())
                }
            },
            Some(mbv_ctrl::OutputState::Local) | None => None,
        }
    }

    /// Whether the playback output reads as connected for the queue header:
    /// a remote output (Cast receiver or Emby Session) is attached — live
    /// or disconnected, which must still present as remote — or the remote
    /// slot names a direct route.
    fn remote_connected(&self, remote_state: RemoteSlotState) -> bool {
        self.remote_output_label().is_some()
            || matches!(remote_state, RemoteSlotState::DirectRemote)
    }

    /// The connected Player-owner endpoint's host label (PR #973 review):
    /// names the attachment the Client actually holds — never an assumed
    /// or configured host. `Local` carries no peer name; the caller's
    /// device-name fallback covers it.
    fn connected_player_endpoint_label(&self) -> Option<String> {
        self.player_endpoint
            .as_ref()
            .and_then(|endpoint| match endpoint {
                mbv_remote_player::DaemonEndpoint::Tcp(addr) => Some(addr.ip().to_string()),
                mbv_remote_player::DaemonEndpoint::Unix(path) => path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string),
                mbv_remote_player::DaemonEndpoint::Local => None,
            })
    }

    /// The playback target's label, gap-prefixed. An attached remote output
    /// names it first; otherwise the remote slot names it, and this machine's
    /// device name is the fallback. `or_else` leaves the slot unread while a
    /// receiver is attached. The queue-header caller trims the gap.
    fn remote_label(&self, remote_state: RemoteSlotState) -> String {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let target = self.remote_output_label().or_else(|| match remote_state {
            RemoteSlotState::LocalDaemon => None,
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| self.connected_player_endpoint_label()),
        });
        match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_emby::device_name()),
        }
    }

    /// The playback target's host label plus whether it names a remote target
    /// (a cast attachment, an Emby Session output, or a direct-remote
    /// route/label): the output's name, the direct-remote route/label, or
    /// this machine's device name when playback is local -- computed together
    /// from one `remote_slot_state()` resolution for callers (the queue header
    /// sync) that need both every tick. Callers style the returned label and
    /// the header row shows it verbatim.
    pub(in crate::app) fn playback_host_label_and_remote(&self) -> (String, bool) {
        let remote_state = self.remote_slot_state();
        let is_remote = self.remote_connected(remote_state);
        let label = self.remote_label(remote_state);
        (label.trim_start().to_string(), is_remote)
    }

    /// The shown queue's Player owner's name (design D7, `queue-playback-panel`
    /// delta): the connected peer by device name while the shown queue is the
    /// peer link, else this machine's device name (the local Owner). The peer
    /// resolution mirrors `remote_label`: route name,
    /// session device name, connected endpoint host.
    pub(in crate::app) fn queue_owner_label(&self) -> String {
        if self.playing_queue_link() == QueueLink::Peer {
            self.active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| self.connected_player_endpoint_label())
                .unwrap_or_else(mbv_emby::device_name)
        } else {
            mbv_emby::device_name()
        }
    }

    pub(in crate::app) fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
        crate::app::LocalPlaybackTarget::displayed_mute(self).then(|| {
            vec![
                Span::styled(
                    " ",
                    Style::default().bg(mbv_theme::surface_colors(
                        mbv_theme::Surface::StatusBarPill,
                        false,
                    )
                    .fill
                    .to_ratatui()),
                ),
                Span::styled(
                    "muted",
                    Style::default()
                        .fg(mbv_theme::Role::StatusError.color().to_ratatui())
                        .bg(
                            mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                                .fill
                                .to_ratatui(),
                        )
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " ",
                    Style::default().bg(mbv_theme::surface_colors(
                        mbv_theme::Surface::StatusBarPill,
                        false,
                    )
                    .fill
                    .to_ratatui()),
                ),
            ]
        })
    }

    pub(in crate::app) fn volume_status_spans(&self) -> Vec<Span<'static>> {
        let volume = crate::app::LocalPlaybackTarget::displayed_volume(self);
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
                .fill
                .to_ratatui()),
            ),
            Span::styled(
                icon,
                Style::default()
                    .fg(mbv_theme::Role::PlaybackMetaFg.color().to_ratatui())
                    .bg(
                        mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                            .fill
                            .to_ratatui(),
                    ),
            ),
            Span::styled(
                format!(" {volume}"),
                Style::default()
                    .fg(mbv_theme::Role::Accent.color().to_ratatui())
                    .bg(
                        mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                            .fill
                            .to_ratatui(),
                    )
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    }

    /// The status row's right segment: queue-source link label, username,
    /// and the service-state glyphs (Emby,
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
            mbv_theme::Role::TextMuted.color().to_ratatui()
        } else if self.player.is_remote_disconnected() {
            mbv_theme::Role::TextFocusAccent.color().to_ratatui()
        } else {
            mbv_theme::Role::StatusError.color().to_ratatui()
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
                    .fill
                    .to_ratatui()),
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
                    .fg(mbv_theme::Role::TextMetadata.color().to_ratatui())
                    .bg(
                        mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                            .fill
                            .to_ratatui(),
                    ),
            ));
            right_spans.push(Span::styled(
                format!(" {username} "),
                Style::default()
                    .fg(mbv_theme::Role::PlaybackMetaFg.color().to_ratatui())
                    .bg(
                        mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                            .fill
                            .to_ratatui(),
                    ),
            ));
        }
        // Service-state indicators — Emby and Audiobookshelf coloured by
        // state (brand colour when ready, grey when not configured, error
        // otherwise), stay-alive daemon lost = yellow. One leading space
        // per indicator, no trailing space. Without Nerd Fonts the Emby
        // and Audiobookshelf private-use code points fall back to the
        // plain labels the sessions panel uses (bug fix: the service
        // glyphs ignored `use_nerd_fonts` and painted as missing-glyph
        // boxes under the default config).
        right_spans.extend([
            Span::raw(" "),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{F06B4}"
                } else {
                    "EMBY"
                },
                Style::default().fg(service_state_color(
                    self.emby_runtime.state,
                    mbv_theme::Role::Accent.color().to_ratatui(),
                )),
            ),
            Span::raw(" "),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{EDE2}"
                } else {
                    "ABS"
                },
                Style::default().fg(service_state_color(
                    self.audiobookshelf_runtime.state,
                    mbv_theme::Role::AccentAudiobookshelf.color().to_ratatui(),
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
        // Peer queue link is omitted here: the active queue is already
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
    Some((label, mbv_theme::Role::TextMuted.color().to_ratatui()))
}

#[cfg(test)]
mod tests {
    use crate::app::dispatch::notify::ToastSeverity;
    use crate::app::tests::{
        make_app_stub, make_audio_items, make_local_daemon_app_stub_with_cmd_rx,
    };
    use mbv_render::components::widgets::service_state_color;

    /// The queue owner label names the connected peer's endpoint when its
    /// device name is empty — the connected endpoint, never a configured or
    /// assumed one (PR #973 review; issue #972 drops the configured-endpoint
    /// config key, the label contract stays).
    #[test]
    fn an_empty_named_peer_labels_the_connected_endpoint_not_the_configured_one() {
        let (mut app, _cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_audio_items(2));
        let mut sess = mbv_emby::test_support::make_session("", "mbv");
        sess.id = "peer-1".to_string();
        let (peer, peer_rx, _peer_cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(make_audio_items(1), 0);
        app.switch_to_direct_remote(
            &sess,
            peer,
            peer_rx,
            &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
        );

        assert_eq!(app.queue_owner_label(), "127.0.0.1");
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
                connection: mbv_ctrl::OutputConnection::Connected,
            },
        ));

        let (host, is_remote) = app.playback_host_label_and_remote();

        assert_eq!(host, "Living Room");
        assert!(is_remote);
    }

    /// A disconnected Cast output keeps naming the header's host with a
    /// " (disconnected)" suffix and still counts as remote, so the header
    /// presents the output as disconnected instead of looking like local
    /// playback (bug: a disconnected output looked like local playback;
    /// spec `cast-session-control` "Connection drops while attached",
    /// `owner-playback-output` "Device disappears" — every Client
    /// presents the output as disconnected).
    #[test]
    fn a_disconnected_cast_output_names_the_queue_header_as_disconnected() {
        let mut app = make_app_stub();
        app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
            mbv_ctrl::OutputState::Cast {
                receiver_id: "device-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::OutputConnection::Disconnected,
            },
        ));

        let (host, is_remote) = app.playback_host_label_and_remote();

        assert_eq!(host, "Living Room (disconnected)");
        assert!(is_remote);
    }

    /// With the default (non-Nerd-Font) config, the service indicators emit
    /// the plain text labels instead of Nerd Font private-use code points,
    /// which would paint as missing-glyph boxes (bug: the service glyphs
    /// ignored `use_nerd_fonts`).
    #[test]
    fn service_indicators_fall_back_to_text_without_nerd_fonts() {
        let app = make_app_stub();
        assert!(!app.use_nerd_fonts, "the stub defaults to no Nerd Fonts");

        let spans = app.status_bar_right_spans();

        assert!(
            !spans
                .iter()
                .any(|span| span.content.contains('\u{F06B4}')
                    || span.content.contains('\u{EDE2}')),
            "no Nerd Font private-use glyph without use_nerd_fonts"
        );
        assert!(spans.iter().any(|span| span.content.contains("EMBY")));
        assert!(spans.iter().any(|span| span.content.contains("ABS")));
    }

    /// The status row's Audiobookshelf glyph colours through the same
    /// `service_state_color` policy as the Emby glyph (issue #976: it was
    /// always painted `AccentAudiobookshelf`, so an unconfigured, connecting,
    /// or failed Service looked healthy).
    #[rstest::rstest]
    #[case::not_configured(mbv_core::service_runtime::ServiceState::NotConfigured)]
    #[case::ready(mbv_core::service_runtime::ServiceState::Ready)]
    #[case::unavailable(mbv_core::service_runtime::ServiceState::Unavailable)]
    fn the_abs_glyph_colour_follows_the_audiobookshelf_service_state(
        #[case] state: mbv_core::service_runtime::ServiceState,
    ) {
        let mut app = make_app_stub();
        app.audiobookshelf_runtime.state = state;

        let expected = service_state_color(
            state,
            mbv_theme::Role::AccentAudiobookshelf.color().to_ratatui(),
        );

        let spans = app.status_bar_right_spans();
        let abs = spans
            .iter()
            .find(|span| span.content.contains("ABS"))
            .expect("the status row shows the Audiobookshelf glyph");
        assert_eq!(abs.style.fg, Some(expected));
    }

    /// A live toast projects its message with the severity color on the
    /// status-bar surface; a delivered desktop notification hides the in-app
    /// row for non-neutral severities ("when the desktop notification
    /// succeeds, the in-app toast row is hidden"); an expired toast stops
    /// projecting (a toast expires on its severity-derived duration and
    /// claims no key). One outcome per case, stub setup in the body (spec
    /// `toast-notification-semantics`). (Bug regression context: the in-app
    /// toast render path was dropped during the `TuiRealm` migration, commit
    /// `e9f3ab957`.)
    #[rstest::rstest]
    #[case::live_toast_projects(ToastSeverity::Neutral, false, false, ToastExpiry::Live(2), true)]
    #[case::delivered_desktop_notification_hides_the_row(
        ToastSeverity::Error,
        true,
        false,
        ToastExpiry::Live(5),
        false
    )]
    #[case::expired_toast_stops_projecting(
        ToastSeverity::Neutral,
        false,
        false,
        ToastExpiry::Expired,
        false
    )]
    fn toast_projection_matches_the_spec(
        #[case] severity: ToastSeverity,
        #[case] system_notifications: bool,
        #[case] notif_failed: bool,
        #[case] expiry: ToastExpiry,
        #[case] projected: bool,
    ) {
        let mut app = make_app_stub();
        app.system_notifications = system_notifications;
        app.notif_failed = notif_failed;
        app.status = "Scanning 'Movies'...".into();
        app.status_severity = severity;
        app.status_expires = Some(toast_expiry(expiry));

        assert_eq!(
            app.toast_presentation()
                .is_some_and(|toast| toast.text.contains("Scanning")),
            projected,
        );
    }

    /// Case fixture: the toast under test's expiry instant, future or past.
    #[derive(Clone, Copy)]
    enum ToastExpiry {
        Live(u64),
        Expired,
    }

    fn toast_expiry(expiry: ToastExpiry) -> std::time::Instant {
        match expiry {
            ToastExpiry::Live(secs) => {
                std::time::Instant::now() + std::time::Duration::from_secs(secs)
            }
            ToastExpiry::Expired => std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap(),
        }
    }
}

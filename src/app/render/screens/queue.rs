use crate::app::layout::LayoutMain;
use crate::app::ui_util::*;
use crate::app::{palette, App, QueueScope, RemoteSlotState};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

// Extra gap reserved between the title and whatever follows it (inline
// percent and/or the right-aligned duration), on top of their own widths
// which are already accounted for separately via `pct_w`/`right_w`.
const QUEUE_TITLE_QUIET_COLUMNS: usize = 2;

/// Time text for one queue row. The now-playing row shows the moving
/// elapsed time next to its duration (`1:05 / 3:22`), matching the playback
/// panel's time readout; every other row shows just its duration. Empty
/// when the duration is unknown.
fn queue_row_time_text(pos_ticks: i64, dur_ticks: i64, show_elapsed: bool) -> String {
    let dur_s = dur_ticks / TICKS_PER_SECOND;
    if dur_s <= 0 {
        return String::new();
    }
    if show_elapsed {
        format!(
            "{} / {}",
            fmt_duration_short(pos_ticks / TICKS_PER_SECOND),
            fmt_duration_short(dur_s)
        )
    } else {
        fmt_duration_short(dur_s)
    }
}

impl App {
    /// Renders the "Queue" title pill (and optional Local/Remote scope pills)
    /// at the top of the queue column on a single row.
    pub(super) fn render_queue_title(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutMain,
    ) {
        if area.height < 1 {
            return;
        }

        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
        );

        layout.queue_scope_local_area = Rect::default();
        layout.queue_scope_remote_area = Rect::default();

        let remote_state = self.remote_slot_state();
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let local_selected = self.visible_queue_scope() == QueueScope::Local;
        let has_remote = matches!(remote_state, RemoteSlotState::DirectRemote);
        let has_attached = matches!(remote_state, RemoteSlotState::AttachedSession);
        let show_split = has_remote || has_attached;
        // DirectRemote only ever exists after a session was confirmed to be an
        // mbv/mbvd client (see `session_direct_endpoint`'s "mbv" check), so it's
        // always an mbv session; AttachedSession covers both mbv/mbvd sessions
        // not yet upgraded to a direct connection and plain Emby clients.
        let is_mbv_session = has_remote
            || (has_attached
                && self
                    .connected_session_state
                    .as_ref()
                    .is_some_and(|session| session.client.eq_ignore_ascii_case("mbv")));

        // --- Left "local" pill: display-only, no longer a scope toggle. ---
        let mut local_spans = self.remote_status_spans(crate::app::RemoteSlotState::Off, "");
        if show_split {
            if let Some(trailing) = local_spans.get_mut(3) {
                trailing.content = "".into();
            }
        }
        if self.use_nerd_fonts {
            if let Some(icon) = local_spans.get_mut(1) {
                icon.content = "\u{F0AFE}".into();
            }
        }
        let local_bg = palette::SURFACE_CHROME;
        let local_fg = palette::TEXT_FOCUS_ACCENT;
        if show_split {
            if let Some(label) = local_spans.get_mut(2) {
                label.content = " Connected: ".into();
            }
        }
        Self::set_status_pill_style(&mut local_spans, local_fg, local_bg);
        if let Some(icon) = local_spans.get_mut(1) {
            icon.style = icon.style.fg(palette::TEXT_METADATA);
        }
        Self::uppercase_status_label(&mut local_spans);

        let local_content_w: usize = local_spans.iter().map(|s| s.content.width()).sum();
        let local_w = (local_content_w as u16).min(area.width);
        let local_area = Rect {
            x: area.x,
            y: area.y,
            width: local_w,
            height: 1,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(local_bg)),
            local_area,
        );
        f.render_widget(Paragraph::new(Line::from(local_spans)), local_area);

        if !show_split {
            return;
        }

        // --- Button pill: local/remote queue-scope toggle, right-aligned.
        // Only rendered for mbv/mbvd sessions -- plain Emby sessions have no
        // remote queue to switch to.
        let remote_icon = if self.use_nerd_fonts {
            "\u{f1616}"
        } else {
            "\u{1F5A7}"
        };
        let local_btn_text = " \u{2302} ";
        let remote_btn_text = format!(" {remote_icon} ");
        let remote_btn_text_w = remote_btn_text.width() as u16;
        let local_btn_w = local_btn_text.width() as u16;
        let button_pill_w = if is_mbv_session {
            (local_btn_w + remote_btn_text_w).min(area.width.saturating_sub(local_w))
        } else {
            0
        };

        // --- Right "target" pill: display-only connected hostname/route. ---
        let target_x = area.x + local_w;
        let target_total_w = area.width.saturating_sub(local_w);
        let target_w = target_total_w.saturating_sub(button_pill_w);
        let target_area = Rect {
            x: target_x,
            y: area.y,
            width: target_w,
            height: 1,
        };
        let (_icon, label) = self.remote_icon_and_label(remote_state, &daemon_endpoint);
        let target_bg = palette::SURFACE_CHROME;
        let target_fg = if is_mbv_session {
            palette::ACCENT
        } else {
            palette::TEXT_FOCUS_ACCENT
        };
        let target_label_style = Style::default()
            .fg(target_fg)
            .bg(target_bg)
            .add_modifier(Modifier::BOLD);
        let tracking = if has_attached {
            self.remote_tracker
                .as_ref()
                .map(|tracker| {
                    let state = match tracker.state() {
                        mbv_core::remote_reconciliation::TrackingState::Starting => "STARTING",
                        mbv_core::remote_reconciliation::TrackingState::Tracking => "TRACKING",
                        mbv_core::remote_reconciliation::TrackingState::Ambiguous => "AMBIGUOUS",
                        mbv_core::remote_reconciliation::TrackingState::Invalid => "INVALID",
                        mbv_core::remote_reconciliation::TrackingState::Suspended => "SUSPENDED",
                    };
                    if matches!(
                        tracker.state(),
                        mbv_core::remote_reconciliation::TrackingState::Ambiguous
                            | mbv_core::remote_reconciliation::TrackingState::Invalid
                            | mbv_core::remote_reconciliation::TrackingState::Suspended
                    ) {
                        let reason = match tracker.reason() {
                            mbv_core::remote_reconciliation::TrackingReason::DuplicateCandidates => "duplicate",
                            mbv_core::remote_reconciliation::TrackingReason::SessionUnavailable => "session unavailable",
                            mbv_core::remote_reconciliation::TrackingReason::ReturningStateRequiresReanchor => "re-anchor required",
                            _ => "sequence mismatch",
                        };
                        format!(" · {state} ({reason})")
                    } else {
                        format!(" · {state}")
                    }
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        let target_label_text = format!("{}{}", label.trim_start(), tracking);
        f.render_widget(
            Block::default().style(Style::default().bg(target_bg)),
            target_area,
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                target_label_text,
                target_label_style,
            )])),
            target_area,
        );

        if !is_mbv_session {
            return;
        }

        let button_area = Rect {
            x: target_x + target_w,
            y: area.y,
            width: button_pill_w,
            height: 1,
        };
        let base_bg = palette::SURFACE_CHROME;
        let local_btn_bg = if local_selected {
            palette::ACCENT
        } else {
            palette::PILL_BG
        };
        let local_btn_fg = if local_selected {
            palette::TEXT_FOCUS_ACCENT
        } else {
            palette::PILL_FG
        };
        let remote_btn_bg = if local_selected {
            palette::PILL_BG
        } else {
            palette::ACCENT
        };
        let remote_btn_fg = if local_selected {
            palette::PILL_FG
        } else {
            palette::TEXT_FOCUS_ACCENT
        };
        let spans = vec![
            Span::styled(
                local_btn_text,
                Style::default().fg(local_btn_fg).bg(local_btn_bg),
            ),
            Span::styled(
                remote_btn_text,
                Style::default().fg(remote_btn_fg).bg(remote_btn_bg),
            ),
        ];
        f.render_widget(
            Block::default().style(Style::default().bg(base_bg)),
            button_area,
        );
        f.render_widget(Paragraph::new(Line::from(spans)), button_area);

        let local_btn_w = local_btn_w.min(button_area.width);
        layout.queue_scope_local_area = Rect {
            x: button_area.x,
            y: button_area.y,
            width: local_btn_w,
            height: 1,
        };
        let remote_btn_x = button_area.x + local_btn_w;
        let remote_btn_w = button_area.width.saturating_sub(local_btn_w);
        layout.queue_scope_remote_area = Rect {
            x: remote_btn_x,
            y: button_area.y,
            width: remote_btn_w,
            height: 1,
        };
    }
}

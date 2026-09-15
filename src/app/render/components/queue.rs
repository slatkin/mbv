use crate::app::components::media_list::{
    SelectedRowStyle, WideMediaList, WideMediaListPaintPolicy, ZebraStripe,
};
use crate::app::{palette, App, QueueScope, RemoteSlotState};
use mbv_core::playback_queue::QueueSlotId;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use tuirealm::component::Component;
use unicode_width::UnicodeWidthStr;

/// The active media-list presentation Queue hands to the render layer this
/// frame (design.md D1/D2). Queue keeps the Wide presentation in every panel
/// mode; the closed handoff keeps its body paint behind the carrier's
/// presentation seam instead of reaching into one adapter.
pub(in crate::app) enum QueuePresentation<'a> {
    Wide(&'a mut WideMediaList<QueueSlotId>),
}

/// Paint the Queue body through the handed-over presentation once: configure
/// the closed paint policy, paint the rows, and retain the current frame's
/// point-resolution facts. This is the sole Queue body painter.
pub(in crate::app) fn render_queue_body(
    frame: &mut Frame,
    area: Rect,
    presentation: QueuePresentation<'_>,
    focused: bool,
) {
    match presentation {
        QueuePresentation::Wide(list) => {
            list.set_paint_policy(
                WideMediaListPaintPolicy::for_queue(focused)
                    .with_zebra(ZebraStripe {
                        focused: Color::from_u32(0x003c4841),
                        unfocused: Color::from_u32(0x00333c43),
                    })
                    .with_selected_style(SelectedRowStyle {
                        bg: palette::QUEUE_SELECTED_ROW_BG,
                        title_fg: palette::QUEUE_SELECTED_ROW_BG,
                        title_bg: palette::TEXT_FOCUS_ACCENT,
                        duration_fg: palette::ACCENT,
                    }),
            );
            Component::view(list, frame, area);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct QueueTitleModel {
    pub local_icon: String,
    pub local_label: String,
    pub remote_icon: String,
    pub local_selected: bool,
    pub show_split: bool,
    pub is_mbv_session: bool,
}

impl App {
    pub(in crate::app) fn queue_title_model(&self) -> QueueTitleModel {
        let remote_state = self.remote_slot_state();
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let show_split = matches!(
            remote_state,
            RemoteSlotState::DirectRemote | RemoteSlotState::AttachedSession
        );
        let is_mbv_session = matches!(remote_state, RemoteSlotState::DirectRemote)
            || (matches!(remote_state, RemoteSlotState::AttachedSession)
                && self
                    .connected_session_state
                    .as_ref()
                    .is_some_and(|session| session.client.eq_ignore_ascii_case("mbv")));
        let mut local_spans = self.remote_status_spans(remote_state, &daemon_endpoint);
        if self.use_nerd_fonts {
            if let Some(icon) = local_spans.get_mut(1) {
                icon.content = "\u{F0AFE}".into();
            }
        }
        Self::set_status_pill_style(
            &mut local_spans,
            palette::TEXT_FOCUS_ACCENT,
            palette::surface_colors(palette::Surface::StatusBarPill, false).fill,
        );
        if let Some(icon) = local_spans.get_mut(1) {
            icon.style = icon.style.fg(palette::TEXT_METADATA);
        }
        Self::uppercase_status_label(&mut local_spans);
        // The column header already names the playback target (`on <host>`),
        // so the title drops the host label and keeps only the scope-toggle
        // icon; attached generic Sessions remain ordinary observed playback
        // targets without an additional status marker.
        let remote_icon = self.remote_icon_and_label(remote_state, &daemon_endpoint).0;
        QueueTitleModel {
            local_icon: local_spans
                .get(1)
                .map(|span| span.content.to_string())
                .unwrap_or_default(),
            local_label: local_spans
                .get(2)
                .map(|span| span.content.to_string())
                .unwrap_or_default(),
            remote_icon: remote_icon.to_string(),
            local_selected: self.viewed_queue_scope() == QueueScope::Local,
            show_split,
            is_mbv_session,
        }
    }
}

pub(in crate::app) fn render_queue_status(
    frame: &mut Frame,
    area: Rect,
    playlist: Vec<Span<'static>>,
    autosave: Option<Vec<Span<'static>>>,
    scope: Option<&QueueTitleModel>,
) -> (Option<Rect>, Option<Rect>) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::QueuePanelBand, false).fill),
        ),
        area,
    );
    frame.render_widget(Paragraph::new(Line::from(playlist)), area);
    // The Local/Remote scope pills (queue concern, shown only for an
    // mbv-based session): at the far right, winning over autosave and the
    // playlist on narrow footers — scope switching stays reachable.
    let (scope_local, scope_remote) = match scope.filter(|m| m.show_split && m.is_mbv_session) {
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
            let scope_w = local_w + remote_w;
            if scope_w == 0 || scope_w >= area.width {
                (None, None)
            } else {
                let scope_x = area.x + area.width - scope_w;
                frame.render_widget(
                    Paragraph::new(Line::from(vec![local_span, remote_span])),
                    Rect {
                        x: scope_x,
                        y: area.y,
                        width: scope_w,
                        height: 1,
                    },
                );
                (
                    Some(Rect {
                        x: scope_x,
                        y: area.y,
                        width: local_w,
                        height: 1,
                    }),
                    Some(Rect {
                        x: scope_x + local_w,
                        y: area.y,
                        width: remote_w,
                        height: 1,
                    }),
                )
            }
        }
        None => (None, None),
    };
    if let Some(spans) = autosave {
        let width = spans
            .iter()
            .map(|span| span.content.width() as u16)
            .sum::<u16>();
        // Autosave yields the far right to the scope pills when shown.
        let scope_w =
            scope_remote.map(|r| r.width).unwrap_or(0) + scope_local.map(|r| r.width).unwrap_or(0);
        let x = area.x + area.width.saturating_sub(scope_w).saturating_sub(width);
        if x > area.x {
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect {
                    x,
                    y: area.y,
                    width,
                    height: 1,
                },
            );
        }
    }
    (scope_local, scope_remote)
}

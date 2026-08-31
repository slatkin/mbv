use crate::app::render::components::chrome::thin_vertical_thumb;
use crate::app::render::components::list_rows::{selection_marker, MarkerEdge};
use crate::app::render::components::widgets::render_scrollbar_with_viewport_at;
use crate::app::types_playback::PlaybackState;
use crate::app::ui_util::*;
use crate::app::{palette, App, QueueScope, RemoteSlotState};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{QueueSlot, QueueSlotId};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

const QUEUE_TITLE_QUIET_COLUMNS: usize = 2;

#[derive(Default)]
pub(in crate::app) struct QueueRenderGeometry {
    pub rows: Vec<(Rect, QueueSlotId)>,
    pub scope_local_area: Rect,
    pub scope_remote_area: Rect,
}

#[derive(Clone, Default)]
pub(in crate::app) struct QueueTitleModel {
    pub local_icon: String,
    pub local_label: String,
    pub remote_icon: String,
    pub remote_label: String,
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
        if show_split {
            if let Some(trailing) = local_spans.get_mut(3) {
                trailing.content = "".into();
            }
            if let Some(label) = local_spans.get_mut(2) {
                label.content = if is_mbv_session {
                    " Connected: ".into()
                } else {
                    " Connected:".into()
                };
            }
        }
        if self.use_nerd_fonts {
            if let Some(icon) = local_spans.get_mut(1) {
                icon.content = "\u{F0AFE}".into();
            }
        }
        Self::set_status_pill_style(
            &mut local_spans,
            palette::TEXT_FOCUS_ACCENT,
            palette::SURFACE_CHROME,
        );
        if let Some(icon) = local_spans.get_mut(1) {
            icon.style = icon.style.fg(palette::TEXT_METADATA);
        }
        Self::uppercase_status_label(&mut local_spans);
        let (remote_icon, remote_label) =
            self.remote_icon_and_label(remote_state, &daemon_endpoint);
        let tracking = if matches!(remote_state, RemoteSlotState::AttachedSession) {
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
            remote_label: format!("{}{}", remote_label.trim_start(), tracking),
            local_selected: self.visible_queue_scope() == QueueScope::Local,
            show_split,
            is_mbv_session,
        }
    }
}

pub(in crate::app) fn render_queue_title_content(
    frame: &mut Frame,
    area: Rect,
    model: &QueueTitleModel,
    geometry: &mut QueueRenderGeometry,
) {
    if area.height < 1 {
        return;
    }
    geometry.scope_local_area = Rect::default();
    geometry.scope_remote_area = Rect::default();
    frame.render_widget(
        Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
        Rect { height: 1, ..area },
    );
    let mut local_spans = vec![
        Span::raw(" "),
        Span::raw(model.local_icon.clone()),
        Span::raw(model.local_label.clone()),
        Span::raw(" "),
    ];
    let local_bg = palette::SURFACE_CHROME;
    let local_fg = palette::TEXT_FOCUS_ACCENT;
    for span in &mut local_spans {
        span.style = Style::default().fg(local_fg).bg(local_bg);
    }
    if !model.show_split {
        local_spans[0].style = local_spans[0].style.fg(ratatui::style::Color::Reset);
    }
    if let Some(icon) = local_spans.get_mut(1) {
        icon.style = icon.style.fg(palette::TEXT_METADATA);
    }
    let local_w = (local_spans
        .iter()
        .map(|span| span.content.width())
        .sum::<usize>() as u16)
        .min(area.width);
    let local_area = Rect {
        width: local_w,
        height: 1,
        ..area
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(local_bg)),
        local_area,
    );
    frame.render_widget(Paragraph::new(Line::from(local_spans)), local_area);
    if !model.show_split {
        return;
    }
    let local_btn_text = " \u{2302} ";
    let remote_btn_text = format!(" {} ", model.remote_icon);
    let button_pill_w = if model.is_mbv_session {
        (local_btn_text.width() as u16 + remote_btn_text.width() as u16)
            .min(area.width.saturating_sub(local_w))
    } else {
        0
    };
    let target_area = Rect {
        x: area.x + local_w,
        width: area.width.saturating_sub(local_w + button_pill_w),
        height: 1,
        ..area
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
        target_area,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            model.remote_label.clone(),
            Style::default()
                .fg(if model.is_mbv_session {
                    palette::ACCENT
                } else {
                    palette::TEXT_FOCUS_ACCENT
                })
                .bg(palette::SURFACE_CHROME)
                .add_modifier(Modifier::BOLD),
        )])),
        target_area,
    );
    if !model.is_mbv_session {
        return;
    }
    let button_area = Rect {
        x: target_area.right(),
        width: button_pill_w,
        height: 1,
        ..area
    };
    let local_btn_w = (local_btn_text.width() as u16).min(button_area.width);
    let local_btn_area = Rect {
        width: local_btn_w,
        height: 1,
        ..button_area
    };
    geometry.scope_local_area = local_btn_area;
    geometry.scope_remote_area = Rect {
        x: button_area.x + local_btn_w,
        width: button_area.width.saturating_sub(local_btn_w),
        height: 1,
        ..button_area
    };
    let local_bg = if model.local_selected {
        palette::ACCENT
    } else {
        palette::PILL_BG
    };
    let local_fg = if model.local_selected {
        palette::TEXT_FOCUS_ACCENT
    } else {
        palette::PILL_FG
    };
    let remote_bg = if model.local_selected {
        palette::PILL_BG
    } else {
        palette::ACCENT
    };
    let remote_fg = if model.local_selected {
        palette::PILL_FG
    } else {
        palette::TEXT_FOCUS_ACCENT
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(local_btn_text, Style::default().fg(local_fg).bg(local_bg)),
            Span::styled(
                remote_btn_text,
                Style::default().fg(remote_fg).bg(remote_bg),
            ),
        ])),
        button_area,
    );
}

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

pub(in crate::app) fn render_queue_content(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    slots: &[QueueSlot],
    cursor: &mut usize,
    scroll: &mut usize,
    playback: PlaybackState,
    empty_text: &str,
    geometry: &mut QueueRenderGeometry,
) {
    geometry.rows.clear();
    if area.height < 1 {
        return;
    }
    if slots.is_empty() {
        *scroll = 0;
        frame.render_widget(
            Paragraph::new(empty_text).style(Style::default().fg(palette::TEXT_MUTED)),
            area,
        );
        return;
    }

    *cursor = (*cursor).min(slots.len() - 1);
    let visible = area.height as usize;
    let max_offset = slots.len().saturating_sub(visible);
    *scroll = (*scroll).min(max_offset);
    if *cursor < *scroll {
        *scroll = *cursor;
    } else if *cursor >= *scroll + visible {
        *scroll = cursor.saturating_sub(visible.saturating_sub(1));
    }
    let offset = *scroll;
    let has_sb = slots.len() > visible;
    let need_sb = has_sb && focused;
    let render_w = area.width.saturating_sub(u16::from(has_sb)) as usize;
    let show_length = render_w > 30;
    let mut list_items = Vec::new();
    for (visible_index, slot) in slots.iter().enumerate().skip(offset) {
        if visible_index - offset >= visible {
            break;
        }
        let is_active = playback.active && playback.active_idx == visible_index;
        let is_cursor = visible_index == *cursor && focused;
        let fg = if is_cursor || focused {
            palette::TEXT_STRONG
        } else {
            palette::QUEUE_ROW_FG
        };
        let row_y = area.y + (visible_index - offset) as u16;
        let row = Rect {
            x: area.x,
            y: row_y,
            width: area.width,
            height: 1,
        };
        geometry.rows.push((row, slot.slot_id));
        let (title_raw, pos_ticks, duration_ticks, pct_str) = match &slot.item {
            mbv_core::playback_queue::QueueItem::Emby(item) => {
                let (pos, runtime) = if is_active {
                    (
                        if playback.position_ticks > 0 {
                            playback.position_ticks
                        } else {
                            item.playback_position_ticks
                        },
                        playback.runtime_ticks,
                    )
                } else {
                    (item.playback_position_ticks, item.runtime_ticks)
                };
                let pct = if item.is_audio() {
                    String::new()
                } else {
                    fmt_playback_pct(pos, runtime)
                };
                (item.name.as_str(), pos, runtime, pct)
            }
            mbv_core::playback_queue::QueueItem::Feed(entry) => (
                entry.title.as_str(),
                if is_active {
                    playback.position_ticks
                } else {
                    0
                },
                entry.duration_ticks.unwrap_or(0) as i64,
                String::new(),
            ),
            mbv_core::playback_queue::QueueItem::Audiobookshelf(ep) => (
                ep.title.as_str(),
                if is_active {
                    playback.position_ticks
                } else {
                    0
                },
                ep.duration_ticks.unwrap_or(0) as i64,
                String::new(),
            ),
            mbv_core::playback_queue::QueueItem::AudiobookshelfBook(book) => (
                book.title.as_str(),
                if is_active {
                    playback.position_ticks
                } else {
                    0
                },
                book.duration_ticks.unwrap_or(0) as i64,
                String::new(),
            ),
        };
        let dur = queue_row_time_text(pos_ticks, duration_ticks, is_active);
        let dur_visible = show_length && !dur.is_empty();
        let pct_visible = !pct_str.is_empty();
        let pct_w = if pct_visible { 1 + pct_str.width() } else { 0 };
        let right_w = if dur_visible { dur.width() } else { 0 };
        let track_content_w = render_w.saturating_sub(2);
        let indent = 2;
        let title_w =
            track_content_w.saturating_sub(indent + pct_w + right_w + QUEUE_TITLE_QUIET_COLUMNS);
        let title = trunc_str(title_raw, title_w);
        if is_cursor {
            frame.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
                row,
            );
        }
        let title_color = if is_active {
            palette::ACCENT
        } else if !focused {
            palette::TEXT_MUTED
        } else {
            fg
        };
        let title_w_actual = title.width();
        let mut spans = vec![
            selection_marker(is_cursor, MarkerEdge::Left),
            Span::raw(" "),
        ];
        spans.push(Span::styled(title, Style::default().fg(title_color)));
        if pct_visible {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                pct_str,
                Style::default().fg(palette::TEXT_METADATA),
            ));
        }
        if dur_visible {
            let used = indent + title_w_actual + pct_w;
            let pad = track_content_w.saturating_sub(used + right_w);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(
                dur,
                Style::default().fg(palette::STATUS_AVAILABLE),
            ));
        }
        list_items.push(ListItem::new(Line::from(spans)).style(Style::default().fg(fg)));
    }
    let mut state = ListState::default();
    state.select(Some(cursor.saturating_sub(offset)));
    frame.render_stateful_widget(
        List::new(list_items).highlight_style(Style::default()),
        Rect {
            width: render_w as u16,
            ..area
        },
        &mut state,
    );
    if need_sb {
        render_scrollbar_with_viewport_at(
            frame,
            area,
            slots.len(),
            visible,
            offset,
            area.x + area.width.saturating_sub(1),
            thin_vertical_thumb(tui_scrollbar::GlyphSet::minimal()),
            palette::TEXT_EMPHASIS,
        );
    }
}
